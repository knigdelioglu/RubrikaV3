use std::path::{Path, PathBuf};
use std::process::Command;

use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfRendererStatus {
    pub available: bool,
    pub backend: String,
    pub pdfinfo_path: Option<String>,
    pub pdftoppm_path: Option<String>,
    pub searched_paths: Vec<String>,
    pub path_env: Option<String>,
    pub install_hint: Option<String>,
    pub warnings: Vec<String>,
}

pub trait PdfService: Send + Sync {
    fn page_count(&self, pdf_path: &Path) -> Result<u32, AppError>;
    fn render_pages(
        &self,
        pdf_path: &Path,
        output_dir: &Path,
        pages: &[u32],
    ) -> Result<Vec<PathBuf>, AppError>;
    fn render_all_pages(
        &self,
        pdf_path: &Path,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>, AppError>;
    fn get_renderer_status(&self) -> Result<PdfRendererStatus, AppError>;
}

#[derive(Clone, Default)]
pub struct SystemPdfService;

#[cfg(target_os = "macos")]
const JXA_PDF_HELPER: &str = r#"function run(argv) {
  ObjC.import("PDFKit");
  ObjC.import("AppKit");
  ObjC.import("Foundation");

  if (argv.length < 2) return "ERROR: Missing arguments";

  var pdfPath = argv[0];
  var action = argv[1];

  var url = $.NSURL.fileURLWithPath(pdfPath);
  var doc = $.PDFDocument.alloc.initWithURL(url);
  if (!doc || doc.isNil) return "ERROR: Failed to load PDF at " + pdfPath;

  if (action === "count") return doc.pageCount.toString();

  if (action === "all") {
    if (argv.length < 3) return "ERROR: Missing output path";
    var outputDir = argv[2];

    var renderedCount = 0;
    for (var i = 1; i <= doc.pageCount; i++) {
      var page = doc.pageAtIndex(i - 1);
      var rect = page.boundsForBox(0);
      var size = rect.size;

      var scale = 2.0;
      var width = size.width * scale;
      var height = size.height * scale;

      var image = $.NSImage.alloc.initWithSize($.NSMakeSize(width, height));
      image.lockFocus;

      var context = $.NSGraphicsContext.currentContext;
      var cgContext = context.CGContext;

      $.CGContextSetRGBFillColor(cgContext, 1.0, 1.0, 1.0, 1.0);
      $.CGContextFillRect(cgContext, $.CGRectMake(0, 0, width, height));
      $.CGContextScaleCTM(cgContext, scale, scale);

      page.drawWithBoxToContext(0, cgContext);
      image.unlockFocus;

      var tiffData = image.TIFFRepresentation;
      var imageRep = $.NSBitmapImageRep.imageRepWithData(tiffData);
      var pngData = imageRep.representationUsingTypeProperties(4, $.NSDictionary.alloc.init);
      var outputPath = outputDir + "/page-" + i + ".png";
      var success = pngData.writeToFileAtomically(outputPath, true);
      if (!success) return "ERROR: Failed to write PNG";
      renderedCount += 1;
    }

    return renderedCount.toString();
  }

  var pageNum = parseInt(action);
  if (isNaN(pageNum)) return "ERROR: Invalid action or page number: " + action;

  if (argv.length < 3) return "ERROR: Missing output path";
  var outputPath = argv[2];

  var pageCount = doc.pageCount;
  if (pageNum < 1 || pageNum > pageCount) return "ERROR: Page number out of bounds";

  var page = doc.pageAtIndex(pageNum - 1);
  var rect = page.boundsForBox(0);
  var size = rect.size;

  var scale = 2.0;
  var width = size.width * scale;
  var height = size.height * scale;
  
  var image = $.NSImage.alloc.initWithSize($.NSMakeSize(width, height));
  image.lockFocus;
  
  var context = $.NSGraphicsContext.currentContext;
  var cgContext = context.CGContext;
  
  $.CGContextSetRGBFillColor(cgContext, 1.0, 1.0, 1.0, 1.0);
  $.CGContextFillRect(cgContext, $.CGRectMake(0, 0, width, height));
  $.CGContextScaleCTM(cgContext, scale, scale);
  
