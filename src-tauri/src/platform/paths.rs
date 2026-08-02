use crate::domain::errors::AppError;
use std::path::{Path, PathBuf};
use tauri::Manager;

pub fn app_log_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Library").join("Logs").join("RubrikaV3")
}

pub fn model_server_log_path(profile_id: &str) -> PathBuf {
    app_log_dir().join(format!("{}-server.log", profile_id))
}

pub fn generate_default_project_path(
    app: &tauri::AppHandle,
    project_name: &str,
    academic_year_id: Option<&str>,
) -> Result<String, AppError> {
    let Some(directory_name) = project_directory_name(project_name, academic_year_id) else {
        return Ok(String::new());
    };

    let mut base_dir = app
        .path()
        .document_dir()
        .unwrap_or_else(|_| app.path().home_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // macOS: ~/Documents/RubrikaV3/Projects/<project_name>_<academic_year>
    // fallback: ~/RubrikaV3/Projects/<project_name>_<academic_year>
    base_dir.push("RubrikaV3");
    base_dir.push("Projects");

    let target_dir = unique_project_path(&base_dir, &directory_name);

    Ok(target_dir.to_string_lossy().to_string())
}

fn project_directory_name(project_name: &str, academic_year_id: Option<&str>) -> Option<String> {
    let safe_name = sanitize_project_name(project_name);
    if safe_name.is_empty() {
        return None;
    }

    let safe_year = academic_year_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(sanitize_project_name)
        .filter(|value| !value.is_empty());

    Some(match safe_year {
        Some(year) => format!("{safe_name}_{year}"),
        None => safe_name,
    })
}

fn unique_project_path(base_dir: &Path, directory_name: &str) -> PathBuf {
    let mut target_dir = base_dir.join(directory_name);
    let mut counter = 2;
    while target_dir.exists() {
        target_dir = base_dir.join(format!("{directory_name}_{counter}"));
        counter += 1;
    }
    target_dir
}

pub fn sanitize_project_name(name: &str) -> String {
    let mut safe = String::new();
    let mut last_was_space = false;
    for c in name.chars() {
        match c {
            'a'..='z'
            | 'A'..='Z'
            | '0'..='9'
            | '_'
            | '-'
            | 'ğ'
            | 'Ğ'
            | 'ü'
            | 'Ü'
            | 'ş'
            | 'Ş'
            | 'ı'
            | 'İ'
            | 'ö'
            | 'Ö'
            | 'ç'
            | 'Ç' => {
                safe.push(c);
                last_was_space = false;
            }
            ' ' => {
                if !last_was_space {
                    safe.push('_');
                    last_was_space = true;
                }
            }
            _ => {
                // Ignore risky characters
            }
        }
    }
    safe = safe.trim_matches('_').to_string();
    safe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_project_name() {
        assert_eq!(
            sanitize_project_name("11C Birinci Yazılı"),
            "11C_Birinci_Yazılı"
        );
        assert_eq!(
            sanitize_project_name("My Project: / \n  Test!"),
            "My_Project_Test"
        );
        assert_eq!(sanitize_project_name("Test   Space"), "Test_Space");
    }

    #[test]
    fn default_project_directory_name_includes_academic_year() {
        assert_eq!(
            project_directory_name("11. edebiyat 1. Yazılı", Some("2026-2027")),
            Some("11_edebiyat_1_Yazılı_2026-2027".to_string())
        );
        assert_ne!(
            project_directory_name("11. edebiyat 1. Yazılı", Some("2026-2027")),
            project_directory_name("11. edebiyat 1. Yazılı", Some("2027-2028"))
        );
        assert_eq!(
            project_directory_name("11. edebiyat 1. Yazılı", Some("2027-2028")),
            Some("11_edebiyat_1_Yazılı_2027-2028".to_string())
        );
    }

    #[test]
    fn same_name_and_year_gets_a_collision_suffix() {
        let base_dir = std::env::temp_dir().join(format!(
            "rubrika-default-project-path-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(base_dir.join("11_edebiyat_1_Yazılı_2026-2027")).unwrap();

        let path = unique_project_path(&base_dir, "11_edebiyat_1_Yazılı_2026-2027");

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("11_edebiyat_1_Yazılı_2026-2027_2")
        );
        std::fs::remove_dir_all(base_dir).unwrap();
    }
}
