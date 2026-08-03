//! One comparison-only Unicode normalization policy for Turkish text.
//!
//! This module must never be used to replace persisted OCR text.  It is used
//! only for comparison, duplicate detection, similarity, cache keys and
//! legacy criterion matching.

use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

/// Normalizes user/model text for comparison without mutating the source text.
///
/// The order is intentional: compatibility decomposition handles composed and
/// combining forms, Turkish case folding is applied explicitly, punctuation is
/// converted to whitespace, and whitespace is collapsed at the end.
pub fn normalize_for_comparison(input: &str) -> String {
    input
        .nfkd()
        .flat_map(char::to_lowercase)
        .filter(|ch| !is_combining_mark(*ch))
        .map(|ch| match ch {
            'ı' => 'i',
            'İ' => 'i',
            'à'..='å' => 'a',
            'ç' => 'c',
            'è'..='ë' => 'e',
            'ğ' => 'g',
            'ì'..='ï' => 'i',
            'ñ' => 'n',
            'ò'..='ö' => 'o',
            'ş' => 's',
            'ù'..='ü' => 'u',
            'ý' | 'ÿ' => 'y',
            ch if ch.is_alphanumeric() || ch.is_whitespace() => ch,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A stable cache/similarity key. Kept separate as an explicit API so call
/// sites cannot accidentally persist the normalized value as OCR text.
pub fn comparison_key(input: &str) -> String {
    normalize_for_comparison(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_turkish_case_variants() {
        assert_eq!(normalize_for_comparison("I İ ı i"), "i i i i");
        assert_eq!(
            normalize_for_comparison("Ç ç Ğ ğ Ö ö Ş ş Ü ü"),
            "c c g g o o s s u u"
        );
    }

    #[test]
    fn folds_combining_marks_and_compatibility_forms() {
        assert_eq!(
            normalize_for_comparison("İstanbul I\u{307}stanbul"),
            "istanbul istanbul"
        );
        assert_eq!(normalize_for_comparison("Cafe\u{301} CAFÉ"), "cafe cafe");
    }

    #[test]
    fn replaces_punctuation_and_collapses_whitespace() {
        assert_eq!(
            normalize_for_comparison("  Merhaba,\n\tDünya!  Çalışma...  "),
            "merhaba dunya calisma"
        );
    }

    #[test]
    fn preserves_source_value_contract() {
        let source = "İyi\u{301} cevap!";
        let _ = normalize_for_comparison(source);
        assert_eq!(source, "İyi\u{301} cevap!");
    }
}
