use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::ImageError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::{ModelInputImage, ModelInputImageKind};
use crate::platform::file_access::atomic_write;

const DEFAULT_LONG_EDGE_MAX: u32 = 1800;
const DEFAULT_JPEG_QUALITY: u8 = 88;

#[derive(Clone)]
pub struct ModelInputImageService {
    long_edge_max: u32,
    jpeg_quality: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelInputBatchMetadata {
    pub kind: ModelInputImageKind,
    pub document_id: String,
    pub created_at: String,
    pub long_edge_max: u32,
    pub jpeg_quality: u8,
    pub total_source_bytes: u64,
    pub total_output_bytes: u64,
    pub total_base64_approx_bytes: u64,
    pub images: Vec<ModelInputImage>,
}

impl ModelInputImageService {
    pub fn new(long_edge_max: u32, jpeg_quality: u8) -> Self {
        Self {
            long_edge_max,
            jpeg_quality,
        }
    }

    pub fn default_question_text() -> Self {
        Self::default()
    }

    pub fn long_edge_max(&self) -> u32 {
        self.long_edge_max
    }

    pub fn jpeg_quality(&self) -> u8 {
        self.jpeg_quality
    }

    pub fn prepare_inputs(
        &self,
        project_root: &Path,
        kind: ModelInputImageKind,
        document_id: &str,
        sources: &[(u32, PathBuf)],
    ) -> Result<Vec<ModelInputImage>, AppError> {
        let output_dir = model_inputs_dir(project_root, &kind, document_id);
        std::fs::create_dir_all(&output_dir).map_err(|error| {
            app_error(
                AppErrorCode::FileWriteFailed,
                "Model giriş klasörü oluşturulamadı.",
                Some(error.to_string()),
                Some(
                    "Project cache permissions should allow writing model input images."
                        .to_string(),
                ),
            )
        })?;

        let created_at = chrono::Utc::now().to_rfc3339();
        let mut images = Vec::with_capacity(sources.len());
        let mut total_source_bytes = 0u64;
        let mut total_output_bytes = 0u64;
        let mut total_base64_approx_bytes = 0u64;

        for (page_number, source_path) in sources {
            let metadata = prepare_single_image(PrepareSingleImageInput {
                output_dir: &output_dir,
                kind: kind.clone(),
                document_id,
                page_number: *page_number,
                source_path,
                long_edge_max: self.long_edge_max,
                jpeg_quality: self.jpeg_quality,
                created_at: &created_at,
            })?;
            total_source_bytes += metadata.source_bytes;
            total_output_bytes += metadata.output_bytes;
            total_base64_approx_bytes += metadata.base64_approx_bytes;
            images.push(metadata);
        }

        let manifest = ModelInputBatchMetadata {
            kind,
            document_id: document_id.to_string(),
            created_at,
            long_edge_max: self.long_edge_max,
            jpeg_quality: self.jpeg_quality,
            total_source_bytes,
            total_output_bytes,
            total_base64_approx_bytes,
            images: images.clone(),
        };
        write_manifest(&output_dir.join("model_inputs.json"), &manifest)?;
        Ok(images)
    }

    pub fn manifest_path(
        project_root: &Path,
        kind: &ModelInputImageKind,
        document_id: &str,
    ) -> PathBuf {
        model_inputs_dir(project_root, kind, document_id).join("model_inputs.json")
    }

