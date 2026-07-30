use serde::{Deserialize, Serialize};

use super::student::PageGroupingMode;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SchoolClassStatus {
    #[default]
    Active,
    Archived,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SchoolClass {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub normalized_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub academic_year: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grade_level: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(default)]
    pub display_order: u32,
    #[serde(default)]
    pub status: SchoolClassStatus,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StudentScanBatch {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub class_id: String,
    #[serde(default)]
    pub document_id: String,
    #[serde(default)]
    pub original_file_name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages_per_student: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouping_mode: Option<PageGroupingMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouping_completed_at: Option<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

pub fn normalize_school_class_name(value: &str) -> Option<String> {
    let uppercase = value.trim().to_uppercase();
    if uppercase.is_empty() {
        return None;
    }

    let mut normalized = String::new();
    let mut separator_pending = false;
    for character in uppercase.chars() {
        if character.is_alphanumeric() {
            if separator_pending && !normalized.is_empty() {
                normalized.push('-');
            }
            separator_pending = false;
            normalized.push(character);
        } else if !normalized.is_empty() {
            separator_pending = true;
        }
    }

    if normalized.is_empty() {
        return None;
    }

    if !normalized.contains('-') {
        let digit_count = normalized
            .chars()
            .take_while(|value| value.is_ascii_digit())
            .count();
        if digit_count > 0 && digit_count < normalized.chars().count() {
            let suffix = normalized.chars().skip(digit_count).collect::<String>();
            normalized = format!("{}-{suffix}", &normalized[..digit_count]);
        }
    }

    Some(normalized)
}

#[cfg(test)]
mod tests {
    use super::normalize_school_class_name;

    #[test]
    fn normalizes_equivalent_class_names() {
        assert_eq!(normalize_school_class_name("11-A").as_deref(), Some("11-A"));
        assert_eq!(
            normalize_school_class_name(" 11 A ").as_deref(),
            Some("11-A")
        );
        assert_eq!(normalize_school_class_name("11-a").as_deref(), Some("11-A"));
        assert_eq!(normalize_school_class_name("11A").as_deref(), Some("11-A"));
        assert_eq!(normalize_school_class_name("   "), None);
    }
}
