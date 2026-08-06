//! Golden OCR scoring benchmark metrics.
//!
//! Pure, model-independent measurement helpers for the committed golden
//! corpus (`testdata/golden/tymm_tde_001/`). These functions never touch the
//! model, the filesystem or persisted project data; they only turn already
//! produced OCR text and the golden ground truth into comparable metrics.
//!
//! The report DTO ([`GoldenOcrBenchmarkReport`]) is the documented measurement
//! contract for Faz 7+: the benchmark runner fills the model-dependent fields
//! (`cer`, `wer`, latency percentiles, image tokens, calls, retries, peak
//! memory) and writes the report without ever mutating the golden files.
//!
//! Normalized comparison is handled by [`crate::services::text_normalization::normalize_for_comparison`]
//! (Turkish case folding, punctuation-to-space, whitespace collapse). This
//! module never replaces persisted OCR text.

use serde::{Deserialize, Serialize};

use crate::services::text_normalization::normalize_for_comparison;

/// Minimum number of overlapping printed-question words for a leakage report
/// to be considered a real leak (guards against single common-word matches).
pub const LEAKAGE_MIN_OVERLAP_WORDS: usize = 2;
/// Minimum overlap ratio (overlap / printed content words) for leakage.
pub const LEAKAGE_MIN_OVERLAP_RATIO: f64 = 0.5;

/// Golden corpus `06` `bbox_normalized` y koordinat dönüşümü.
///
/// `06_Golden_Set_Beklentileri.json` bbox'ları PDF kullanıcı uzayında saklanır
/// (y ekseni aşağıdan yukarı büyür: değer `y_bottom`'dur). Üretim crop
/// matematiği (`crop_rect_normalized`) ve `NormalizedBBox` sözleşmesi üst-sol
/// kaynaklı y bekler (yukarıdan aşağı büyür). Dönüşüm `y_top = 1 - (y_bottom +
/// height)` ile alt-sol konvansiyonunu üst-sol'a çevirir ve [0,1] aralığına
/// kenetler. Golden dosyaları değiştirilmez; dönüşüm yalnız bbox'ları tüketen
/// kodda (benchmark runner ve golden entegrasyon testi) uygulanır.
pub fn corpus_bbox_bottom_left_y_to_top_left(y_bottom: f32, height: f32) -> f32 {
    (1.0 - (y_bottom + height)).clamp(0.0, 1.0)
}

/// A small Turkish function-word set excluded from printed-question leakage
/// overlap so "bir", "ve", "de" etc. do not trigger false leaks.
const TURKISH_STOPWORDS: &[&str] = &[
    "bir", "ve", "ile", "için", "de", "da", "mi", "mu", "ki", "çok", "daha", "en", "bu", "şu", "o",
    "onun", "gibi", "kadar", "ama", "ancak",
];

/// Character-level Levenshtein edit distance between two strings.
///
/// Operates on Unicode characters (not bytes). Used as the basis for CER.
pub fn levenshtein_distance(reference: &str, hypothesis: &str) -> usize {
    let ref_chars = reference.chars().collect::<Vec<_>>();
    let hyp_chars = hypothesis.chars().collect::<Vec<_>>();
    edit_distance(&ref_chars, &hyp_chars)
}

