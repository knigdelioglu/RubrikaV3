//! Committed golden corpus integration test — `tymm_tde_001`.
//!
//! Model-independent validation of the synthetic golden exam package under
//! `testdata/golden/tymm_tde_001/`. These tests are read-only against the
//! golden files: PDFs and JSON contracts are only read, rendered output is
//! written to a tempdir and cleaned up best-effort.
//!
//! Model benchmark gates (CER/WER/leakage measured on real OCR output) are
//! exercised with `NEEDS_MODEL_RUNTIME` because no model binary exists in the
//! test environment; see `docs/GOLDEN_OCR_SCORING_BENCHMARK.md`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use image::GenericImageView;

use app_lib::domain::question::AnswerType;
use app_lib::domain::structured_answer::{
    validate_for_answer_type, CorrectionTableRow, GrammarAnalysisItem, MatchingPair,
    StructuredAnswer, StructuredTableRow,
};
use app_lib::domain::student::{
    AnswerRegionRole, ContinuationPolicy, NormalizedBBox, QuestionAnswerRegion,
    QuestionAnswerTemplate,
};
use app_lib::services::deterministic_scoring_service::DeterministicScoringService;
use app_lib::services::golden_ocr_metrics::{
    character_error_rate, corpus_bbox_bottom_left_y_to_top_left, printed_question_leakage,
    structured_field_exact_match, structured_fields_all_exact, word_error_rate,
    GoldenModelRuntimeStatus, GoldenOcrBenchmarkReport,
};
use app_lib::services::ocr_image_geometry_service::{
    deskew_image, measure_registration_deviation, normalize_dpi, render_scale_to_dpi,
    validate_dpi_in_range, DEFAULT_MAX_REGISTRATION_DEVIATION, DESKEW_DEFAULT_MAX_ANGLE,
    DESKEW_MAX_ACCEPTED_ANGLE, OCR_MAX_ACCEPTED_DPI, OCR_MIN_ACCEPTED_DPI, OCR_RENDER_TARGET_DPI,
};
use app_lib::services::pdf_service::{PdfService, SystemPdfService};
use app_lib::services::student_answer_crop_service::crop_rect_normalized;
use serde_json::Value;

const GOLDEN_DIR: &str = "../testdata/golden/tymm_tde_001";
const EXAM_ID: &str = "tymm_tde_001";
const EXPECTED_EXAM_PAGE_COUNT: u32 = 4;

fn golden_dir() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join(GOLDEN_DIR)
}

fn golden_file(name: &str) -> PathBuf {
    golden_dir().join(name)
}

fn load_golden_json(name: &str) -> Value {
    let path = golden_file(name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read golden JSON {}: {e}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse golden JSON {}: {e}", path.display()))
}

fn sha256_hex(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).expect("read file for hashing");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    hex::encode(hasher.finalize())
}

fn render_pages_by_number(
    service: &SystemPdfService,
    pdf: &Path,
    tempdir: &Path,
) -> BTreeMap<u32, PathBuf> {
    let rendered = service
        .render_all_pages(pdf, tempdir)
        .expect("render all pages of golden PDF");
    rendered
        .into_iter()
        .map(|path| {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("rendered file stem");
            let page = stem
                .rsplit(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|d| d.parse::<u32>().ok())
                .expect("rendered page number");
            (page, path)
        })
        .collect()
}

/// Test corpus helper: build a `QuestionAnswerRegion` from a golden region
/// entry (page is 1-indexed; page_offset is 0-indexed within the submission).
///
/// The golden 06 bboxes are stored in PDF bottom-left y space; the domain
/// `NormalizedBBox`/`crop_rect_normalized` convention is top-left. The shared
/// conversion `corpus_bbox_bottom_left_y_to_top_left` is applied here so every
/// golden crop consumer feeds the production crop math the correct coordinates.
fn region_from_golden(question_id: &str, order: u32, entry: &Value) -> QuestionAnswerRegion {
    let page = entry["page"].as_u64().expect("region page") as u32;
    let bbox = entry["bbox_normalized"]
        .as_array()
        .expect("region bbox array");
    let role = match entry["role"].as_str().unwrap_or("primary") {
        "primary" => AnswerRegionRole::Primary,
        "continuation" => AnswerRegionRole::Continuation,
        _ => AnswerRegionRole::Supporting,
    };
    let continuation_policy = if role == AnswerRegionRole::Continuation {
        ContinuationPolicy::ContinuesPrevious
    } else {
        ContinuationPolicy::Independent
    };
    QuestionAnswerRegion {
        region_id: format!("{question_id}-region-{order}"),
        page_offset: page.saturating_sub(1),
        order,
        normalized_bbox: NormalizedBBox {
            x: bbox[0].as_f64().expect("bbox x") as f32,
            y: corpus_bbox_bottom_left_y_to_top_left(
                bbox[1].as_f64().expect("bbox y") as f32,
                bbox[3].as_f64().expect("bbox h") as f32,
            ),
            width: bbox[2].as_f64().expect("bbox w") as f32,
            height: bbox[3].as_f64().expect("bbox h") as f32,
        },
        region_role: role,
        continuation_policy,
        label: None,
        note: None,
    }
}

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rubrika-golden-tymm-tde-001-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

