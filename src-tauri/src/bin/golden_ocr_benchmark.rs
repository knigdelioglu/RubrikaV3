//! Golden OCR / scoring benchmark runner (Faz 7+).
//!
//! Drives the committed golden corpus (`testdata/golden/tymm_tde_001/`) through
//! the real model pipeline and reports the documented `GoldenOcrBenchmarkReport`.
//!
//! Pipeline per question:
//!   1. render the golden PDF (default: the scanned variant `03_...`) at a fixed
//!      DPI with poppler `pdftoppm`;
//!   2. crop each golden answer region with the production crop math
//!      (`crop_rect_normalized`, 4% margin, clamped);
//!   3. deskew each crop with the production deskew function
//!      (`ocr_image_geometry_service::deskew_image`) and apply the deterministic
//!      preprocess-variant selection (`ocr_image_preprocess_service`);
//!   4. prepare model inputs with the production JPEG encoder
//!      (`ModelInputImageService::prepare_inputs`);
//!   5. send the request through the production gateway
//!      (`LlamaServerGateway::extract_student_answer_ocr`) using the exact
//!      production OCR prompt contract and sampling parameters;
//!   6. measure CER/WER/critical-token/leakage/structured-exact with
//!      `golden_ocr_metrics` against the golden ground truth.
//!
//! Golden files are only read; every artifact (pages, crops, model inputs,
//! responses, the report) is written under the `--outdir`. Use `--skip-model`
//! for a model-independent preview report (deskew/registration/crops only),
//! which mirrors the `NEEDS_MODEL_RUNTIME` DTO state.
//!
//! Run:
//!   cargo run --bin golden_ocr_benchmark -- --outdir <dir> [--base-url http://127.0.0.1:8080] [--dpi 300]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use clap::Parser;
use image::GenericImageView;
use serde_json::{json, Value};

use app_lib::domain::model::{
    ModelInputImageKind, ModelRequestKind, ModelResponseFormat, StudentAnswerOcrRequest,
};
use app_lib::domain::question::AnswerType;
use app_lib::domain::student::NormalizedBBox;
use app_lib::services::golden_ocr_metrics::{
    character_error_rate, corpus_bbox_bottom_left_y_to_top_left, critical_token_error,
    percentile_p50, percentile_p95, printed_question_leakage, structured_field_exact_match,
    word_error_rate, GoldenAggregateMetric, GoldenModelRuntimeStatus, GoldenOcrBenchmarkReport,
    GoldenQuestionMetric,
};
use app_lib::services::llama_server_gateway::LlamaServerGateway;
use app_lib::services::model_gateway::ModelGateway;
use app_lib::services::model_input_image_service::ModelInputImageService;
use app_lib::services::ocr_image_geometry_service::{
    deskew_image, measure_registration_deviation, DESKEW_DEFAULT_MAX_ANGLE,
};
use app_lib::services::ocr_image_preprocess_service::{
    compute_image_statistics, select_preprocess_variant, ImageStatistics,
    OcrImagePreprocessService, DEFAULT_PREPROCESS_MODE,
};
use app_lib::services::prompt_contract::{build_prompt_contract, default_sampling};
use app_lib::services::student_answer_crop_service::crop_rect_normalized;
use app_lib::services::student_answer_ocr_service::{
    answer_type_label, build_student_answer_ocr_prompt, PREPROCESS_VERSION, PROMPT_VERSION,
};

const DEFAULT_GOLDEN_DIR: &str = "../testdata/golden/tymm_tde_001";
const EXAM_ID: &str = "tymm_tde_001";
const SCANNED_PDF: &str = "03_Doldurulmus_Tarama_Varyanti.pdf";
const RUBRIC_JSON: &str = "05_Rubrik_Golden.json";
const EXPECTATIONS_JSON: &str = "06_Golden_Set_Beklentileri.json";
const MANIFEST_JSON: &str = "manifest.sha256";
const REPORT_SCHEMA_VERSION: &str = "rubrika.golden.ocr.benchmark.v1";
const LAYOUT_HINT: &str = "manual answer regions";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);

/// Printed question text for each golden question (source: `01_Bos_Sinav_Kagidi.pdf`).
const PRINTED_QUESTIONS: [(&str, &str); 6] = [
    (
        "q1",
        "Metindeki temel çatışmayı belirleyiniz. Cevabınızı metinden iki ayrı kanıtla gerekçelendiriniz.",
    ),
    ("q2", "Metni aşağıdaki yapı unsurlarına göre çözümleyiniz."),
    ("q3", "Cümleleri kullanılan söz sanatıyla eşleştiriniz."),
    (
        "q4",
        "Aşağıdaki cümleleri yazım ve noktalama bakımından düzeltiniz.",
    ),
    (
        "q5",
        "“Ece, defteri müdür yardımcısına teslim etmek için koridorda hızla yürüdü.” cümlesini ögelerine ayırınız ve yüklemin yapısını belirtiniz.",
    ),
    (
        "q6",
        "Şiirin baskın duygusunu belirleyiniz. Bir dizeyi kanıt göstererek yorumlayınız.",
    ),
];