/// Generic Levenshtein distance over any equatable item sequence.
fn edit_distance<T: PartialEq>(reference: &[T], hypothesis: &[T]) -> usize {
    let (m, n) = (reference.len(), hypothesis.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut previous = (0..=n).collect::<Vec<_>>();
    let mut current = vec![0usize; n + 1];
    for i in 1..=m {
        current[0] = i;
        for j in 1..=n {
            let substitution = if reference[i - 1] == hypothesis[j - 1] {
                0
            } else {
                1
            };
            current[j] = (previous[j] + 1)
                .min(current[j - 1] + 1)
                .min(previous[j - 1] + substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[n]
}

/// Character Error Rate: normalized Levenshtein distance divided by the
/// normalized reference length. `0.0` when the reference is empty (both
/// identical normalized inputs yield `0.0`).
pub fn character_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let ref_norm = normalize_for_comparison(reference);
    let hyp_norm = normalize_for_comparison(hypothesis);
    let ref_len = ref_norm.chars().count();
    if ref_len == 0 {
        return 0.0;
    }
    levenshtein_distance(&ref_norm, &hyp_norm) as f64 / ref_len as f64
}

/// Word Error Rate: word-level Levenshtein distance divided by the reference
/// word count. Words are derived from the normalized comparison form.
pub fn word_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let ref_words = words(reference);
    let hyp_words = words(hypothesis);
    if ref_words.is_empty() {
        return 0.0;
    }
    edit_distance(&ref_words, &hyp_words) as f64 / ref_words.len() as f64
}

/// Deterministic word tokens from the normalized comparison form.
fn words(input: &str) -> Vec<String> {
    normalize_for_comparison(input)
        .split(' ')
        .filter(|word| !word.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Result of a critical-token check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CriticalTokenReport {
    /// Critical tokens that are absent from the normalized hypothesis.
    pub missing_tokens: Vec<String>,
}

impl CriticalTokenReport {
    pub fn missing_count(&self) -> usize {
        self.missing_tokens.len()
    }

    pub fn is_clean(&self) -> bool {
        self.missing_tokens.is_empty()
    }
}

/// Counts the critical tokens (key names, numbers, field values) that do not
/// appear in the hypothesis text. Presence is a normalized substring check so
/// multi-word field values ("müdür yardımcısı") are matched as a unit.
pub fn critical_token_error(
    _reference: &str,
    hypothesis: &str,
    critical: &[&str],
) -> CriticalTokenReport {
    let hyp = normalize_for_comparison(hypothesis);
    let mut missing = Vec::new();
    for token in critical {
        if !hyp.contains(&normalize_for_comparison(token)) {
            missing.push((*token).to_string());
        }
    }
    CriticalTokenReport {
        missing_tokens: missing,
    }
}

/// Result of a printed-question leakage check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeakageReport {
    pub leaked: bool,
    pub overlap_words: Vec<String>,
    pub overlap_ratio: f64,
}

/// Detects whether the printed question text leaked into the OCR'd student
/// answer. Overlap is measured over the printed question's content words
/// (stopwords removed) present in the OCR text. A leak requires both a
/// minimum word count and a minimum overlap ratio, so a handful of shared
/// function words never triggers a report.
pub fn printed_question_leakage(ocr_text: &str, printed_question: &str) -> LeakageReport {
    let ocr_words = words(ocr_text);
    let printed_content = words(printed_question)
        .into_iter()
        .filter(|word| !TURKISH_STOPWORDS.contains(&word.as_str()))
        .collect::<Vec<_>>();
    if printed_content.is_empty() {
        return LeakageReport {
            leaked: false,
            overlap_words: Vec::new(),
            overlap_ratio: 0.0,
        };
    }
    let overlap = printed_content
        .iter()
        .filter(|word| ocr_words.contains(word))
        .cloned()
        .collect::<Vec<_>>();
    let ratio = overlap.len() as f64 / printed_content.len() as f64;
    let leaked = overlap.len() >= LEAKAGE_MIN_OVERLAP_WORDS && ratio >= LEAKAGE_MIN_OVERLAP_RATIO;
    LeakageReport {
        leaked,
        overlap_words: overlap,
        overlap_ratio: ratio,
    }
}

/// Exact normalized match of a single structured field (used for Q2 table
/// fields and Q3 matching keys).
pub fn structured_field_exact_match(expected: &str, actual: &str) -> bool {
    normalize_for_comparison(expected) == normalize_for_comparison(actual)
}

/// Whether every expected structured field matches its actual counterpart
/// exactly (order and length must agree). Used for the `1.0` exact-match
/// quality gate.
pub fn structured_fields_all_exact(expected: &[String], actual: &[String]) -> bool {
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual.iter())
            .all(|(expected, actual)| structured_field_exact_match(expected, actual))
}

/// `p`-th percentile (0..100) over a copy of the samples, rounded to the
/// nearest rank. `None` for an empty input.
pub fn percentile(samples: &[f64], p: f64) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = ((p.clamp(0.0, 100.0) / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted.get(rank).copied()
}

/// Median (p50) of the samples.
pub fn percentile_p50(samples: &[f64]) -> Option<f64> {
    percentile(samples, 50.0)
}

/// p95 of the samples.
pub fn percentile_p95(samples: &[f64]) -> Option<f64> {
    percentile(samples, 95.0)
}

/// Whether a model binary was available when the report was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoldenModelRuntimeStatus {
    /// Model binary present; OCR/scoring metrics were actually measured.
    Available,
    /// Model binary absent; model-dependent fields are `None`.
    NeedsModelRuntime,
}

/// Per-question golden measurement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GoldenQuestionMetric {
    pub question_id: String,
    pub answer_type: String,
    pub pages: Vec<u32>,
    pub region_count: usize,
    /// OCR quality metrics; `None` without a model runtime.
    pub cer: Option<f64>,
    pub wer: Option<f64>,
    pub critical_token_missing: Option<usize>,
    pub printed_question_leakage: Option<bool>,
    pub structured_exact_match: Option<bool>,
    /// p50/p95 model latency in milliseconds.
    pub duration_ms_p50: Option<u64>,
    pub duration_ms_p95: Option<u64>,
    pub image_token_count: Option<u64>,
    pub model_call_count: Option<u64>,
    pub retry_count: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
}

/// Aggregate golden report fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GoldenAggregateMetric {
    pub cer_p50: Option<f64>,
    pub cer_p95: Option<f64>,
    pub wer_p50: Option<f64>,
    pub wer_p95: Option<f64>,
    pub total_image_tokens: Option<u64>,
    pub total_model_calls: Option<u64>,
    pub total_retries: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
}