/// Normalized bboxes of the golden answer regions that live on `page`
/// (1-indexed), in document order. Bboxes are converted from the golden
/// bottom-left y convention to the production top-left convention.
fn regions_for_page(expectations: &Value, page: u32) -> Vec<NormalizedBBox> {
    let mut regions = Vec::new();
    for entries in expectations["regions"]
        .as_object()
        .expect("regions map")
        .values()
    {
        for entry in entries.as_array().expect("region entries") {
            if entry["page"].as_u64().expect("region page") != page as u64 {
                continue;
            }
            let bbox = entry["bbox_normalized"].as_array().expect("bbox array");
            regions.push(NormalizedBBox {
                x: bbox[0].as_f64().expect("x") as f32,
                y: corpus_bbox_bottom_left_y_to_top_left(
                    bbox[1].as_f64().expect("y") as f32,
                    bbox[3].as_f64().expect("h") as f32,
                ),
                width: bbox[2].as_f64().expect("w") as f32,
                height: bbox[3].as_f64().expect("h") as f32,
            });
        }
    }
    regions
}

#[test]
fn manifest_sha256_verifies_all_golden_files() {
    let manifest_path = golden_file("manifest.sha256");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("missing committed manifest: {e}"));
    let expected_files = [
        "01_Bos_Sinav_Kagidi.pdf",
        "02_Doldurulmus_Ornek_Kagit.pdf",
        "03_Doldurulmus_Tarama_Varyanti.pdf",
        "04_Cevap_Anahtari_ve_Rubrik.pdf",
        "05_Rubrik_Golden.json",
        "06_Golden_Set_Beklentileri.json",
        "07_CodeX_Teknik_Borc_Kapanis_Promptu.md",
        "README.md",
    ];
    let mut verified = BTreeMap::new();
    for line in manifest.lines().filter(|line| !line.trim().is_empty()) {
        let mut parts = line.split_whitespace();
        let hash = parts.next().expect("manifest hash");
        let name = parts.next().expect("manifest filename");
        let file = golden_file(name);
        let actual = sha256_hex(&file);
        assert_eq!(
            actual,
            hash,
            "golden file {} does not match manifest hash",
            file.display()
        );
        verified.insert(name.to_string(), true);
    }
    for name in expected_files {
        assert!(
            verified.contains_key(name),
            "golden file {name} is missing from the committed manifest"
        );
        assert!(
            golden_file(name).is_file(),
            "golden file {name} is missing from the corpus directory"
        );
    }
}

#[test]
fn golden_contracts_parse_and_expected_score_is_consistent() {
    let expectations = load_golden_json("06_Golden_Set_Beklentileri.json");
    assert_eq!(
        expectations["schema_version"],
        "rubrika.synthetic.golden.v1"
    );
    assert_eq!(expectations["exam_id"], EXAM_ID);
    assert_eq!(expectations["synthetic"], Value::Bool(true));
    assert_eq!(
        expectations["page_count"].as_u64(),
        Some(EXPECTED_EXAM_PAGE_COUNT as u64)
    );

    let rubric = load_golden_json("05_Rubrik_Golden.json");
    assert_eq!(rubric["exam_id"], EXAM_ID);
    assert_eq!(rubric["max_score"].as_u64(), Some(100));

    let mut rubric_sum: u64 = 0;
    for question in rubric["questions"].as_array().expect("rubric questions") {
        rubric_sum += question["max_points"].as_u64().expect("max_points");
    }
    assert_eq!(
        rubric_sum, 100,
        "rubric per-question points must total max_score"
    );

    let scoring = &expectations["expected_scoring"];
    let mut expected_sum: u64 = 0;
    for key in ["q1", "q2", "q3", "q4", "q5", "q6"] {
        expected_sum += scoring[key].as_u64().expect("expected per-question score");
    }
    assert_eq!(
        expected_sum, 80,
        "per-question expected scores must total 80"
    );
    assert_eq!(scoring["total"].as_u64(), Some(80));
    assert_eq!(scoring["decision_state"], "teacher_approved_golden");
}