    pub fn load_manifests(project_root: &Path) -> Result<Vec<ModelInputBatchMetadata>, AppError> {
        let root = project_root.join("cache").join("model_inputs");
        if !root.exists() {
            return Ok(vec![]);
        }

        let mut manifests = Vec::new();
        for kind_entry in std::fs::read_dir(&root).map_err(|error| {
            app_error(
                AppErrorCode::FileReadFailed,
                "Model input cache okunamadı.",
                Some(error.to_string()),
                Some("Check project cache permissions.".to_string()),
            )
        })? {
            let kind_entry = kind_entry.map_err(|error| {
                app_error(
                    AppErrorCode::FileReadFailed,
                    "Model input cache okunamadı.",
                    Some(error.to_string()),
                    Some("Check project cache permissions.".to_string()),
                )
            })?;
            if !kind_entry.path().is_dir() {
                continue;
            }
            for doc_entry in std::fs::read_dir(kind_entry.path()).map_err(|error| {
                app_error(
                    AppErrorCode::FileReadFailed,
                    "Model input cache okunamadı.",
                    Some(error.to_string()),
                    Some("Check project cache permissions.".to_string()),
                )
            })? {
                let doc_entry = doc_entry.map_err(|error| {
                    app_error(
                        AppErrorCode::FileReadFailed,
                        "Model input cache okunamadı.",
                        Some(error.to_string()),
                        Some("Check project cache permissions.".to_string()),
                    )
                })?;
                let manifest_path = doc_entry.path().join("model_inputs.json");
                if !manifest_path.exists() {
                    continue;
                }
                let content = std::fs::read_to_string(&manifest_path).map_err(|error| {
                    app_error(
                        AppErrorCode::FileReadFailed,
                        "Model input metadata okunamadı.",
                        Some(error.to_string()),
                        Some("Open the manifest file again.".to_string()),
                    )
                })?;
                let manifest: ModelInputBatchMetadata =
                    serde_json::from_str(&content).map_err(|error| {
                        app_error(
                            AppErrorCode::ProjectLoadFailed,
                            "Model input metadata bozuk.",
                            Some(error.to_string()),
                            Some("Rebuild the model input cache.".to_string()),
                        )
                    })?;
                manifests.push(manifest);
            }
        }

        manifests.sort_by(|a, b| {
            a.document_id
                .cmp(&b.document_id)
                .then(a.created_at.cmp(&b.created_at))
        });
        Ok(manifests)
    }
}

impl Default for ModelInputImageService {
    fn default() -> Self {
        Self::new(DEFAULT_LONG_EDGE_MAX, DEFAULT_JPEG_QUALITY)
    }
}

struct PrepareSingleImageInput<'a> {
    output_dir: &'a Path,
    kind: ModelInputImageKind,
    document_id: &'a str,
    page_number: u32,
    source_path: &'a Path,
    long_edge_max: u32,
    jpeg_quality: u8,
    created_at: &'a str,
}

fn prepare_single_image(input: PrepareSingleImageInput<'_>) -> Result<ModelInputImage, AppError> {
    let PrepareSingleImageInput {
        output_dir,
        kind,
        document_id,
        page_number,
        source_path,
        long_edge_max,
        jpeg_quality,
        created_at,
    } = input;
    if !source_path.exists() {
        return Err(app_error(
            AppErrorCode::FileReadFailed,
            "Model girdisi için kaynak görüntü bulunamadı.",
            Some(source_path.to_string_lossy().to_string()),
            Some("Preview or rendered page image is missing.".to_string()),
        ));
    }

    let source_bytes = std::fs::metadata(source_path)
        .map_err(|error| {
            app_error(
                AppErrorCode::FileReadFailed,
                "Kaynak görüntü okunamadı.",
                Some(error.to_string()),
                Some("Check the source image path.".to_string()),
            )
        })?
        .len();
    let (source_width, source_height) = image::image_dimensions(source_path)
        .map_err(map_image_error("Kaynak görüntü boyutları okunamadı."))?;
    let image = image::open(source_path).map_err(map_image_error("Kaynak görüntü açılamadı."))?;
    let (output_image, output_width, output_height) =
        resize_image(image, source_width, source_height, long_edge_max);
    let output_path = output_dir.join(format!("page_{page_number:03}.jpg"));

    let file = File::create(&output_path).map_err(|error| {
        app_error(
            AppErrorCode::FileWriteFailed,
            "Model giriş JPEG dosyası oluşturulamadı.",
            Some(error.to_string()),
            Some("Check project cache permissions.".to_string()),
        )
    })?;
    let mut writer = BufWriter::new(file);
    let mut encoder = JpegEncoder::new_with_quality(&mut writer, jpeg_quality);
    encoder
        .encode_image(&output_image)
        .map_err(map_image_error("Model giriş JPEG dosyası yazılamadı."))?;
    let output_bytes = std::fs::metadata(&output_path)
        .map_err(|error| {
            app_error(
                AppErrorCode::FileReadFailed,
                "Model giriş JPEG boyutu okunamadı.",
                Some(error.to_string()),
                Some("Check the generated image file.".to_string()),
            )
        })?
        .len();

    Ok(ModelInputImage {
        kind,
        document_id: document_id.to_string(),
        page_number,
        source_image_path: source_path.to_string_lossy().to_string(),
        output_image_path: output_path.to_string_lossy().to_string(),
        source_width,
        source_height,
        output_width,
        output_height,
        source_bytes,
        output_bytes,
        base64_approx_bytes: base64_approx_bytes(output_bytes),
        long_edge_max,
        jpeg_quality,
        created_at: created_at.to_string(),
    })
}