/// Fixed field order of the golden Q2 table (rubric `fields`, document order).
const Q2_FIELD_ORDER: [&str; 5] = [
    "Anlatıcı",
    "Mekân",
    "Zaman",
    "Ece’nin özelliği",
    "Metinsel kanıt",
];

/// Q3 answer key sequence (ground truth, 06). The rubric (05) matches items 1-4
/// and diverges at item 5 (`5-E`); the canonical-key decision is documented.
const Q3_GROUND_TRUTH: &str = "1-A, 2-B, 3-C, 4-D, 5-A";

#[derive(Parser)]
#[command(name = "golden_ocr_benchmark")]
struct Cli {
    /// Committed golden corpus directory (contains 05/06 JSONs + the PDFs).
    #[arg(long)]
    golden_dir: Option<PathBuf>,
    /// Working/output directory; never the golden dir.
    #[arg(long)]
    outdir: PathBuf,
    /// Live model server base URL.
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    base_url: String,
    /// Render DPI for the source PDF (golden quality gates assume 300).
    #[arg(long, default_value_t = 300)]
    dpi: u32,
    /// Golden exam PDF to benchmark (default: the scanned variant).
    #[arg(long, default_value = SCANNED_PDF)]
    pdf: String,
    /// Report-only mode: render/deskew/registration/crops, no model calls.
    #[arg(long)]
    skip_model: bool,
}

#[derive(Debug, Clone)]
struct QuestionSpec {
    id: String,
    number: u32,
    answer_type: AnswerType,
    question_text: String,
    printed_question: String,
}