#[test]
fn blank_exam_is_renderable_and_has_four_pages() {
    let pdf = golden_file("01_Bos_Sinav_Kagidi.pdf");
    assert!(pdf.is_file(), "blank exam PDF missing");
    let service = SystemPdfService;
    let status = service.get_renderer_status().expect("renderer status");
    assert!(
        status.available,
        "PDF renderer unavailable; cannot validate golden renders"
    );
    let count = service.page_count(&pdf).expect("page count of blank exam");
    assert_eq!(count, EXPECTED_EXAM_PAGE_COUNT);
    let dir = tempdir();
    let pages = render_pages_by_number(&service, &pdf, &dir);
    assert_eq!(pages.len(), EXPECTED_EXAM_PAGE_COUNT as usize);
    assert_eq!(pages.keys().copied().collect::<Vec<_>>(), vec![1, 2, 3, 4]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn filled_exam_regions_crop_within_bounds() {
    let pdf = golden_file("02_Doldurulmus_Ornek_Kagit.pdf");
    assert!(pdf.is_file(), "filled exam PDF missing");
    let service = SystemPdfService;
    let status = service.get_renderer_status().expect("renderer status");
    assert!(status.available);
    let expectations = load_golden_json("06_Golden_Set_Beklentileri.json");
    let regions = expectations["regions"].as_object().expect("regions map");

    let dir = tempdir();
    let pages = render_pages_by_number(&service, &pdf, &dir);
    for (question_id, entries) in regions {
        for entry in entries.as_array().expect("region entries") {
            let page = entry["page"].as_u64().expect("region page") as u32;
            let preview = pages
                .get(&page)
                .unwrap_or_else(|| panic!("no render for page {page}"));
            let image = image::open(preview).expect("open rendered page");
            let (width, height) = image.dimensions();
            let bbox = entry["bbox_normalized"].as_array().expect("bbox array");
            let bbox = NormalizedBBox {
                x: bbox[0].as_f64().expect("x") as f32,
                y: corpus_bbox_bottom_left_y_to_top_left(
                    bbox[1].as_f64().expect("y") as f32,
                    bbox[3].as_f64().expect("h") as f32,
                ),
                width: bbox[2].as_f64().expect("w") as f32,
                height: bbox[3].as_f64().expect("h") as f32,
            };
            for value in [bbox.x, bbox.y, bbox.width, bbox.height] {
                assert!(
                    (0.0..=1.0).contains(&value),
                    "golden bbox for {question_id} out of [0,1]: {value}"
                );
            }
            let (x, y, w, h, clamped, _margin) = crop_rect_normalized(&bbox, width, height);
            assert!(w > 0 && h > 0, "empty crop for {question_id}");
            assert!(
                x + w <= width && y + h <= height,
                "crop for {question_id} exceeds page bounds"
            );
            let cropped = image.crop_imm(x, y, w, h);
            assert_eq!(cropped.dimensions(), (w, h));
            assert!(
                w >= 64 && h >= 64,
                "filled vector crop for {question_id} too small to OCR ({w}x{h}); clamped={clamped}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scanned_variant_is_valid_and_renderable_with_bounded_crops() {
    let pdf = golden_file("03_Doldurulmus_Tarama_Varyanti.pdf");
    assert!(pdf.is_file(), "scanned variant PDF missing");
    let service = SystemPdfService;
    let status = service.get_renderer_status().expect("renderer status");
    assert!(status.available);
    let count = service
        .page_count(&pdf)
        .expect("page count of scanned variant");
    assert_eq!(count, EXPECTED_EXAM_PAGE_COUNT);
    let dir = tempdir();
    let pages = render_pages_by_number(&service, &pdf, &dir);
    assert_eq!(pages.len(), EXPECTED_EXAM_PAGE_COUNT as usize);
    // The scanned variant is skewed/contrast-degraded (deskew is Faz 7). This
    // phase only proves the pipeline accepts the input and that the golden
    // bboxes remain inside the rendered page bounds.
    let expectations = load_golden_json("06_Golden_Set_Beklentileri.json");
    for (question_id, entries) in expectations["regions"].as_object().expect("regions") {
        for entry in entries.as_array().expect("entries") {
            let page = entry["page"].as_u64().expect("page") as u32;
            let preview = pages
                .get(&page)
                .unwrap_or_else(|| panic!("no render for page {page}"));
            let image = image::open(preview).expect("open scanned page");
            let (width, height) = image.dimensions();
            let bbox = entry["bbox_normalized"].as_array().expect("bbox");
            let bbox = NormalizedBBox {
                x: bbox[0].as_f64().expect("x") as f32,
                y: corpus_bbox_bottom_left_y_to_top_left(
                    bbox[1].as_f64().expect("y") as f32,
                    bbox[3].as_f64().expect("h") as f32,
                ),
                width: bbox[2].as_f64().expect("w") as f32,
                height: bbox[3].as_f64().expect("h") as f32,
            };
            let (x, y, w, h, _clamped, _margin) = crop_rect_normalized(&bbox, width, height);
            assert!(
                x + w <= width && y + h <= height && w > 0 && h > 0,
                "scanned crop for {question_id} out of bounds"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn corpus_bboxes_are_converted_to_top_left_before_cropping() {
    // Faz 9 regression: golden 06 bboxes use PDF bottom-left y origin; feeding
    // them into `crop_rect_normalized` unconverted crops the wrong region (only
    // Q5 happens to align). Every golden consumer must apply the shared
    // `corpus_bbox_bottom_left_y_to_top_left` conversion. This locks that
    // behavior for the `region_from_golden` / `regions_for_page` helpers.
    let expectations = load_golden_json("06_Golden_Set_Beklentileri.json");
    let mut converted_count = 0;
    let mut total = 0;
    for (question_id, entries) in expectations["regions"].as_object().expect("regions") {
        for (order, entry) in entries.as_array().expect("entries").iter().enumerate() {
            let bbox = entry["bbox_normalized"].as_array().expect("bbox array");
            let y_raw = bbox[1].as_f64().expect("y") as f32;
            let height = bbox[3].as_f64().expect("h") as f32;
            let region = region_from_golden(question_id, order as u32, entry);
            assert_eq!(
                region.normalized_bbox.y,
                corpus_bbox_bottom_left_y_to_top_left(y_raw, height),
                "{question_id} region {order} y must be converted to top-left"
            );
            total += 1;
            if (y_top_raw_differs(y_raw, height)).abs() > 1e-4 {
                converted_count += 1;
            }
        }
    }
    assert_eq!(
        converted_count, total,
        "every golden region bbox must require y-axis conversion"
    );
}

/// `y_top - y_raw` for a bottom-left→top-left conversion; kept as a helper so
/// the regression test asserts the conversion is never the identity here.
fn y_top_raw_differs(y_bottom: f32, height: f32) -> f32 {
    corpus_bbox_bottom_left_y_to_top_left(y_bottom, height) - y_bottom
}

#[test]
fn q1_has_primary_and_continuation_regions_in_document_order() {
    let expectations = load_golden_json("06_Golden_Set_Beklentileri.json");
    let q1 = expectations["regions"]["q1"]
        .as_array()
        .expect("q1 regions array");
    assert_eq!(q1.len(), 2, "Q1 must span exactly two regions");

    let template = QuestionAnswerTemplate {
        question_id: "q1".to_string(),
        regions: q1
            .iter()
            .enumerate()
            .map(|(order, entry)| region_from_golden("q1", order as u32, entry))
            .collect(),
    };
    let mut normalized = template.clone();
    normalized.normalize_order();
    let sorted = normalized.sorted_regions();
    assert_eq!(sorted.len(), 2);
    assert_eq!(sorted[0].region_role, AnswerRegionRole::Primary);
    assert_eq!(sorted[0].page_offset, 0);
    assert_eq!(sorted[1].region_role, AnswerRegionRole::Continuation);
    assert_eq!(sorted[1].page_offset, 1);
    assert_eq!(
        sorted[1].continuation_policy,
        ContinuationPolicy::ContinuesPrevious
    );
    assert!(sorted[0].order < sorted[1].order);
    for region in &sorted {
        for value in [
            region.normalized_bbox.x,
            region.normalized_bbox.y,
            region.normalized_bbox.width,
            region.normalized_bbox.height,
        ] {
            assert!((0.0..=1.0).contains(&value));
        }
    }
}

#[test]
fn golden_answer_types_match_structured_answer_variants() {
    let rubric = load_golden_json("05_Rubrik_Golden.json");
    let questions = rubric["questions"].as_array().expect("questions");

    let expected = [
        (
            "q1",
            "open_ended",
            AnswerType::Essay,
            StructuredAnswer::OpenText {
                text: "cevap".into(),
            },
        ),
        (
            "q2",
            "table",
            AnswerType::Table,
            StructuredAnswer::Table {
                rows: vec![StructuredTableRow {
                    index: 0,
                    cells: vec!["değer".into()],
                }],
            },
        ),
        (
            "q3",
            "matching",
            AnswerType::Matching,
            StructuredAnswer::Matching {
                pairs: vec![MatchingPair {
                    left: "1".into(),
                    right: "A".into(),
                }],
            },
        ),
        (
            "q4",
            "correction_table",
            AnswerType::CorrectionTable,
            StructuredAnswer::CorrectionTable {
                rows: vec![CorrectionTableRow {
                    index: 0,
                    original: "yanlış".into(),
                    correction: "doğru".into(),
                    explanation: None,
                }],
            },
        ),
        (
            "q5",
            "grammar_analysis",
            AnswerType::GrammarAnalysis,
            StructuredAnswer::GrammarAnalysis {
                items: vec![GrammarAnalysisItem {
                    text: "Ece".into(),
                    label: "Özne".into(),
                    explanation: None,
                }],
            },
        ),
        (
            "q6",
            "open_ended",
            AnswerType::Essay,
            StructuredAnswer::OpenText {
                text: "yorum".into(),
            },
        ),
    ];

    assert_eq!(questions.len(), expected.len());
    for (index, question) in questions.iter().enumerate() {
        let id = question["question_id"].as_str().expect("question_id");
        let golden_type = question["answer_type"].as_str().expect("answer_type");
        let (expected_id, expected_golden_type, answer_type, sample) = &expected[index];
        assert_eq!(id, *expected_id);
        assert_eq!(golden_type, *expected_golden_type);

        // The golden answer type must be accepted by the domain validator for
        // the expected typed StructuredAnswer variant.
        validate_for_answer_type(answer_type, sample)
            .unwrap_or_else(|e| panic!("golden {id} answer type rejected: {}", e.message));

        // A mismatched variant must be rejected (fails closed).
        let wrong = StructuredAnswer::Numeric {
            value: Some("7".into()),
            unit: None,
        };
        let result = validate_for_answer_type(answer_type, &wrong);
        assert!(
            result.is_err(),
            "golden {id} must fail closed for a mismatched StructuredAnswer variant"
        );
    }
}

#[test]
fn golden_deterministic_questions_are_covered_by_the_deterministic_scorer() {
    // TD-37 coverage decision lock: the golden rubric flags exactly the
    // objectively-gradeable questions as `deterministic: true`. Every such
    // question's answer type must be covered by the deterministic scorer,
    // and the criterion/open-ended ones must remain on the semantic path.
    let rubric = load_golden_json("05_Rubrik_Golden.json");
    let questions = rubric["questions"].as_array().expect("questions");

    let expected = [
        ("q1", "open_ended", AnswerType::Essay, false),
        ("q2", "table", AnswerType::Table, true),
        ("q3", "matching", AnswerType::Matching, true),
        ("q4", "correction_table", AnswerType::CorrectionTable, true),
        ("q5", "grammar_analysis", AnswerType::GrammarAnalysis, false),
        ("q6", "open_ended", AnswerType::Essay, false),
    ];
    assert_eq!(questions.len(), expected.len());
    for (index, question) in questions.iter().enumerate() {
        let id = question["question_id"].as_str().expect("question_id");
        let flagged = question["deterministic"].as_bool().unwrap_or(false);
        let (expected_id, _, answer_type, expected_supported) = &expected[index];
        assert_eq!(id, *expected_id);
        assert_eq!(flagged, *expected_supported, "{id} deterministic flag");
        assert_eq!(
            DeterministicScoringService::supports(answer_type),
            *expected_supported,
            "{id} deterministic coverage must match the golden flag"
        );
    }
}

#[test]
fn metric_functions_are_clean_against_golden_ground_truth() {
    let expectations = load_golden_json("06_Golden_Set_Beklentileri.json");
    let ground_truth = &expectations["ocr_ground_truth"];

    // CER/WER of an identical hypothesis is exactly zero.
    for key in ["q1", "q3", "q5", "q6"] {
        let text = ground_truth[key].as_str().expect("ground truth text");
        assert_eq!(character_error_rate(text, text), 0.0);
        assert_eq!(word_error_rate(text, text), 0.0);
    }

    // Q2 structured table fields must all match exactly (1.0 quality gate).
    let q2 = ground_truth["q2"].as_object().expect("q2 object");
    let expected_fields = q2.keys().cloned().collect::<Vec<_>>();
    let actual_values = q2.values().filter_map(Value::as_str).collect::<Vec<_>>();
    assert_eq!(expected_fields.len(), 5);
    for value in &actual_values {
        assert!(!value.trim().is_empty());
    }
    assert!(structured_field_exact_match(
        "Üçüncü kişi anlatıcı",
        "ÜÇÜNCÜ KİŞİ ANLATICI"
    ));

    // Q3 matching answer key is deterministic (05 rubric).
    let rubric = load_golden_json("05_Rubrik_Golden.json");
    let q3 = rubric["questions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["question_id"] == "q3")
        .expect("q3 rubric");
    let answer_key = q3["answer_key"].as_object().expect("q3 answer_key");
    assert_eq!(answer_key.len(), 5);
    assert_eq!(answer_key["1"], "A");
    assert_eq!(answer_key["4"], "D");
    // Ground truth uses the "1-A, 2-B, ..." sequence format. NOTE: the golden
    // package has a known internal inconsistency at item 5 (rubric `5:E` vs
    // ground truth `5-A`); it is reported verbatim and not "fixed" here.
    let golden_q3 = ground_truth["q3"].as_str().expect("q3 ground truth");
    assert_eq!(golden_q3, "1-A, 2-B, 3-C, 4-D, 5-A");
    for (pair_index, expected_letter) in ["A", "B", "C", "D"].iter().enumerate() {
        let pair = golden_q3.split(", ").nth(pair_index).expect("q3 pair");
        assert_eq!(pair, format!("{}-{expected_letter}", pair_index + 1));
    }
    // The rubric key agrees with the ground truth on items 1-4 and diverges on
    // item 5 (rubric=E vs ground truth=A); the divergence is part of the
    // golden corpus record, so we assert the observed values, not agreement.
    assert_eq!(answer_key["5"], "E");
    assert!(!golden_q3.contains("5-E"));

    // structured_fields_all_exact round-trips the q2 field names against
    // themselves.
    let same = expected_fields
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert!(structured_fields_all_exact(&same, &same));
    assert!(!structured_fields_all_exact(&same, &same[..4]));

    // The golden answer text must not contain the printed question prompt
    // (leakage gate: printed_question_leakage_allowed = false). The printed
    // instruction for Q6 is "Şiirin baskın duygusunu belirleyiniz. Bir dizeyi
    // kanıt göstererek yorumlayınız." — the answer may quote the *poem* line,
    // but it must not reproduce the printed instruction.
    let q6 = ground_truth["q6"].as_str().expect("q6 ground truth");
    let printed_hint =
        "Şiirin baskın duygusunu belirleyiniz. Bir dizeyi kanıt göstererek yorumlayınız.";
    let leak = printed_question_leakage(q6, printed_hint);
    assert!(
        !leak.leaked,
        "golden q6 answer leaks printed question text: {:?}",
        leak.overlap_words
    );
}

#[test]
fn benchmark_report_dto_documents_needs_model_runtime() {
    let report = GoldenOcrBenchmarkReport {
        schema_version: "rubrika.golden.ocr.benchmark.v1".to_string(),
        exam_id: EXAM_ID.to_string(),
        generated_at: "2026-08-06T00:00:00Z".to_string(),
        model_runtime: GoldenModelRuntimeStatus::NeedsModelRuntime,
        corpus_manifest_sha256: Some(sha256_hex(&golden_file("manifest.sha256"))),
        per_question: Vec::new(),
        aggregate: app_lib::services::golden_ocr_metrics::GoldenAggregateMetric {
            cer_p50: None,
            cer_p95: None,
            wer_p50: None,
            wer_p95: None,
            total_image_tokens: None,
            total_model_calls: None,
            total_retries: None,
            peak_memory_bytes: None,
        },
    };
    let json = serde_json::to_string(&report).expect("serialize report");
    assert!(json.contains("\"modelRuntime\":\"needs_model_runtime\""));
    assert!(json.contains("corpusManifestSha256"));
    assert!(json.contains(EXAM_ID));
}

#[test]
fn scanned_variant_deskew_accepts_every_page_within_golden_bounds() {
    let pdf = golden_file("03_Doldurulmus_Tarama_Varyanti.pdf");
    let service = SystemPdfService;
    let status = service.get_renderer_status().expect("renderer status");
    assert!(status.available);
    let dir = tempdir();
    let pages = render_pages_by_number(&service, &pdf, &dir);

    let mut accepted_pages = 0;
    for (page_number, path) in &pages {
        let image = image::open(path).expect("open scanned page");
        let result = deskew_image(&image, DESKEW_DEFAULT_MAX_ANGLE).unwrap_or_else(|error| {
            panic!(
                "golden 03 page {page_number} rejected by deskew: {}",
                error.message
            )
        });
        assert!(
            result.angle_degrees.abs() < DESKEW_MAX_ACCEPTED_ANGLE,
            "golden page {page_number} skew {} outside accepted range",
            result.angle_degrees
        );
        accepted_pages += 1;
    }
    assert_eq!(accepted_pages, EXPECTED_EXAM_PAGE_COUNT as usize);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scanned_variant_registration_deviation_stays_within_golden_bounds() {
    let pdf = golden_file("03_Doldurulmus_Tarama_Varyanti.pdf");
    let service = SystemPdfService;
    let status = service.get_renderer_status().expect("renderer status");
    assert!(status.available);
    let expectations = load_golden_json("06_Golden_Set_Beklentileri.json");
    let dir = tempdir();
    let pages = render_pages_by_number(&service, &pdf, &dir);

    let mut measured_pages = 0;
    for (page_number, path) in &pages {
        let regions = regions_for_page(&expectations, *page_number);
        if regions.is_empty() {
            continue;
        }
        let image = image::open(path)
            .expect("open scanned page")
            .grayscale()
            .to_luma8();
        let measurement = measure_registration_deviation(&image, &regions);
        measured_pages += 1;
        // The scanned variant must satisfy the production registration
        // threshold (the same gate `validate_registration` enforces).
        assert!(
            measurement.deviation < DEFAULT_MAX_REGISTRATION_DEVIATION,
            "golden 03 page {page_number} exceeds production registration bound {}: {}",
            DEFAULT_MAX_REGISTRATION_DEVIATION,
            measurement.deviation
        );
    }
    assert!(
        measured_pages >= 1,
        "no golden pages carried answer regions"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn golden_render_dpi_normalizes_to_fixed_ocr_target() {
    let pdf = golden_file("03_Doldurulmus_Tarama_Varyanti.pdf");
    let service = SystemPdfService;
    let status = service.get_renderer_status().expect("renderer status");
    assert!(status.available);
    let dir = tempdir();
    let pages = render_pages_by_number(&service, &pdf, &dir);
    let first = pages.values().next().expect("at least one rendered page");
    let image = image::open(first).expect("open rendered page");
    let (width, height) = image.dimensions();

    // The preview pipeline renders at scale 2.0 (~144 DPI). The pure DPI
    // normalization must validate that source and compute the 300 DPI resize.
    let source_dpi = render_scale_to_dpi(2.0);
    assert_eq!(source_dpi, 144);
    assert!(validate_dpi_in_range(
        source_dpi,
        OCR_MIN_ACCEPTED_DPI,
        OCR_MAX_ACCEPTED_DPI
    ));
    let normalization = normalize_dpi(source_dpi, OCR_RENDER_TARGET_DPI, width, height);
    assert!(normalization.adjusted);
    assert_eq!(normalization.target_dpi, OCR_RENDER_TARGET_DPI);
    assert!(normalization.output_width > width);
    assert!(normalization.output_height > height);
    assert!(!validate_dpi_in_range(
        30,
        OCR_MIN_ACCEPTED_DPI,
        OCR_MAX_ACCEPTED_DPI
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
