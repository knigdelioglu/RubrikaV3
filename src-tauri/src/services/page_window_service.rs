//! Question-to-page mapping and bounded page windows for extraction.
//!
//! Extraction previously resent every prepared page image for each target
//! question (TD-19). This module provides the pure functions used to target a
//! single page (or a small window) per question and to escalate to a bounded
//! broad fallback only when the targeted result is not visible.
//!
//! These helpers never touch the model or the filesystem; they only derive
//! page sets from the `pdftotext` raw text and the prepared model inputs.

use std::collections::{BTreeMap, BTreeSet};

use crate::domain::model::ModelInputImage;
use crate::services::document_content_extraction_service::detect_question_markers;

/// Radius of the ±N page window used when the exact target page does not
/// contain the expected question.
pub const WINDOW_RADIUS: u32 = 1;

/// Maps every detected question marker to the page(s) of the raw `pdftotext`
/// output (pages are separated by form feed, U+000C) on which it appears.
pub fn question_numbers_by_page(raw_text: &str) -> BTreeMap<u32, Vec<u32>> {
    raw_text
        .split('\u{c}')
        .enumerate()
        .filter_map(|(index, page_text)| {
            let numbers = detect_question_markers(page_text)
                .into_keys()
                .collect::<Vec<_>>();
            if numbers.is_empty() {
                None
            } else {
                Some(((index + 1) as u32, numbers))
            }
        })
        .collect()
}

/// The pages a question is expected on: the page(s) where its marker was
/// detected, or a deterministic linear estimate across the document when no
/// marker evidence exists (e.g. scanned PDFs). Always non-empty unless there
/// are no pages at all.
pub fn candidate_pages_for_question(
    question_number: u32,
    page_questions: &BTreeMap<u32, Vec<u32>>,
    expected_question_count: u32,
    page_count: u32,
) -> Vec<u32> {
    let matched = page_questions
        .iter()
        .filter(|(_, numbers)| numbers.contains(&question_number))
        .map(|(page, _)| *page)
        .collect::<Vec<_>>();
    if !matched.is_empty() {
        return matched;
    }
    let estimate =
        estimated_page_for_question(question_number, expected_question_count, page_count);
    if estimate == 0 {
        Vec::new()
    } else {
        vec![estimate]
    }
}

/// Deterministic linear page estimate for a question without a detected marker.
pub fn estimated_page_for_question(
    question_number: u32,
    expected_question_count: u32,
    page_count: u32,
) -> u32 {
    if page_count == 0 {
        return 0;
    }
    if expected_question_count == 0 {
        return 1;
    }
    let scaled = (question_number.max(1) as u64)
        .saturating_mul(page_count as u64)
        .div_ceil(expected_question_count.max(1) as u64)
        .max(1)
        .min(page_count as u64);
    scaled as u32
}

/// Expands the given pages by ±`radius`, clamped to the document page range.
/// Returns pages in ascending order with no duplicates.
pub fn expand_page_window(pages: &[u32], page_count: u32, radius: u32) -> Vec<u32> {
    let mut window = BTreeSet::new();
    for page in pages {
        let start = page.saturating_sub(radius).max(1);
        let end = page.saturating_add(radius).min(page_count.max(1));
        for candidate in start..=end {
            window.insert(candidate);
        }
    }
    window.into_iter().collect()
}

/// Selects the prepared model inputs that belong to the given pages, in
/// document order.
pub fn select_inputs_by_pages(inputs: &[ModelInputImage], pages: &[u32]) -> Vec<ModelInputImage> {
    let page_set = pages.iter().copied().collect::<BTreeSet<_>>();
    inputs
        .iter()
        .filter(|input| page_set.contains(&input.page_number))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{ModelInputImage, ModelInputImageKind};

    fn image(page_number: u32) -> ModelInputImage {
        ModelInputImage {
            kind: ModelInputImageKind::QuestionText,
            document_id: "d1".to_string(),
            page_number,
            source_image_path: format!("src-{page_number}.jpg"),
            output_image_path: format!("out-{page_number}.jpg"),
            source_width: 1,
            source_height: 1,
            output_width: 1,
            output_height: 1,
            source_bytes: 0,
            output_bytes: 0,
            base64_approx_bytes: 0,
            long_edge_max: 2000,
            jpeg_quality: 92,
            created_at: "now".to_string(),
            source_sha256: None,
            output_sha256: None,
            cache_key: None,
            cache_transaction_id: None,
            cache_hit: false,
        }
    }

    #[test]
    fn question_numbers_by_page_splits_on_form_feed() {
        let raw = "Başlık\nS1. İlk soru.\n\u{c}S2. İkinci soru.\nS3. Üçüncü soru.\n\u{c}S4. Dördüncü soru.";
        let map = question_numbers_by_page(raw);
        assert_eq!(map.get(&1), Some(&vec![1]));
        assert_eq!(map.get(&2), Some(&vec![2, 3]));
        assert_eq!(map.get(&3), Some(&vec![4]));
    }

    #[test]
    fn question_numbers_by_page_returns_empty_for_blank_text() {
        assert!(question_numbers_by_page("").is_empty());
        assert!(question_numbers_by_page("\u{c}\u{c}").is_empty());
    }

    #[test]
    fn candidate_pages_prefers_detected_marker() {
        let mut page_questions = BTreeMap::new();
        page_questions.insert(1, vec![1]);
        page_questions.insert(2, vec![2, 3]);
        assert_eq!(
            candidate_pages_for_question(3, &page_questions, 4, 2),
            vec![2]
        );
    }

    #[test]
    fn candidate_pages_falls_back_to_linear_estimate() {
        let page_questions = BTreeMap::new();
        // 5 questions over 5 pages: question 4 -> page 4.
        assert_eq!(
            candidate_pages_for_question(4, &page_questions, 5, 5),
            vec![4]
        );
        // 10 questions over 2 pages: question 1 -> page 1, question 10 -> page 2.
        assert_eq!(
            candidate_pages_for_question(1, &page_questions, 10, 2),
            vec![1]
        );
        assert_eq!(
            candidate_pages_for_question(10, &page_questions, 10, 2),
            vec![2]
        );
    }

    #[test]
    fn candidate_pages_clamps_estimate_to_page_range() {
        let page_questions = BTreeMap::new();
        assert_eq!(
            candidate_pages_for_question(99, &page_questions, 10, 3),
            vec![3]
        );
        assert_eq!(
            candidate_pages_for_question(0, &page_questions, 10, 3),
            vec![1]
        );
    }

    #[test]
    fn expand_page_window_clamps_to_document_range() {
        assert_eq!(expand_page_window(&[1], 5, 1), vec![1, 2]);
        assert_eq!(expand_page_window(&[5], 5, 1), vec![4, 5]);
        assert_eq!(expand_page_window(&[3], 5, 1), vec![2, 3, 4]);
        assert_eq!(expand_page_window(&[2, 4], 5, 1), vec![1, 2, 3, 4, 5]);
        assert_eq!(expand_page_window(&[1], 1, 1), vec![1]);
    }

    #[test]
    fn select_inputs_by_pages_filters_and_keeps_order() {
        let inputs = vec![image(1), image(2), image(3), image(4)];
        let selected = select_inputs_by_pages(&inputs, &[3, 1]);
        assert_eq!(
            selected.iter().map(|i| i.page_number).collect::<Vec<_>>(),
            vec![1, 3]
        );
    }

    #[test]
    fn select_inputs_by_pages_is_empty_for_no_match() {
        let inputs = vec![image(1), image(2)];
        assert!(select_inputs_by_pages(&inputs, &[9]).is_empty());
    }
}