  page.drawWithBoxToContext(0, cgContext);
  image.unlockFocus;
  
  var tiffData = image.TIFFRepresentation;
  var imageRep = $.NSBitmapImageRep.imageRepWithData(tiffData);
  var pngData = imageRep.representationUsingTypeProperties(4, $.NSDictionary.alloc.init);
  
  var success = pngData.writeToFileAtomically(outputPath, true);
  return success ? "SUCCESS" : "ERROR: Failed to write PNG";
}"#;

#[cfg(target_os = "macos")]
fn run_jxa_pdf_helper(
    pdf_path: &Path,
    action: &str,
    extra_arg: Option<&str>,
) -> Result<String, String> {
    let mut cmd = Command::new("osascript");
    cmd.arg("-l")
        .arg("JavaScript")
        .arg("-e")
        .arg(JXA_PDF_HELPER);
    cmd.arg(pdf_path.to_string_lossy().as_ref());
    cmd.arg(action);
    if let Some(arg) = extra_arg {
        cmd.arg(arg);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute osascript: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("osascript exited with error: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.starts_with("ERROR:") {
        return Err(stdout);
    }

    Ok(stdout)
}

impl SystemPdfService {
    fn render_page(
        &self,
        pdf_path: &Path,
        output_dir: &Path,
        page: u32,
    ) -> Result<PathBuf, AppError> {
        ensure_pdf_exists(pdf_path)?;
        std::fs::create_dir_all(output_dir).map_err(|e| AppError {
            code: AppErrorCode::FileWriteFailed,
            message: "Failed to create preview directory.".to_string(),
            recoverable: false,
            suggested_action: Some("Check permissions for the project cache folder.".to_string()),
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        let prefix = output_dir.join(format!("page_{page}"));
        let output_png = prefix.with_extension("png");

        #[cfg(target_os = "macos")]
        {
            match run_jxa_pdf_helper(
                pdf_path,
                &page.to_string(),
                Some(&output_png.to_string_lossy()),
            ) {
                Ok(_) => return Ok(output_png),
                Err(err) => {
                    log::warn!(
                        "JXA PDF renderer failed: {}. Falling back to pdftoppm.",
                        err
                    );
                }
            }
        }

        let binary_path = find_binary("pdftoppm");
        let Some(binary_path) = binary_path else {
            let searched_paths = get_searched_paths();
            let path_env = std::env::var("PATH").unwrap_or_default();
            let selected_pdfinfo_path = find_binary("pdfinfo")
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "not found".to_string());
            let selected_pdftoppm_path = "not found".to_string();

            return Err(AppError {
                code: AppErrorCode::PdfRendererNotFound,
                message: "PDF önizleme aracı bulunamadı. Poppler kurulu olmayabilir. Terminalde `brew install poppler` komutunu çalıştırıp tekrar deneyin.".to_string(),
                recoverable: true,
                suggested_action: Some("Terminalde `brew install poppler` komutunu çalıştırıp tekrar deneyin.".to_string()),
                technical_details: Some(format!(
                    "selected_pdfinfo_path: {}\nselected_pdftoppm_path: {}\nsearched_paths: {:?}\npath_env: {}",
                    selected_pdfinfo_path,
                    selected_pdftoppm_path,
                    searched_paths,
                    path_env
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        };

        let mut cmd = configure_command("pdftoppm");
        cmd.arg("-png")
            .arg("-singlefile")
            .arg("-f")
            .arg(page.to_string())
            .arg("-l")
            .arg(page.to_string())
            .arg(pdf_path)
            .arg(&prefix);

        let status = cmd.status().map_err(|e| {
            let searched_paths = get_searched_paths();
            let path_env = std::env::var("PATH").unwrap_or_default();
            let selected_pdfinfo_path = find_binary("pdfinfo").map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "not found".to_string());
            let selected_pdftoppm_path = binary_path.to_string_lossy().into_owned();
            let spawn_args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
            let working_directory = cmd.get_current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| ".".to_string());

            AppError {
                code: AppErrorCode::PdfRendererStartFailed,
                message: "PDF önizleme aracı başlatılamadı.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Lütfen sistem izinlerini veya Poppler kurulumunu kontrol edin.".to_string(),
                ),
                technical_details: Some(format!(
                    "selected_pdfinfo_path: {}\nselected_pdftoppm_path: {}\nsearched_paths: {:?}\npath_env: {}\nspawn_args: {:?}\nworking_directory: {}\nos_error: {}",
                    selected_pdfinfo_path,
                    selected_pdftoppm_path,
                    searched_paths,
                    path_env,
                    spawn_args,
                    working_directory,
                    e
                )),
                correlation_id: Uuid::new_v4().to_string(),
            }
        })?;

        if !status.success() {
            return Err(AppError {
                code: AppErrorCode::PdfRenderFailed,
                message: "PDF önizleme aracı hata döndürdü.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Farklı bir PDF dosyası deneyin veya poppler araçlarını kurun.".to_string(),
                ),
                technical_details: Some(format!("pdftoppm çıkış kodu: {status}")),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        Ok(output_png)
    }
}

impl PdfService for SystemPdfService {
    fn page_count(&self, pdf_path: &Path) -> Result<u32, AppError> {
        ensure_pdf_exists(pdf_path)?;
        #[cfg(target_os = "macos")]
        {
            match run_jxa_pdf_helper(pdf_path, "count", None) {
                Ok(count_str) => {
                    if let Ok(count) = count_str.parse::<u32>() {
                        return Ok(count);
                    }
                }
                Err(err) => {
                    if find_binary("pdfinfo").is_none() {
                        return Err(AppError {
                            code: AppErrorCode::PdfRendererNotFound,
                            message: "PDF önizleme aracı bulunamadı. Poppler kurulu olmayabilir. Terminalde `brew install poppler` komutunu çalıştırıp tekrar deneyin.".to_string(),
                            recoverable: true,
                            suggested_action: Some(
                                "Lütfen geçerli bir PDF deneyin veya PDF araçlarını kurun."
                                    .to_string(),
                            ),
                            technical_details: Some(err),
                            correlation_id: Uuid::new_v4().to_string(),
                        });
                    }
                    log::warn!(
                        "JXA PDF page count failed: {}. Falling back to pdfinfo.",
                        err
                    );
                }
            }
        }

        let binary_path = find_binary("pdfinfo");
        let Some(binary_path) = binary_path else {
            let searched_paths = get_searched_paths();
            let path_env = std::env::var("PATH").unwrap_or_default();
            let selected_pdfinfo_path = "not found".to_string();
            let selected_pdftoppm_path = find_binary("pdftoppm")
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "not found".to_string());

            return Err(AppError {
                code: AppErrorCode::PdfRendererNotFound,
                message: "PDF önizleme aracı bulunamadı. Poppler kurulu olmayabilir. Terminalde `brew install poppler` komutunu çalıştırıp tekrar deneyin.".to_string(),
                recoverable: true,
                suggested_action: Some("Terminalde `brew install poppler` komutunu çalıştırıp tekrar deneyin.".to_string()),
                technical_details: Some(format!(
                    "selected_pdfinfo_path: {}\nselected_pdftoppm_path: {}\nsearched_paths: {:?}\npath_env: {}",
                    selected_pdfinfo_path,
                    selected_pdftoppm_path,
                    searched_paths,
                    path_env
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        };

        let mut cmd = configure_command("pdfinfo");
        cmd.arg(pdf_path);

        let output = cmd.output().map_err(|e| {
            let searched_paths = get_searched_paths();
            let path_env = std::env::var("PATH").unwrap_or_default();
            let selected_pdfinfo_path = binary_path.to_string_lossy().into_owned();
            let selected_pdftoppm_path = find_binary("pdftoppm").map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "not found".to_string());
            let spawn_args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
            let working_directory = cmd.get_current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| ".".to_string());

            AppError {
                code: AppErrorCode::PdfRendererStartFailed,
                message: "PDF önizleme aracı başlatılamadı.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Lütfen sistem izinlerini veya Poppler kurulumunu kontrol edin.".to_string(),
                ),
                technical_details: Some(format!(
                    "selected_pdfinfo_path: {}\nselected_pdftoppm_path: {}\nsearched_paths: {:?}\npath_env: {}\nspawn_args: {:?}\nworking_directory: {}\nos_error: {}",
                    selected_pdfinfo_path,
                    selected_pdftoppm_path,
                    searched_paths,
                    path_env,
                    spawn_args,
                    working_directory,
                    e
                )),
                correlation_id: Uuid::new_v4().to_string(),
            }
        })?;

        if !output.status.success() {
            return Err(AppError {
                code: AppErrorCode::PdfPageCountFailed,
                message: "PDF bilgi aracı hata döndürdü.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Farklı bir PDF dosyası deneyin veya poppler araçlarını kurun.".to_string(),
                ),
                technical_details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .find_map(|line| {
                line.strip_prefix("Pages:")
                    .and_then(|value| value.trim().parse::<u32>().ok())
            })
            .ok_or_else(|| AppError {
                code: AppErrorCode::PdfPageCountFailed,
                message: "PDF sayfa sayısı belirlenemedi.".to_string(),
                recoverable: true,
                suggested_action: Some("Geçerli bir PDF dosyası kullanın.".to_string()),
                technical_details: Some(stdout.to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            })
    }

    fn render_pages(
        &self,
        pdf_path: &Path,
        output_dir: &Path,
        pages: &[u32],
    ) -> Result<Vec<PathBuf>, AppError> {
        let mut rendered = Vec::new();
        for page in pages {
            rendered.push(self.render_page(pdf_path, output_dir, *page)?);
        }
        Ok(rendered)
    }

    fn render_all_pages(
        &self,
        pdf_path: &Path,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>, AppError> {
        ensure_pdf_exists(pdf_path)?;
        std::fs::create_dir_all(output_dir).map_err(|e| AppError {
            code: AppErrorCode::FileWriteFailed,
            message: "Failed to create preview directory.".to_string(),
            recoverable: false,
            suggested_action: Some("Check permissions for the project cache folder.".to_string()),
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        #[cfg(target_os = "macos")]
        {
            match run_jxa_pdf_helper(pdf_path, "all", Some(&output_dir.to_string_lossy())) {
                Ok(count_str) => {
                    let count = count_str.parse::<u32>().map_err(|e| AppError {
                        code: AppErrorCode::PdfRenderFailed,
                        message: "PDF sayfa önizlemeleri okunamadı.".to_string(),
                        recoverable: true,
                        suggested_action: Some(
                            "Lütfen geçerli bir PDF deneyin veya PDF araçlarını kurun.".to_string(),
                        ),
                        technical_details: Some(e.to_string()),
                        correlation_id: Uuid::new_v4().to_string(),
                    })?;

                    let mut rendered = Vec::new();
                    for page in 1..=count {
                        rendered.push(output_dir.join(format!("page-{page}.png")));
                    }
                    return Ok(rendered);
                }
                Err(err) => {
                    log::warn!(
                        "JXA PDF all-pages renderer failed: {}. Falling back to pdftoppm.",
                        err
                    );
                }
            }
        }

        let binary_path = find_binary("pdftoppm");
        let Some(binary_path) = binary_path else {
            let searched_paths = get_searched_paths();
            let path_env = std::env::var("PATH").unwrap_or_default();
            let selected_pdfinfo_path = find_binary("pdfinfo")
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "not found".to_string());
            let selected_pdftoppm_path = "not found".to_string();

            return Err(AppError {
                code: AppErrorCode::PdfRendererNotFound,
                message: "PDF önizleme aracı bulunamadı. Poppler kurulu olmayabilir. Terminalde `brew install poppler` komutunu çalıştırıp tekrar deneyin.".to_string(),
                recoverable: true,
                suggested_action: Some("Terminalde `brew install poppler` komutunu çalıştırıp tekrar deneyin.".to_string()),
                technical_details: Some(format!(
                    "selected_pdfinfo_path: {}\nselected_pdftoppm_path: {}\nsearched_paths: {:?}\npath_env: {}",
                    selected_pdfinfo_path,
                    selected_pdftoppm_path,
                    searched_paths,
                    path_env
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        };

        let output_prefix = output_dir.join("page");
        let mut cmd = configure_command("pdftoppm");
        cmd.arg("-png").arg(pdf_path).arg(&output_prefix);

        let status = cmd.status().map_err(|e| {
            let searched_paths = get_searched_paths();
            let path_env = std::env::var("PATH").unwrap_or_default();
            let selected_pdfinfo_path = find_binary("pdfinfo").map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "not found".to_string());
            let selected_pdftoppm_path = binary_path.to_string_lossy().into_owned();
            let spawn_args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
            let working_directory = cmd.get_current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| ".".to_string());

            AppError {
                code: AppErrorCode::PdfRendererStartFailed,
                message: "PDF önizleme aracı başlatılamadı.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Lütfen sistem izinlerini veya Poppler kurulumunu kontrol edin.".to_string(),
                ),
                technical_details: Some(format!(
                    "selected_pdfinfo_path: {}\nselected_pdftoppm_path: {}\nsearched_paths: {:?}\npath_env: {}\nspawn_args: {:?}\nworking_directory: {}\nos_error: {}",
                    selected_pdfinfo_path,
                    selected_pdftoppm_path,
                    searched_paths,
                    path_env,
                    spawn_args,
                    working_directory,
                    e
                )),
                correlation_id: Uuid::new_v4().to_string(),
            }
        })?;

        if !status.success() {
            return Err(AppError {
                code: AppErrorCode::PdfRenderFailed,
                message: "PDF önizleme aracı hata döndürdü.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Farklı bir PDF dosyası deneyin veya poppler araçlarını kurun.".to_string(),
                ),
                technical_details: Some(format!("pdftoppm çıkış kodu: {status}")),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let mut rendered = std::fs::read_dir(output_dir)
            .map_err(|e| AppError {
                code: AppErrorCode::PdfRenderFailed,
                message: "PDF önizleme çıktıları okunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Önizlemeleri yeniden oluşturun.".to_string()),
                technical_details: Some(e.to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            })?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("png"))
            .collect::<Vec<_>>();

        rendered.sort_by_key(|path| extract_page_number(path).unwrap_or(u32::MAX));
        Ok(rendered)
    }

    fn get_renderer_status(&self) -> Result<PdfRendererStatus, AppError> {
        let pdfinfo_opt = find_binary("pdfinfo");
        let pdftoppm_opt = find_binary("pdftoppm");
        let poppler_available = pdfinfo_opt.is_some() && pdftoppm_opt.is_some();
        // macOS has a native PDFKit renderer. The render methods try it first
        // and only fall back to Poppler when PDFKit cannot render the source.
        // Keep the status consistent with that runtime behavior; otherwise the
        // preview job is rejected before the native fallback is reached.
        let native_macos_available = cfg!(target_os = "macos");
        let available = poppler_available || native_macos_available;

        let mut backend = "none".to_string();
        if poppler_available {
            backend = "poppler".to_string();
        } else if native_macos_available {
            backend = "macos_fallback".to_string();
        }

        let pdfinfo_path = pdfinfo_opt.map(|p| p.to_string_lossy().into_owned());
        let pdftoppm_path = pdftoppm_opt.map(|p| p.to_string_lossy().into_owned());
        let searched_paths = get_searched_paths();
        let path_env = std::env::var("PATH").ok();

        let mut warnings = Vec::new();
        let mut install_hint = None;

        if !poppler_available && native_macos_available {
            warnings.push("Poppler bulunamadı; macOS PDFKit kullanılacak".to_string());
        } else if !available {
            warnings.push("pdfinfo or pdftoppm binary not found".to_string());
            install_hint = Some("brew install poppler".to_string());
        }

        Ok(PdfRendererStatus {
            available,
            backend,
            pdfinfo_path,
            pdftoppm_path,
            searched_paths,
            path_env,
            install_hint,
            warnings,
        })
    }
}

fn ensure_pdf_exists(pdf_path: &Path) -> Result<(), AppError> {
    if pdf_path.exists() && pdf_path.is_file() {
        return Ok(());
    }

    Err(AppError {
        code: AppErrorCode::PdfDocumentNotFound,
        message: "PDF dosyası bulunamadı.".to_string(),
        recoverable: true,
        suggested_action: Some("PDF dosyasını tekrar içe aktarın.".to_string()),
        technical_details: Some(pdf_path.to_string_lossy().to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })
}

fn extract_page_number(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_str()?;
    stem.rsplit(|ch: char| !ch.is_ascii_digit())
        .next()
        .and_then(|value| value.parse::<u32>().ok())
}

pub fn find_binary(name: &str) -> Option<PathBuf> {
    // 1. PATH search
    if let Ok(path_val) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_val) {
            let p = dir.join(name);
            if p.exists() && p.is_file() {
                return Some(p);
            }
        }
    }
    // 2. Fallbacks
    let fallbacks = [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ];
    for dir in &fallbacks {
        let p = PathBuf::from(dir).join(name);
        if p.exists() && p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn get_searched_paths() -> Vec<String> {
    let mut dirs = Vec::new();
    if let Ok(path_val) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_val) {
            dirs.push(dir);
        }
    }
    let fallbacks = [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ];
    for dir in &fallbacks {
        let path_buf = PathBuf::from(dir);
        if !dirs.contains(&path_buf) {
            dirs.push(path_buf);
        }
    }

    let mut paths = Vec::new();
    for dir in dirs {
        paths.push(dir.join("pdfinfo").to_string_lossy().into_owned());
        paths.push(dir.join("pdftoppm").to_string_lossy().into_owned());
    }
    paths
}

pub fn configure_command(binary_name: &str) -> Command {
    let binary_path = find_binary(binary_name).unwrap_or_else(|| PathBuf::from(binary_name));
    let mut cmd = Command::new(binary_path);

    // Prepend common directories to PATH
    let new_paths = vec![
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ];
    let mut paths =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    for p in new_paths {
        let path_buf = PathBuf::from(p);
        if !paths.contains(&path_buf) {
            paths.insert(0, path_buf);
        }
    }
    if let Ok(new_path_env) = std::env::join_paths(paths) {
        cmd.env("PATH", new_path_env);
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_binary_nonexistent() {
        let result = find_binary("non_existent_binary_abc_123");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_searched_paths_not_empty() {
        let paths = get_searched_paths();
        assert!(!paths.is_empty());
        assert!(paths.iter().any(|p| p.contains("pdfinfo")));
        assert!(paths.iter().any(|p| p.contains("pdftoppm")));
    }

    #[test]
    fn test_configure_command_sets_path() {
        let cmd = configure_command("pdfinfo");
        let envs = cmd.get_envs().collect::<Vec<_>>();
        let path_var = envs.iter().find(|(k, _)| k == &"PATH");
        assert!(path_var.is_some());
    }

    #[test]
    fn test_get_renderer_status() {
        let service = SystemPdfService;
        let status = service.get_renderer_status().expect("renderer status");
        assert!(!status.searched_paths.is_empty());
        assert!(status.available);
        assert!(status
            .pdfinfo_path
            .as_deref()
            .is_some_and(|path| path.contains("pdfinfo")));
        assert!(status
            .pdftoppm_path
            .as_deref()
            .is_some_and(|path| path.contains("pdftoppm")));
    }

    #[test]
    fn test_system_pdf_service_render_all_pages() {
        use uuid::Uuid;
        let root = std::env::temp_dir().join(format!("rubrika-v3-pdf-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();

        let pdf_path = root.join("test.pdf");
        let pdf_content = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> /MediaBox [0 0 595 842] /Contents 4 0 R >>\nendobj\n4 0 obj\n<< /Length 48 >>\nstream\nBT\n/F1 24 Tf\n100 700 Td\n(Rubrika Test PDF)\nTj\nET\nendstream\nendobj\nxref\n0 5\n0000000000 65535 f \n0000000009 00000 n \n0000000058 00000 n \n0000000115 00000 n \n0000000251 00000 n \ntrailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n349\n%%EOF\n";
        std::fs::write(&pdf_path, pdf_content).unwrap();

        let service = SystemPdfService;

        // Test page count
        let count = service.page_count(&pdf_path).expect("page count");
        assert_eq!(count, 1);

        // Test render all pages
        let output_dir = root.join("previews");
        let rendered = service
            .render_all_pages(&pdf_path, &output_dir)
            .expect("render all pages");
        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].exists());

        // Clean up
        let _ = std::fs::remove_dir_all(&root);
    }
}