fn resize_image(
    image: image::DynamicImage,
    source_width: u32,
    source_height: u32,
    long_edge_max: u32,
) -> (image::DynamicImage, u32, u32) {
    let long_edge = source_width.max(source_height);
    if long_edge <= long_edge_max {
        return (image, source_width, source_height);
    }

    let scale = long_edge_max as f32 / long_edge as f32;
    let output_width = ((source_width as f32) * scale).round().max(1.0) as u32;
    let output_height = ((source_height as f32) * scale).round().max(1.0) as u32;
    let resized = image.resize(output_width, output_height, FilterType::Lanczos3);
    (resized, output_width, output_height)
}

fn base64_approx_bytes(bytes: u64) -> u64 {
    if bytes == 0 {
        return 0;
    }
    bytes.div_ceil(3) * 4
}

fn model_inputs_dir(project_root: &Path, kind: &ModelInputImageKind, document_id: &str) -> PathBuf {
    project_root
        .join("cache")
        .join("model_inputs")
        .join(kind_folder(kind))
        .join(document_id)
}

fn kind_folder(kind: &ModelInputImageKind) -> &'static str {
    match kind {
        ModelInputImageKind::QuestionText => "question_text",
        ModelInputImageKind::Rubric => "rubric",
        ModelInputImageKind::StudentOcr => "student_ocr",
        ModelInputImageKind::StudentIdentityOcr => "student_identity_ocr",
        ModelInputImageKind::StudentAnswerOcrIssueCorrection => "student_ocr_issue_correction",
    }
}

fn write_manifest(
    manifest_path: &Path,
    manifest: &ModelInputBatchMetadata,
) -> Result<(), AppError> {
    let content = serde_json::to_string_pretty(manifest).map_err(|error| {
        app_error(
            AppErrorCode::ProjectSaveFailed,
            "Model input metadata serialize edilemedi.",
            Some(error.to_string()),
            None,
        )
    })?;
    atomic_write(manifest_path, &content).map_err(|error| {
        app_error(
            AppErrorCode::ProjectSaveFailed,
            "Model input metadata yazılamadı.",
            Some(error.to_string()),
            Some("Check model input cache permissions.".to_string()),
        )
    })
}

fn map_image_error(message: &'static str) -> impl FnOnce(ImageError) -> AppError {
    move |error| {
        app_error(
            AppErrorCode::PdfRenderFailed,
            message,
            Some(error.to_string()),
            Some("Inspect the source PDF image cache.".to_string()),
        )
    }
}

fn app_error(
    code: AppErrorCode,
    message: &str,
    technical_details: Option<String>,
    suggested_action: Option<String>,
) -> AppError {
    AppError {
        code,
        message: message.to_string(),
        recoverable: true,
        suggested_action,
        technical_details,
        correlation_id: Uuid::new_v4().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("rubrika-inputs-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_test_png(path: &Path, width: u32, height: u32) {
        let image = image::RgbImage::from_fn(width, height, |_x, _y| image::Rgb([120, 80, 40]));
        image.save(path).unwrap();
    }

    #[test]
    fn large_image_is_resized_and_manifest_is_written() {
        let root = temp_root();
        let source = root.join("source.png");
        write_test_png(&source, 3000, 2000);

        let service = ModelInputImageService::default();
        let images = service
            .prepare_inputs(
                &root,
                ModelInputImageKind::QuestionText,
                "doc-1",
                &[(1, source.clone())],
            )
            .unwrap();

        assert_eq!(images.len(), 1);
        assert!(images[0].output_width <= 1800);
        assert!(images[0].output_height <= 1800);
        assert!(Path::new(&images[0].output_image_path).exists());
        assert!(ModelInputImageService::manifest_path(
            &root,
            &ModelInputImageKind::QuestionText,
            "doc-1"
        )
        .exists());
    }

    #[test]
    fn small_image_is_not_upscaled() {
        let root = temp_root();
        let source = root.join("source-small.png");
        write_test_png(&source, 600, 800);

        let service = ModelInputImageService::default();
        let images = service
            .prepare_inputs(
                &root,
                ModelInputImageKind::Rubric,
                "doc-2",
                &[(1, source.clone())],
            )
            .unwrap();

        assert_eq!(images[0].output_width, 600);
        assert_eq!(images[0].output_height, 800);
        assert!(images[0].output_bytes > 0);
        assert!(images[0].base64_approx_bytes >= images[0].output_bytes);
    }
}