impl QuestionSpec {
    fn all() -> Vec<QuestionSpec> {
        let specs = [
            ("q1", 1u32, AnswerType::Essay),
            ("q2", 2, AnswerType::Table),
            ("q3", 3, AnswerType::Matching),
            ("q4", 4, AnswerType::CorrectionTable),
            ("q5", 5, AnswerType::GrammarAnalysis),
            ("q6", 6, AnswerType::Essay),
        ];
        specs
            .into_iter()
            .map(|(id, number, answer_type)| {
                let printed_question = PRINTED_QUESTIONS
                    .iter()
                    .find(|(pid, _)| *pid == id)
                    .map(|(_, text)| text.to_string())
                    .unwrap_or_default();
                QuestionSpec {
                    id: id.to_string(),
                    number,
                    answer_type,
                    question_text: printed_question.clone(),
                    printed_question,
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct RegionSpec {
    page: u32,
    bbox: NormalizedBBox,
    #[allow(dead_code)]
    role: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct QuestionRunResult {
    question_id: String,
    number: u32,
    answer_type: String,
    pages: Vec<u32>,
    region_count: usize,
    images: Vec<String>,
    ok: bool,
    error: Option<String>,
    answer_text: String,
    structured_hypothesis: Option<Vec<String>>,
    structured_exact: Option<bool>,
    reference: String,
    reference_fields: Vec<String>,
    cer: Option<f64>,
    wer: Option<f64>,
    critical_missing: Option<usize>,
    critical_missing_tokens: Vec<String>,
    leakage: Option<bool>,
    leakage_overlap: Vec<String>,
    duration_ms: Option<u64>,
    parse_error: Option<String>,
    printed_question_leak_detected: Option<bool>,
    deskew_angles: Vec<f32>,
    selected_preprocess_mode: String,
    preprocess_reason: String,
}

fn sha256_hex(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).expect("read file for hashing");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    hex::encode(hasher.finalize())
}

fn read_json(path: &Path) -> Value {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

/// The committed golden corpus stores normalized region coordinates in PDF
/// user space (bottom-left y origin: `y` grows upward). The production crop
/// math (`crop_rect_normalized`) and the domain `NormalizedBBox` convention use
/// a top-left y origin (`y` grows downward). This is a latent corpus defect:
/// feeding the 06 bboxes straight into `crop_rect_normalized` (as the golden
/// integration test helper does) crops the wrong area on every page — only Q5
/// happens to align. The benchmark runner therefore converts the corpus bboxes
/// to the top-left convention (single shared function in `golden_ocr_metrics`)
/// before cropping so it measures OCR of the actual golden content.
fn normalize_bbox(values: &[Value]) -> NormalizedBBox {
    let x = values[0].as_f64().expect("bbox x") as f32;
    let y_bottom = values[1].as_f64().expect("bbox y") as f32;
    let width = values[2].as_f64().expect("bbox w") as f32;
    let height = values[3].as_f64().expect("bbox h") as f32;
    NormalizedBBox {
        x,
        y: corpus_bbox_bottom_left_y_to_top_left(y_bottom, height),
        width,
        height,
    }
}

fn regions_for_question(expectations: &Value, question_id: &str) -> Vec<RegionSpec> {
    expectations["regions"][question_id]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| RegionSpec {
                    page: entry["page"].as_u64().expect("region page") as u32,
                    bbox: normalize_bbox(entry["bbox_normalized"].as_array().expect("bbox array")),
                    role: entry["role"].as_str().unwrap_or("primary").to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn regions_for_page(expectations: &Value, page: u32) -> Vec<NormalizedBBox> {
    let mut regions = Vec::new();
    if let Some(map) = expectations["regions"].as_object() {
        for entries in map.values() {
            if let Some(entries) = entries.as_array() {
                for entry in entries {
                    if entry["page"].as_u64().expect("region page") != page as u64 {
                        continue;
                    }
                    regions.push(normalize_bbox(
                        entry["bbox_normalized"].as_array().expect("bbox array"),
                    ));
                }
            }
        }
    }
    regions
}

fn render_pdf_pages(pdf: &Path, dpi: u32, outdir: &Path) -> Vec<PathBuf> {
    let prefix = outdir.join("page");
    let status = Command::new("pdftoppm")
        .arg("-png")
        .arg("-r")
        .arg(dpi.to_string())
        .arg(pdf)
        .arg(&prefix)
        .status()
        .expect("failed to spawn pdftoppm (poppler)");
    assert!(status.success(), "pdftoppm exited with {status:?}");
    let mut pages = std::fs::read_dir(outdir)
        .expect("read render dir")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|stem| stem.starts_with("page-"))
        })
        .collect::<Vec<_>>();
    pages.sort();
    pages
}

fn save_image(image: &image::DynamicImage, path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create output dir");
    }
    image.save(path).expect("save image");
}

/// Ground-truth comparison string for a question (06 `ocr_ground_truth`).
fn ground_truth_reference(expectations: &Value, id: &str) -> String {
    let gt = &expectations["ocr_ground_truth"];
    match id {
        "q2" => {
            let obj = gt["q2"].as_object().expect("q2 ground truth object");
            Q2_FIELD_ORDER
                .iter()
                .map(|field| obj.get(*field).and_then(Value::as_str).unwrap_or_default())
                .collect::<Vec<_>>()
                .join(" | ")
        }
        "q4" => {
            let obj = gt["q4"].as_object().expect("q4 ground truth object");
            ["a", "b", "c"]
                .iter()
                .map(|key| obj.get(*key).and_then(Value::as_str).unwrap_or_default())
                .collect::<Vec<_>>()
                .join(" | ")
        }
        _ => gt[id].as_str().unwrap_or_default().to_string(),
    }
}

/// Structured ground-truth field values for the exact-match gate (Q2/Q4).
fn ground_truth_fields(expectations: &Value, id: &str) -> Vec<String> {
    let gt = &expectations["ocr_ground_truth"];
    match id {
        "q2" => {
            let obj = gt["q2"].as_object().expect("q2 ground truth object");
            Q2_FIELD_ORDER
                .iter()
                .map(|field| {
                    obj.get(*field)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                })
                .collect()
        }
        "q4" => {
            let obj = gt["q4"].as_object().expect("q4 ground truth object");
            ["a", "b", "c"]
                .iter()
                .map(|key| {
                    obj.get(*key)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                })
                .collect()
        }
        _ => vec![gt[id].as_str().unwrap_or_default().to_string()],
    }
}

/// Flattens the model `structuredAnswer` into comparable field values in
/// document order. Returns `None` when no structured answer was parsed.
fn structured_hypothesis(
    id: &str,
    structured: &Option<app_lib::domain::structured_answer::StructuredAnswer>,
) -> Option<Vec<String>> {
    let value = serde_json::to_value(structured.as_ref()?).ok()?;
    match id {
        "q2" => {
            let rows = value["rows"].as_array()?;
            let mut indexed = rows
                .iter()
                .filter_map(|row| {
                    let index = row["index"].as_u64()?;
                    let cell = row["cells"].as_array()?.first()?.as_str()?.to_string();
                    Some((index, cell))
                })
                .collect::<Vec<_>>();
            indexed.sort_by_key(|(index, _)| *index);
            Some(indexed.into_iter().map(|(_, cell)| cell).collect())
        }
        "q4" => {
            let rows = value["rows"].as_array()?;
            let mut indexed = rows
                .iter()
                .filter_map(|row| {
                    let index = row["index"].as_u64()?;
                    let correction = row["correction"].as_str()?.to_string();
                    Some((index, correction))
                })
                .collect::<Vec<_>>();
            indexed.sort_by_key(|(index, _)| *index);
            Some(
                indexed
                    .into_iter()
                    .map(|(_, correction)| correction)
                    .collect(),
            )
        }
        "q5" => {
            let items = value["items"].as_array()?;
            let mut indexed = items
                .iter()
                .enumerate()
                .filter_map(|(position, item)| {
                    let text = item["text"].as_str()?.to_string();
                    let label = item["label"].as_str().unwrap_or_default().to_string();
                    Some((position as u32, format!("{label}: {text}")))
                })
                .collect::<Vec<_>>();
            indexed.sort_by_key(|(position, _)| *position);
            Some(indexed.into_iter().map(|(_, item)| item).collect())
        }
        _ => None,
    }
}

fn critical_tokens_for(id: &str) -> Vec<&'static str> {
    match id {
        "q1" => vec!["Ece", "müdür yardımcısı", "sorumluluk"],
        "q2" => vec![
            "Üçüncü kişi anlatıcı",
            "Okulun eski kütüphanesi",
            "Yağmurlu bir okul günü",
            "Sorumluluk sahibi",
            "müdür yardımcısına teslim",
        ],
        "q3" => vec!["1-A", "2-B", "3-C", "4-D"],
        "q4" => vec!["Küçük Ağa", "gideceksin", "her şey"],
        "q5" => vec!["Özne", "Belirtili nesne", "Yüklem"],
        "q6" => vec!["ayrılık", "umut", "gitmek bazen dönmeyi anlamaktır"],
        _ => vec![],
    }
}

fn golden_dir_from_env() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_GOLDEN_DIR)
}

fn rubric_q3_sequence(rubric: &Value) -> Option<String> {
    let answer_key = rubric["questions"]
        .as_array()?
        .iter()
        .find(|q| q["question_id"] == "q3")?
        .get("answer_key")?
        .as_object()?;
    Some(
        (1u32..=5)
            .map(|n| {
                format!(
                    "{n}-{}",
                    answer_key[&n.to_string()].as_str().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join(", "),
    )
}

struct MetricInputs {
    spec: QuestionSpec,
    expectations: Value,
    rubric_q3: Option<String>,
}

/// Per-question run context passed into metric computation; keeps the
/// `compute_metrics` signature small (clippy `too_many_arguments`).
struct RunOutcome {
    error: Option<String>,
    duration_ms: Option<u64>,
    deskew_angles: Vec<f32>,
    images: Vec<String>,
    selected_preprocess_mode: String,
    preprocess_reason: String,
}

fn compute_metrics(
    inputs: &MetricInputs,
    result: Option<&app_lib::domain::model::StudentAnswerOcrResult>,
    outcome: RunOutcome,
) -> QuestionRunResult {
    let RunOutcome {
        error,
        duration_ms,
        deskew_angles,
        images,
        selected_preprocess_mode,
        preprocess_reason,
    } = outcome;
    let spec = &inputs.spec;
    let mut run = QuestionRunResult {
        question_id: spec.id.clone(),
        number: spec.number,
        answer_type: answer_type_label(&spec.answer_type).to_string(),
        pages: regions_for_question(&inputs.expectations, &spec.id)
            .iter()
            .map(|r| r.page)
            .collect(),
        region_count: regions_for_question(&inputs.expectations, &spec.id).len(),
        images,
        ok: result.is_some() && error.is_none(),
        error,
        answer_text: result
            .map(|r| r.output.answer_text.clone())
            .unwrap_or_default(),
        structured_hypothesis: result
            .and_then(|r| structured_hypothesis(&spec.id, &r.output.structured_answer)),
        structured_exact: None,
        reference: ground_truth_reference(&inputs.expectations, &spec.id),
        reference_fields: ground_truth_fields(&inputs.expectations, &spec.id),
        cer: None,
        wer: None,
        critical_missing: None,
        critical_missing_tokens: Vec::new(),
        leakage: None,
        leakage_overlap: Vec::new(),
        duration_ms,
        parse_error: result.and_then(|r| r.parse_error.clone()),
        printed_question_leak_detected: result.map(|r| r.printed_question_leak_detected),
        deskew_angles,
        selected_preprocess_mode,
        preprocess_reason,
    };

    let Some(result) = result else {
        return run;
    };

    // Structured exact-match gate (quality gate `structured_field_exact_match_min`).
    // When a structured-type question produced no parseable typed structured
    // answer, the gate fails closed (production marks it needs_review) — the
    // OCR text may still be perfect (visible via CER/WER on the answer text).
    if matches!(spec.id.as_str(), "q2" | "q4" | "q5") {
        if let Some(hyp) = &run.structured_hypothesis {
            let expected = ground_truth_fields(&inputs.expectations, &spec.id);
            run.structured_exact = Some(
                expected.len() == hyp.len()
                    && expected
                        .iter()
                        .zip(hyp.iter())
                        .all(|(e, h)| structured_field_exact_match(e, h)),
            );
        } else {
            run.structured_exact = Some(false);
        }
    }

    // Hypothesis text used for CER/WER/critical/leakage.
    let hypothesis_text = if matches!(spec.id.as_str(), "q2" | "q4" | "q5") {
        run.structured_hypothesis
            .as_ref()
            .map(|fields| fields.join(" | "))
            .unwrap_or_else(|| result.output.answer_text.clone())
    } else if spec.id == "q3" {
        run.structured_hypothesis
            .as_ref()
            .map(|pairs| pairs.join(", "))
            .unwrap_or_else(|| result.output.answer_text.clone())
    } else {
        result.output.answer_text.clone()
    };

    if spec.id == "q3" {
        // The typed matching structure may be absent even when the OCR text is
        // the exact answer key sequence; accept either the structured pairs or
        // the plain answer text against the ground-truth / rubric key.
        let hyp_seq = match &run.structured_hypothesis {
            Some(pairs) => pairs.join(", "),
            None => result.output.answer_text.clone(),
        };
        let mut exact = structured_field_exact_match(Q3_GROUND_TRUTH, &hyp_seq);
        if !exact {
            exact = inputs
                .rubric_q3
                .as_ref()
                .is_some_and(|r| structured_field_exact_match(r, &hyp_seq));
        }
        run.structured_exact = Some(exact);
    }

    if !run.reference.trim().is_empty() {
        run.cer = Some(character_error_rate(&run.reference, &hypothesis_text));
        run.wer = Some(word_error_rate(&run.reference, &hypothesis_text));
    }

    let tokens = critical_tokens_for(&spec.id);
    if !tokens.is_empty() {
        let report = critical_token_error(&run.reference, &hypothesis_text, &tokens);
        run.critical_missing = Some(report.missing_count());
        run.critical_missing_tokens = report.missing_tokens;
    }

    if !spec.printed_question.is_empty() {
        let report = printed_question_leakage(&hypothesis_text, &spec.printed_question);
        run.leakage = Some(report.leaked);
        run.leakage_overlap = report.overlap_words;
    }

    run
}

fn aggregate_stats(stats: &[ImageStatistics]) -> Option<ImageStatistics> {
    if stats.is_empty() {
        return None;
    }
    let count = stats.len() as f32;
    Some(ImageStatistics {
        mean: stats.iter().map(|s| s.mean).sum::<f32>() / count,
        std_dev: stats.iter().map(|s| s.std_dev).sum::<f32>() / count,
        edge_density: stats.iter().map(|s| s.edge_density).sum::<f32>() / count,
    })
}

fn build_request(
    spec: &QuestionSpec,
    expectations: &Value,
    model_inputs: &[app_lib::domain::model::ModelInputImage],
    selected_mode: app_lib::domain::student::OcrImagePreprocessMode,
) -> StudentAnswerOcrRequest {
    let regions = regions_for_question(expectations, &spec.id);
    let region_ids = regions
        .iter()
        .enumerate()
        .map(|(order, _)| format!("{}-region-{order}", spec.id))
        .collect::<Vec<_>>();
    let region_orders = (0..regions.len() as u32).collect::<Vec<_>>();
    let region_page_offsets = regions
        .iter()
        .map(|r| r.page.saturating_sub(1))
        .collect::<Vec<_>>();
    let source_page_numbers = regions.iter().map(|r| r.page).collect::<Vec<_>>();

    let prompt = build_student_answer_ocr_prompt(
        spec.number,
        &spec.question_text,
        &spec.answer_type,
        LAYOUT_HINT,
    );
    let contract = build_prompt_contract(
        ModelRequestKind::Ocr,
        PROMPT_VERSION,
        "student_answer_ocr_output_v1",
        "ocr_review_policy_v1",
        prompt.clone(),
        json!({
            "questionNumber": spec.number,
            "questionText": spec.question_text,
            "answerType": answer_type_label(&spec.answer_type),
            "layoutHint": LAYOUT_HINT,
            "preprocessMode": selected_mode,
            "preprocessVersion": PREPROCESS_VERSION,
            "sourcePageNumbers": source_page_numbers,
            "regionIds": region_ids,
            "regionOrders": region_orders,
            "regionPageOffsets": region_page_offsets,
        }),
        default_sampling(4096),
        Some(ModelResponseFormat::JsonObject),
        None,
    );

    StudentAnswerOcrRequest {
        prompt,
        prompt_contract: Some(contract),
        project_root_path: None,
        job_id: None,
        submission_id: "golden_tymm_tde_001".to_string(),
        question_id: spec.id.clone(),
        question_number: spec.number,
        question_text: spec.question_text.clone(),
        answer_type: answer_type_label(&spec.answer_type).to_string(),
        preprocess_mode: Some(selected_mode),
        preprocess_version: Some(PREPROCESS_VERSION.to_string()),
        model_input_crop_ref: model_inputs
            .first()
            .map(|image| image.output_image_path.clone()),
        source_page_numbers,
        region_ids,
        region_orders,
        region_page_offsets,
        model_input_images: model_inputs.to_vec(),
    }
}

async fn run_question(
    spec: &QuestionSpec,
    expectations: &Value,
    rubric: &Value,
    deskewed_pages: &BTreeMap<u32, PathBuf>,
    outdir: &Path,
    gateway: Option<&LlamaServerGateway>,
) -> QuestionRunResult {
    let question_out = outdir.join("results").join(&spec.id);
    std::fs::create_dir_all(&question_out).expect("create question dir");

    let regions = regions_for_question(expectations, &spec.id);
    let mut deskew_angles = Vec::new();
    let mut crops = Vec::new();
    for (order, region) in regions.iter().enumerate() {
        let page_image = image::open(
            deskewed_pages
                .get(&region.page)
                .unwrap_or_else(|| panic!("no deskewed page {} for {}", region.page, spec.id)),
        )
        .expect("open deskewed page");
        let (width, height) = page_image.dimensions();
        let (x, y, w, h, _clamped, _margin) = crop_rect_normalized(&region.bbox, width, height);
        let crop = page_image.crop_imm(x, y, w, h);
        let deskewed = deskew_image(&crop, DESKEW_DEFAULT_MAX_ANGLE).unwrap_or_else(|e| {
            panic!(
                "deskew failed for {} region {}: {}",
                spec.id, order, e.message
            )
        });
        deskew_angles.push(deskewed.angle_degrees);
        let crop_path = outdir
            .join("crops")
            .join(format!("{}_{}_region_{order}.png", spec.id, spec.number));
        save_image(&deskewed.image, &crop_path);
        crops.push((region.page, crop_path));
    }

    // Deterministic preprocess-variant selection (TD-22) over the deskewed crops.
    let stats = crops
        .iter()
        .filter_map(|(_, path)| image::open(path).ok())
        .map(|image| compute_image_statistics(&image))
        .collect::<Vec<_>>();
    let selection = aggregate_stats(&stats)
        .map(|aggregate| select_preprocess_variant(&aggregate))
        .unwrap_or_else(|| {
            select_preprocess_variant(&ImageStatistics {
                mean: 0.0,
                std_dev: 0.0,
                edge_density: 0.0,
            })
        });
    let selected_mode = selection.selected;

    // Apply the selected variant with the production preprocess service; fall
    // back to the deskewed crop when enhancement fails.
    let preprocess_service = OcrImagePreprocessService;
    let mut prepared_crops = Vec::new();
    for (page, crop_path) in &crops {
        if selected_mode == DEFAULT_PREPROCESS_MODE {
            prepared_crops.push((*page, crop_path.clone()));
        } else {
            match preprocess_service.preprocess_image(outdir, crop_path, selected_mode) {
                Ok(result) => prepared_crops.push((*page, PathBuf::from(result.output_image_path))),
                Err(_) => prepared_crops.push((*page, crop_path.clone())),
            }
        }
    }

    let inputs = MetricInputs {
        spec: spec.clone(),
        expectations: expectations.clone(),
        rubric_q3: rubric_q3_sequence(rubric),
    };
    let mode_label = format!("{selected_mode:?}");
    let reason = selection.reason.clone();
    let build_outcome =
        |error: Option<String>, duration_ms: Option<u64>, images: Vec<String>| RunOutcome {
            error,
            duration_ms,
            deskew_angles: deskew_angles.clone(),
            images,
            selected_preprocess_mode: mode_label.clone(),
            preprocess_reason: reason.clone(),
        };

    // Production JPEG model-input preparation.
    let batch_id = format!("golden_tymm_tde_001_q{}", spec.number);
    let images = match ModelInputImageService::default().prepare_inputs(
        outdir,
        ModelInputImageKind::StudentOcr,
        &batch_id,
        &prepared_crops,
    ) {
        Ok(images) => images,
        Err(error) => {
            let outcome = build_outcome(
                Some(format!(
                    "image_preparation_error:{:?}:{}",
                    error.code, error.message
                )),
                None,
                Vec::new(),
            );
            return compute_metrics(&inputs, None, outcome);
        }
    };
    let images_meta = images
        .iter()
        .map(|image| image.output_image_path.clone())
        .collect::<Vec<_>>();

    let Some(gateway) = gateway else {
        let outcome = build_outcome(None, None, images_meta);
        return compute_metrics(&inputs, None, outcome);
    };

    let request = build_request(spec, expectations, &images, selected_mode);
    let started = Instant::now();
    let call =
        tokio::time::timeout(REQUEST_TIMEOUT, gateway.extract_student_answer_ocr(request)).await;
    let duration_ms = Some(started.elapsed().as_millis() as u64);

    match call {
        Ok(Ok(result)) => {
            let raw = result.raw_response.clone();
            std::fs::write(question_out.join("response_raw.txt"), &raw).expect("save raw response");
            std::fs::write(
                question_out.join("extracted.json"),
                serde_json::to_string_pretty(&result.output).expect("serialize output"),
            )
            .expect("save extracted output");
            let outcome = build_outcome(None, duration_ms, images_meta);
            compute_metrics(&inputs, Some(&result), outcome)
        }
        Ok(Err(error)) => {
            let message = format!("model_call_error:{:?}:{}", error.code, error.message);
            let outcome = build_outcome(Some(message), duration_ms, images_meta);
            compute_metrics(&inputs, None, outcome)
        }
        Err(_) => {
            let outcome = build_outcome(
                Some("model_call_timeout".to_string()),
                duration_ms,
                images_meta,
            );
            compute_metrics(&inputs, None, outcome)
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let golden_dir = cli.golden_dir.clone().unwrap_or_else(golden_dir_from_env);
    let expectations = read_json(&golden_dir.join(EXPECTATIONS_JSON));
    let rubric = read_json(&golden_dir.join(RUBRIC_JSON));
    let scanned_pdf = golden_dir.join(&cli.pdf);

    let outdir = if cli.outdir.exists() {
        std::fs::canonicalize(&cli.outdir).expect("canonicalize outdir")
    } else {
        std::fs::create_dir_all(&cli.outdir).expect("create outdir");
        std::fs::canonicalize(&cli.outdir).expect("canonicalize outdir")
    };
    for sub in ["pages", "crops", "results"] {
        std::fs::create_dir_all(outdir.join(sub)).expect("create outdir subdir");
    }

    // Render at the requested DPI.
    let pages = render_pdf_pages(&scanned_pdf, cli.dpi, &outdir.join("pages"));

    // Deskew every rendered page (production deskew) + registration measurement.
    let mut deskewed_pages = BTreeMap::new();
    let mut page_skew = Vec::new();
    let mut registration = Vec::new();
    for (index, path) in pages.iter().enumerate() {
        let page_number = index as u32 + 1;
        let image = image::open(path).expect("open rendered page");
        let deskewed = deskew_image(&image, DESKEW_DEFAULT_MAX_ANGLE)
            .unwrap_or_else(|e| panic!("page {page_number} deskew failed: {}", e.message));
        page_skew.push((page_number, deskewed.angle_degrees));
        let deskewed_path = outdir
            .join("pages")
            .join(format!("page-{page_number}.deskewed.png"));
        save_image(&deskewed.image, &deskewed_path);
        deskewed_pages.insert(page_number, deskewed_path);

        let regions = regions_for_page(&expectations, page_number);
        if !regions.is_empty() {
            let measurement =
                measure_registration_deviation(&image.grayscale().to_luma8(), &regions);
            registration.push((page_number, measurement));
        }
    }

    let manifest_sha256 = sha256_hex(&golden_dir.join(MANIFEST_JSON));
    let generated_at = chrono::Utc::now().to_rfc3339();

    let gateway = if cli.skip_model {
        None
    } else {
        let gateway = LlamaServerGateway::new(cli.base_url.clone());
        match gateway.get_status().await {
            Ok(status) if status.server_running => Some(gateway),
            Ok(status) => {
                eprintln!(
                    "model server not running (health_ok={}); report-only",
                    status.health_ok
                );
                None
            }
            Err(error) => {
                eprintln!(
                    "model server unavailable: {} ({:?})",
                    error.message, error.code
                );
                None
            }
        }
    };

    let specs = QuestionSpec::all();
    let mut per_question = Vec::new();
    let mut qruns = Vec::new();
    for spec in &specs {
        let run = run_question(
            spec,
            &expectations,
            &rubric,
            &deskewed_pages,
            &outdir,
            gateway.as_ref(),
        )
        .await;
        qruns.push(run.clone());

        let mut metric = GoldenQuestionMetric {
            question_id: spec.id.clone(),
            answer_type: answer_type_label(&spec.answer_type).to_string(),
            pages: run.pages.clone(),
            region_count: run.region_count,
            cer: run.cer,
            wer: run.wer,
            critical_token_missing: run.critical_missing,
            printed_question_leakage: run.leakage.or(run.printed_question_leak_detected),
            structured_exact_match: run.structured_exact,
            duration_ms_p50: run.duration_ms,
            duration_ms_p95: run.duration_ms,
            image_token_count: None,
            model_call_count: if cli.skip_model { None } else { Some(1) },
            retry_count: Some(0),
            peak_memory_bytes: None,
        };
        if !run.ok {
            metric.cer = None;
            metric.wer = None;
            metric.critical_token_missing = None;
            metric.printed_question_leakage = None;
            metric.structured_exact_match = None;
        }
        per_question.push(metric);

        println!(
            "[{}] answer_type={} pages={:?} ok={} cer={:?} wer={:?} exact={:?} leak={:?} critical_missing={:?} duration_ms={:?}",
            spec.id,
            answer_type_label(&spec.answer_type),
            run.pages,
            run.ok,
            run.cer,
            run.wer,
            run.structured_exact,
            run.leakage,
            run.critical_missing,
            run.duration_ms,
        );
        if let Some(error) = &run.error {
            eprintln!("  error: {error}");
        }
    }

    let cers = qruns.iter().filter_map(|r| r.cer).collect::<Vec<_>>();
    let wers = qruns.iter().filter_map(|r| r.wer).collect::<Vec<_>>();
    let aggregate = GoldenAggregateMetric {
        cer_p50: percentile_p50(&cers),
        cer_p95: percentile_p95(&cers),
        wer_p50: percentile_p50(&wers),
        wer_p95: percentile_p95(&wers),
        total_image_tokens: None,
        total_model_calls: if cli.skip_model {
            None
        } else {
            Some(per_question.len() as u64)
        },
        total_retries: Some(0),
        peak_memory_bytes: None,
    };

    let report = GoldenOcrBenchmarkReport {
        schema_version: REPORT_SCHEMA_VERSION.to_string(),
        exam_id: EXAM_ID.to_string(),
        generated_at: generated_at.clone(),
        model_runtime: if cli.skip_model || gateway.is_none() {
            GoldenModelRuntimeStatus::NeedsModelRuntime
        } else {
            GoldenModelRuntimeStatus::Available
        },
        corpus_manifest_sha256: Some(manifest_sha256.clone()),
        per_question,
        aggregate,
    };

    let report_json = serde_json::to_string_pretty(&report).expect("serialize report");
    std::fs::write(outdir.join("benchmark_report.json"), &report_json).expect("write report");
    let details = json!({
        "schemaVersion": REPORT_SCHEMA_VERSION,
        "examId": EXAM_ID,
        "generatedAt": generated_at,
        "dpi": cli.dpi,
        "baseUrl": cli.base_url,
        "pdf": cli.pdf,
        "pageSkewDegrees": page_skew,
        "registrationDeviation": registration
            .iter()
            .map(|(page, m)| json!({
                "page": page,
                "deviation": m.deviation,
                "maxDeviation": m.max_deviation,
                "sampledRegions": m.sampled_regions,
                "totalRegions": m.total_regions,
            }))
            .collect::<Vec<_>>(),
        "questions": qruns,
    });
    std::fs::write(
        outdir.join("benchmark_details.json"),
        serde_json::to_string_pretty(&details).expect("serialize details"),
    )
    .expect("write details");

    println!("\nmanifest_sha256: {manifest_sha256}");
    println!("report: {}", outdir.join("benchmark_report.json").display());
    println!(
        "details: {}",
        outdir.join("benchmark_details.json").display()
    );
}
