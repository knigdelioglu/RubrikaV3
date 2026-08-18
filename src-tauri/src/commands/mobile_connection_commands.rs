use serde::Serialize;
#[cfg(target_os = "macos")]
use std::process::Command;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileConnectionInfo {
    pub local_host_name: String,
    pub device_host: String,
    pub web_url: String,
    pub api_url: String,
    pub api_enabled: bool,
    pub token_configured: bool,
}

#[tauri::command]
pub fn get_mobile_connection_info() -> MobileConnectionInfo {
    let local_host_name = mac_local_host_name();
    let device_host = format!("{local_host_name}.local");
    let web_port = std::env::var("VITE_PORT").unwrap_or_else(|_| "5173".to_string());
    let api_port = std::env::var("RUBRIKA_LAN_API_PORT").unwrap_or_else(|_| "8787".to_string());
    let api_enabled = std::env::var("RUBRIKA_LAN_API")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false);

    MobileConnectionInfo {
        local_host_name,
        web_url: format!("http://{device_host}:{web_port}/mobile"),
        api_url: format!("http://{device_host}:{api_port}"),
        device_host,
        api_enabled,
        token_configured: std::env::var("RUBRIKA_LAN_API_TOKEN")
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
    }
}

fn mac_local_host_name() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("scutil")
            .args(["--get", "LocalHostName"])
            .output()
        {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !value.is_empty() {
                return value;
            }
        }
    }

    std::env::var("HOSTNAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "rubrika-host".to_string())
        .trim_end_matches(".local")
        .to_string()
}