/// Documented measurement report contract for the golden OCR/scoring
/// benchmark. Model-dependent fields are optional so a report can be produced
/// (with `model_runtime = needs_model_runtime`) even when no model binary is
/// present; such a report is explicitly a structural/preview report, never a
/// claimed benchmark pass.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GoldenOcrBenchmarkReport {
    /// Documented report schema version (bump on contract change).
    pub schema_version: String,
    pub exam_id: String,
    pub generated_at: String,
    pub model_runtime: GoldenModelRuntimeStatus,
    /// SHA-256 of the committed golden manifest that gated this corpus run.
    pub corpus_manifest_sha256: Option<String>,
    pub per_question: Vec<GoldenQuestionMetric>,
    pub aggregate: GoldenAggregateMetric,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_bbox_y_converts_bottom_left_to_top_left() {
        // Bottom-left y=0.9 with height 0.1 spans [0.9, 1.0] in PDF space;
        // top-left equivalent spans [0.0, 0.1].
        assert_eq!(corpus_bbox_bottom_left_y_to_top_left(0.9, 0.1), 0.0);
        assert_eq!(corpus_bbox_bottom_left_y_to_top_left(0.5, 0.2), 0.3);
        assert_eq!(corpus_bbox_bottom_left_y_to_top_left(0.0, 0.1), 0.9);
        assert_eq!(corpus_bbox_bottom_left_y_to_top_left(0.0, 1.0), 0.0);
    }

    #[test]
    fn corpus_bbox_y_conversion_clamps_to_unit_interval() {
        // A region poking past the top of the page (negative bottom-left y)
        // maps to y_top=1.0; a region below the page maps to y_top=0.0.
        assert_eq!(corpus_bbox_bottom_left_y_to_top_left(-0.1, 0.0), 1.0);
        assert_eq!(corpus_bbox_bottom_left_y_to_top_left(1.2, 0.0), 0.0);
        assert_eq!(corpus_bbox_bottom_left_y_to_top_left(0.5, 0.7), 0.0);
    }

    #[test]
    fn corpus_bbox_y_conversion_is_invertible_round_trip() {
        for (y_bottom, height) in [(0.1_f32, 0.3_f32), (0.4, 0.2), (0.0, 0.5), (0.7, 0.1)] {
            let y_top = corpus_bbox_bottom_left_y_to_top_left(y_bottom, height);
            let expected_top = 1.0 - (y_bottom + height);
            assert!((y_top - expected_top).abs() < 1e-6);
        }
    }

    #[test]
    fn levenshtein_basics() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("abc", ""), 3);
    }

    #[test]
    fn levenshtein_counts_unicode_chars_not_bytes() {
        assert_eq!(levenshtein_distance("ğçş", "ğçş"), 0);
        assert_eq!(levenshtein_distance("ğçş", "ğç"), 1);
    }

    #[test]
    fn cer_is_zero_for_identical_turkish_text() {
        assert_eq!(
            character_error_rate(
                "Ece, defteri müdür yardımcısına teslim etti.",
                "Ece, defteri müdür yardımcısına teslim etti."
            ),
            0.0
        );
    }

    #[test]
    fn cer_is_fractional_for_substitutions() {
        // One token ("kitabı") substituted out of the reference.
        let ref_text = "Ece defteri okudu.";
        let hyp_text = "Ece kitabı okudu.";
        let cer = character_error_rate(ref_text, hyp_text);
        assert!(cer > 0.0 && cer < 1.0);
    }

    #[test]
    fn cer_is_bounded_by_one() {
        let cer = character_error_rate("abcde", "zyxwv");
        assert!(cer <= 1.0);
    }

    #[test]
    fn wer_is_zero_for_identical_word_sequences() {
        assert_eq!(
            word_error_rate("Üçüncü kişi anlatıcı", "ÜÇÜNCÜ KİŞİ ANLATICI"),
            0.0
        );
    }

    #[test]
    fn wer_counts_single_word_substitution() {
        let wer = word_error_rate("bir iki üç", "bir iki dört");
        assert!((wer - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn critical_token_error_reports_missing_only() {
        let report = critical_token_error(
            "Ece defteri müdür yardımcısına teslim etti.",
            "defteri müdür yardımcısına teslim etti.",
            &["Ece", "defteri", "müdür yardımcısı"],
        );
        assert_eq!(report.missing_tokens, vec!["Ece"]);
        assert_eq!(report.missing_count(), 1);
    }

    #[test]
    fn critical_token_error_is_clean_when_all_present() {
        let report = critical_token_error(
            "Ece defteri müdür yardımcısına teslim etti.",
            "Ece, defteri müdür yardımcısına teslim etti.",
            &["Ece", "müdür yardımcısı"],
        );
        assert!(report.is_clean());
    }

    #[test]
    fn leakage_detects_printed_question_in_ocr() {
        let printed = "Aşağıdaki metni okuyun ve Ece'nin davranışını gerekçelendirin.";
        let ocr_leaked =
            "Ece'nin davranışı sorumluluk ve özel alana saygı arasındadır. Metni okuyun.";
        let report = printed_question_leakage(ocr_leaked, printed);
        assert!(
            report.leaked,
            "expected leak, overlap={:?} ratio={}",
            report.overlap_words, report.overlap_ratio
        );
    }

    #[test]
    fn leakage_is_clean_for_pure_student_answer() {
        let printed = "Aşağıdaki metni okuyun ve Ece'nin davranışını gerekçelendirin.";
        let ocr = "Ece sorumluluk sahibi bir öğrencidir; defteri iade etmeye karar verdi.";
        let report = printed_question_leakage(ocr, printed);
        assert!(
            !report.leaked,
            "unexpected leak: {:?}",
            report.overlap_words
        );
    }

    #[test]
    fn leakage_stopwords_do_not_trigger_false_positive() {
        let printed = "Şiirde ayrılık ile umut arasında bir duygu vardır.";
        let ocr = "öğrenci bir cevap yazdı ve öğretmenine verdi";
        let report = printed_question_leakage(ocr, printed);
        assert!(!report.leaked);
    }

    #[test]
    fn structured_field_exact_match_folds_turkish_case() {
        assert!(structured_field_exact_match(
            "Üçüncü kişi anlatıcı",
            "ÜÇÜNCÜ KİŞİ ANLATICI"
        ));
        assert!(structured_field_exact_match(
            "Okulun eski kütüphanesi",
            "okulun eski kütüphanesi"
        ));
    }

    #[test]
    fn structured_fields_all_exact_requires_same_order_and_length() {
        let expected = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        assert!(structured_fields_all_exact(&expected, &expected));
        assert!(!structured_fields_all_exact(
            &expected,
            &["A".to_string(), "B".to_string()]
        ));
        assert!(!structured_fields_all_exact(
            &expected,
            &["B".to_string(), "A".to_string(), "C".to_string()]
        ));
    }

    #[test]
    fn percentiles_compute_p50_and_p95() {
        let samples = [1.0, 2.0, 3.0, 4.0, 100.0];
        assert_eq!(percentile_p50(&samples), Some(3.0));
        assert_eq!(percentile_p95(&samples), Some(100.0));
        assert_eq!(percentile(&[], 50.0), None);
    }

    #[test]
    fn report_dto_round_trips_with_needs_model_runtime() {
        let report = GoldenOcrBenchmarkReport {
            schema_version: "rubrika.golden.ocr.benchmark.v1".to_string(),
            exam_id: "tymm_tde_001".to_string(),
            generated_at: "2026-08-06T00:00:00Z".to_string(),
            model_runtime: GoldenModelRuntimeStatus::NeedsModelRuntime,
            corpus_manifest_sha256: Some("abc123".to_string()),
            per_question: Vec::new(),
            aggregate: GoldenAggregateMetric {
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
        let json = serde_json::to_string(&report).expect("serialize");
        let decoded: GoldenOcrBenchmarkReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, report);
        assert!(json.contains("\"modelRuntime\":\"needs_model_runtime\""));
        assert!(json.contains("corpusManifestSha256"));
    }
}
