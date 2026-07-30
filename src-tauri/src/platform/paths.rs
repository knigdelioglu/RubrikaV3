use crate::domain::errors::AppError;
use std::path::PathBuf;
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
) -> Result<String, AppError> {
    let safe_name = sanitize_project_name(project_name);
    if safe_name.is_empty() {
        return Ok(String::new());
    }

    let mut base_dir = app
        .path()
        .document_dir()
        .unwrap_or_else(|_| app.path().home_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Wait, the requirements state:
    // macOS: ~/Documents/RubrikaV3/Projects/<safe_project_name>
    // fallback: ~/RubrikaV3/Projects/<safe_project_name>

    // In both cases, we just append RubrikaV3/Projects
    base_dir.push("RubrikaV3");
    base_dir.push("Projects");

    let mut target_dir = base_dir.join(&safe_name);

    let mut counter = 2;
    while target_dir.exists() {
        target_dir = base_dir.join(format!("{}_{}", safe_name, counter));
        counter += 1;
    }

    Ok(target_dir.to_string_lossy().to_string())
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
}
