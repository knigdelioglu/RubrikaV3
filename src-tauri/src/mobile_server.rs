//! Opt-in local-network API for the tablet MVP.
//!
//! This deliberately starts with read-only endpoints. The desktop Tauri
//! commands remain the canonical desktop boundary; the mobile boundary will
//! grow endpoint by endpoint as each workflow gets conflict/error semantics.

use crate::domain::errors::AppError;
use crate::AppState;
use serde::Serialize;
use serde_json::json;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::thread;
use tauri::{AppHandle, Manager};

const DEFAULT_PORT: u16 = 8787;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    app_version: String,
    platform: String,
    tauri_ready: bool,
    rust_backend_ready: bool,
    mobile_api_ready: bool,
}

pub fn start_if_enabled(app: AppHandle) {
    let enabled = std::env::var("RUBRIKA_LAN_API")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false);
    if !enabled {
        return;
    }

    let host = std::env::var("RUBRIKA_LAN_API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("RUBRIKA_LAN_API_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    let token = std::env::var("RUBRIKA_LAN_API_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());

    let Ok(ip) = host.parse::<IpAddr>() else {
        log::error!("LAN API host geçersiz: {host}");
        return;
    };
    if !ip.is_loopback() && token.is_none() {
        log::error!("LAN API loopback dışına açılmak için RUBRIKA_LAN_API_TOKEN gereklidir");
        return;
    }

    let address = SocketAddr::new(ip, port);
    let listener = match TcpListener::bind(address) {
        Ok(listener) => listener,
        Err(error) => {
            log::error!("LAN API başlatılamadı ({address}): {error}");
            return;
        }
    };

    log::info!("LAN API hazır: http://{address}");
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let app = app.clone();
                    let token = token.clone();
                    thread::spawn(move || handle_connection(stream, &app, token.as_deref()));
                }
                Err(error) => log::warn!("LAN API bağlantısı kabul edilemedi: {error}"),
            }
        }
    });
}

fn handle_connection(mut stream: TcpStream, app: &AppHandle, expected_token: Option<&str>) {
    let mut buffer = [0_u8; 16 * 1024];
    let Ok(bytes_read) = stream.read(&mut buffer) else {
        return;
    };
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let mut lines = request.lines();
    let Some(request_line) = lines.next() else {
        return;
    };
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default();
    let path = request_parts.next().unwrap_or_default();

    if method == "OPTIONS" {
        write_response(&mut stream, 204, "", None);
        return;
    }
    if method != "GET" {
        write_response(
            &mut stream,
            405,
            &json!({ "message": "Yalnızca GET destekleniyor." }).to_string(),
            Some("application/json"),
        );
        return;
    }
    if let Some(expected_token) = expected_token {
        let provided_token = request
            .lines()
            .find_map(|line| line.strip_prefix("X-Rubrika-Token:").map(str::trim));
        if provided_token != Some(expected_token) {
            write_response(
                &mut stream,
                401,
                &json!({ "message": "Geçersiz veya eksik mobil erişim anahtarı." }).to_string(),
                Some("application/json"),
            );
            return;
        }
    }

    match route(path, app) {
        Ok((status, body)) => write_response(&mut stream, status, &body, Some("application/json")),
        Err(error) => write_response(
            &mut stream,
            500,
            &serde_json::to_string(&error).unwrap_or_else(|_| "{}".to_string()),
            Some("application/json"),
        ),
    }
}

fn route(path: &str, app: &AppHandle) -> Result<(u16, String), AppError> {
    let state = app.state::<AppState>();
    match path.trim_end_matches('/') {
        "/api/mobile/health" => {
            let body = HealthResponse {
                app_version: env!("CARGO_PKG_VERSION").to_string(),
                platform: std::env::consts::OS.to_string(),
                tauri_ready: true,
                rust_backend_ready: true,
                mobile_api_ready: true,
            };
            Ok((
                200,
                serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_string()),
            ))
        }
        "/api/mobile/projects" => Ok((
            200,
            serde_json::to_string(&state.project_store.list_projects())
                .unwrap_or_else(|_| "{}".to_string()),
        )),
        path if path.starts_with("/api/mobile/projects/") => {
            let project_id = path.trim_start_matches("/api/mobile/projects/").trim();
            if project_id.is_empty() || project_id.contains('/') {
                return Ok((404, json!({ "message": "Proje bulunamadı." }).to_string()));
            }
            let project = state
                .project_store
                .get_project_snapshot(project_id.to_string())?;
            Ok((
                200,
                serde_json::to_string(&project).unwrap_or_else(|_| "{}".to_string()),
            ))
        }
        _ => Ok((
            404,
            json!({ "message": "Mobil API yolu bulunamadı." }).to_string(),
        )),
    }
}

fn write_response(stream: &mut TcpStream, status: u16, body: &str, content_type: Option<&str>) {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Internal Server Error",
    };
    let content_type = content_type.unwrap_or("text/plain; charset=utf-8");
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\nAccess-Control-Allow-Headers: X-Rubrika-Token, Content-Type\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}
