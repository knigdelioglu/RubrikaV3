use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::imageops::filter3x3;
use image::{DynamicImage, GenericImageView, GrayImage, ImageFormat, Luma};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::student::{OcrImagePreprocessDiagnostics, OcrImagePreprocessMode};
use crate::platform::project_paths::TrustedProjectRoot;

const OCR_PREPROCESS_VERSION: &str = "ocr_image_preprocess_v2";
const CLEAN_GRAY_BACKGROUND_RADIUS: f32 = 24.0;
const CLEAN_GRAY_CONTRAST_GAIN: f32 = 1.18;
const HANDWRITING_BACKGROUND_RADIUS: f32 = 28.0;
const HANDWRITING_CONTRAST_GAIN: f32 = 1.34;
const HANDWRITING_GAMMA: f32 = 1.10;
const HANDWRITING_CLIP_LOW: f32 = 0.015;
const HANDWRITING_CLIP_HIGH: f32 = 0.992;
const HIGH_CONTRAST_BACKGROUND_RADIUS: f32 = 18.0;
const HIGH_CONTRAST_GAIN: f32 = 1.48;
const HIGH_CONTRAST_GAMMA: f32 = 1.16;
const HIGH_CONTRAST_CLIP_LOW: f32 = 0.030;
const HIGH_CONTRAST_CLIP_HIGH: f32 = 0.985;
const BW_BACKGROUND_RADIUS: f32 = 18.0;
const BW_CONTRAST_GAIN: f32 = 1.22;
const BW_LOCAL_RADIUS: u32 = 12;
const BW_THRESHOLD_OFFSET: f32 = 11.0;
const DENOISE_KERNEL: [f32; 9] = [1.0 / 9.0; 9];
const SHARPEN_KERNEL_MILD: [f32; 9] = [0.0, -0.75, 0.0, -0.75, 4.0, -0.75, 0.0, -0.75, 0.0];

#[derive(Clone, Default)]
pub struct OcrImagePreprocessService;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrImagePreprocessResult {
    pub output_image_path: String,
    pub diagnostics: OcrImagePreprocessDiagnostics,
}

impl OcrImagePreprocessService {
    pub fn preprocess_image(
        &self,
        project_root: &Path,
        input_image_path: &Path,
        mode: OcrImagePreprocessMode,
    ) -> Result<OcrImagePreprocessResult, AppError> {
        let trusted_root =
            TrustedProjectRoot::from_canonical_root(project_root.to_path_buf(), false)?;
        if !input_image_path.exists() {
            return Err(app_error(
                AppErrorCode::FileReadFailed,
                "OCR görüntüsü bulunamadı.",
                Some(input_image_path.to_string_lossy().to_string()),
                Some("OCR crop cache must exist before preprocessing.".to_string()),
            ));
        }
        let input_managed = trusted_root.relative_for_existing(input_image_path)?;
        let input_image_path = trusted_root.resolve_existing_file(&input_managed)?;
        if mode == OcrImagePreprocessMode::Original {
            return self.build_original_result(&input_image_path);
        }

        let source_metadata = fs::metadata(&input_image_path).map_err(|error| {
            app_error(
                AppErrorCode::FileReadFailed,
                "OCR görüntüsü bulunamadı.",
                Some(error.to_string()),
                Some("OCR crop cache must exist before preprocessing.".to_string()),
            )
        })?;
        let source_bytes = source_metadata.len();
        let source_content = fs::read(&input_image_path).map_err(|error| {
            app_error(
                AppErrorCode::FileReadFailed,
                "OCR görüntüsü okunamadı.",
                Some(error.to_string()),
                Some("OCR crop cache must exist before preprocessing.".to_string()),
            )
        })?;
        let source_image = image::open(&input_image_path).map_err(|error| {
            app_error(
                AppErrorCode::OcrFailed,
                "OCR görüntüsü açılamadı.",
                Some(error.to_string()),
                Some("Rebuild the crop cache and try again.".to_string()),
            )
        })?;
        let (source_width, source_height) = source_image.dimensions();
        let cache_path = preprocess_cache_path(
            &trusted_root,
            &input_image_path,
            &mode,
            stable_preprocess_hash(&source_content),
            OCR_PREPROCESS_VERSION,
        )?;

        if cache_path.exists() {
            let cache_managed = trusted_root.managed_for_path(&cache_path)?;
            let cache_path = trusted_root.resolve_existing_file(&cache_managed)?;
            let (output_width, output_height) =
                image::image_dimensions(&cache_path).map_err(|error| {
                    app_error(
                        AppErrorCode::OcrFailed,
                        "Önbelleğe alınmış preprocess görüntüsü okunamadı.",
                        Some(error.to_string()),
                        Some("Delete the cached preprocess image and try again.".to_string()),
                    )
                })?;
            let output_bytes = fs::metadata(&cache_path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            return Ok(OcrImagePreprocessResult {
                output_image_path: cache_path.to_string_lossy().to_string(),
                diagnostics: OcrImagePreprocessDiagnostics {
                    mode,
                    preprocess_version: OCR_PREPROCESS_VERSION.to_string(),
                    source_image_path: input_image_path.to_string_lossy().to_string(),
                    output_image_path: cache_path.to_string_lossy().to_string(),
                    source_width,
                    source_height,
                    output_width,
                    output_height,
                    source_bytes,
                    output_bytes,
                    cache_hit: true,
                    applied: true,
                    warnings: vec![],
                    error_message: None,
                    technical_details: None,
                },
            });
        }

        if let Some(parent) = cache_path.parent() {
            trusted_root.ensure_managed_directory(parent)?;
        }

        let preprocessed = match mode {
            OcrImagePreprocessMode::HandwritingEnhanced => preprocess_gray(
                &source_image,
                PreprocessGrayConfig {
                    background_radius: HANDWRITING_BACKGROUND_RADIUS,
                    contrast_gain: HANDWRITING_CONTRAST_GAIN,
                    gamma: HANDWRITING_GAMMA,
                    denoise: true,
                    sharpen: true,
                    autocontrast: true,
                    clip_range: Some((HANDWRITING_CLIP_LOW, HANDWRITING_CLIP_HIGH)),
                },
            ),
            OcrImagePreprocessMode::CleanGrayscale => preprocess_gray(
                &source_image,
                PreprocessGrayConfig {
                    background_radius: CLEAN_GRAY_BACKGROUND_RADIUS,
                    contrast_gain: CLEAN_GRAY_CONTRAST_GAIN,
                    gamma: 1.0,
                    denoise: true,
                    sharpen: true,
                    autocontrast: true,
                    clip_range: None,
                },
            ),
            OcrImagePreprocessMode::HighContrast => preprocess_gray(
                &source_image,
                PreprocessGrayConfig {
                    background_radius: HIGH_CONTRAST_BACKGROUND_RADIUS,
                    contrast_gain: HIGH_CONTRAST_GAIN,
                    gamma: HIGH_CONTRAST_GAMMA,
                    denoise: false,
                    sharpen: true,
                    autocontrast: true,
                    clip_range: Some((HIGH_CONTRAST_CLIP_LOW, HIGH_CONTRAST_CLIP_HIGH)),
                },
            ),
            OcrImagePreprocessMode::HighContrastBw => preprocess_high_contrast_bw(
                &source_image,
                BW_BACKGROUND_RADIUS,
                BW_CONTRAST_GAIN,
                BW_LOCAL_RADIUS,
                BW_THRESHOLD_OFFSET,
            ),
            OcrImagePreprocessMode::Original => unreachable!(),
        };

        let output_image = DynamicImage::ImageLuma8(preprocessed);
        write_png_atomic(&trusted_root, &cache_path, &output_image)?;
        let (output_width, output_height) =
            image::image_dimensions(&cache_path).map_err(|error| {
                app_error(
                    AppErrorCode::OcrFailed,
                    "Preprocess çıktısı doğrulanamadı.",
                    Some(error.to_string()),
                    Some("Delete the cached preprocess image and try again.".to_string()),
                )
            })?;
        let output_bytes = fs::metadata(&cache_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        Ok(OcrImagePreprocessResult {
            output_image_path: cache_path.to_string_lossy().to_string(),
            diagnostics: OcrImagePreprocessDiagnostics {
                mode,
                preprocess_version: OCR_PREPROCESS_VERSION.to_string(),
                source_image_path: input_image_path.to_string_lossy().to_string(),
                output_image_path: cache_path.to_string_lossy().to_string(),
                source_width,
                source_height,
                output_width,
                output_height,
                source_bytes,
                output_bytes,
                cache_hit: false,
                applied: true,
                warnings: vec![],
                error_message: None,
                technical_details: None,
            },
        })
    }

    fn build_original_result(
        &self,
        input_image_path: &Path,
    ) -> Result<OcrImagePreprocessResult, AppError> {
        let source_metadata = fs::metadata(input_image_path).map_err(|error| {
            app_error(
                AppErrorCode::FileReadFailed,
                "OCR görüntüsü bulunamadı.",
                Some(error.to_string()),
                Some("OCR crop cache must exist before preprocessing.".to_string()),
            )
        })?;
        let (source_width, source_height) =
            image::image_dimensions(input_image_path).map_err(|error| {
                app_error(
                    AppErrorCode::OcrFailed,
                    "OCR görüntüsü açılamadı.",
                    Some(error.to_string()),
                    Some("Rebuild the crop cache and try again.".to_string()),
                )
            })?;
        let source_bytes = source_metadata.len();
        Ok(OcrImagePreprocessResult {
            output_image_path: input_image_path.to_string_lossy().to_string(),
            diagnostics: OcrImagePreprocessDiagnostics {
                mode: OcrImagePreprocessMode::Original,
                preprocess_version: OCR_PREPROCESS_VERSION.to_string(),
                source_image_path: input_image_path.to_string_lossy().to_string(),
                output_image_path: input_image_path.to_string_lossy().to_string(),
                source_width,
                source_height,
                output_width: source_width,
                output_height: source_height,
                source_bytes,
                output_bytes: source_bytes,
                cache_hit: false,
                applied: false,
                warnings: vec![],
                error_message: None,
                technical_details: None,
            },
        })
    }
}

struct PreprocessGrayConfig {
    background_radius: f32,
    contrast_gain: f32,
    gamma: f32,
    denoise: bool,
    sharpen: bool,
    autocontrast: bool,
    clip_range: Option<(f32, f32)>,
}

fn preprocess_gray(source_image: &DynamicImage, config: PreprocessGrayConfig) -> GrayImage {
    let grayscale = source_image.grayscale();
    let base = if config.denoise {
        filter3x3(&grayscale.to_luma8(), &DENOISE_KERNEL)
    } else {
        grayscale.to_luma8()
    };
    let background = grayscale.blur(config.background_radius).to_luma8();
    let (width, height) = base.dimensions();
    let mut normalized = GrayImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let signal = base.get_pixel(x, y).0[0] as f32;
            let bg = background.get_pixel(x, y).0[0].max(1) as f32;
            let lifted = ((signal / bg) * 175.0 + 24.0).clamp(0.0, 255.0);
            let contrasted = ((lifted - 128.0) * config.contrast_gain + 128.0).clamp(0.0, 255.0);
            normalized.put_pixel(x, y, Luma([contrasted.round() as u8]));
        }
    }

    let normalized = if config.autocontrast {
        apply_percentile_autocontrast(&normalized, config.clip_range.unwrap_or((0.02, 0.98)))
    } else {
        normalized
    };
    let normalized = apply_gamma(&normalized, config.gamma);

    if config.sharpen {
        filter3x3(&normalized, &SHARPEN_KERNEL_MILD)
    } else {
        normalized
    }
}

fn preprocess_high_contrast_bw(
    source_image: &DynamicImage,
    background_radius: f32,
    contrast_gain: f32,
    local_radius: u32,
    threshold_offset: f32,
) -> GrayImage {
    let grayscale = source_image.grayscale();
    let base = filter3x3(&grayscale.to_luma8(), &DENOISE_KERNEL);
    let background = grayscale.blur(background_radius).to_luma8();
    let local_mean = grayscale.blur(local_radius as f32).to_luma8();
    let (width, height) = base.dimensions();
    let mut normalized = GrayImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let signal = base.get_pixel(x, y).0[0] as f32;
            let bg = background.get_pixel(x, y).0[0].max(1) as f32;
            let local = local_mean.get_pixel(x, y).0[0] as f32;
            let lifted = ((signal / bg) * 175.0 + 24.0).clamp(0.0, 255.0);
            let contrasted = ((lifted - 128.0) * contrast_gain + 128.0).clamp(0.0, 255.0);
            let adaptive_threshold = (local - threshold_offset).clamp(88.0, 236.0);
            let value = if contrasted <= adaptive_threshold {
                0
            } else {
                255
            };
            normalized.put_pixel(x, y, Luma([value]));
        }
    }

    normalized
}

fn apply_gamma(image: &GrayImage, gamma: f32) -> GrayImage {
    if (gamma - 1.0).abs() < f32::EPSILON {
        return image.clone();
    }
    let mut output = GrayImage::new(image.width(), image.height());
    for y in 0..image.height() {
        for x in 0..image.width() {
            let value = image.get_pixel(x, y).0[0] as f32 / 255.0;
            let adjusted = value.powf(gamma).clamp(0.0, 1.0);
            output.put_pixel(x, y, Luma([(adjusted * 255.0).round() as u8]));
        }
    }
    output
}

fn apply_percentile_autocontrast(image: &GrayImage, clip_range: (f32, f32)) -> GrayImage {
    let mut histogram = [0u32; 256];
    for pixel in image.pixels() {
        histogram[pixel.0[0] as usize] += 1;
    }
    let total = (image.width() as u64 * image.height() as u64).max(1) as f32;
    let low_target = (total * clip_range.0).round() as u32;
    let high_target = (total * clip_range.1).round() as u32;
    let mut cumulative = 0u32;
    let mut low = 0u8;
    for (index, count) in histogram.iter().enumerate() {
        cumulative = cumulative.saturating_add(*count);
        if cumulative >= low_target {
            low = index as u8;
            break;
        }
    }
    cumulative = 0;
    let mut high = 255u8;
    for (index, count) in histogram.iter().enumerate().rev() {
        cumulative = cumulative.saturating_add(*count);
        if cumulative >= (total as u32).saturating_sub(high_target) {
            high = index as u8;
            break;
        }
    }
    if high <= low {
        return image.clone();
    }
    let span = (high - low).max(1) as f32;
    let mut output = GrayImage::new(image.width(), image.height());
    for y in 0..image.height() {
        for x in 0..image.width() {
            let value = image.get_pixel(x, y).0[0];
            let adjusted = if value <= low {
                0
            } else if value >= high {
                255
            } else {
                (((value - low) as f32 / span) * 255.0).round() as u8
            };
            output.put_pixel(x, y, Luma([adjusted]));
        }
    }
    output
}

fn preprocess_cache_path(
    trusted_root: &TrustedProjectRoot,
    input_image_path: &Path,
    mode: &OcrImagePreprocessMode,
    content_hash: u64,
    preprocess_version: &str,
) -> Result<PathBuf, AppError> {
    let mode_dir = preprocess_mode_dir_name(mode);
    let fingerprint =
        stable_preprocess_fingerprint(input_image_path, mode, content_hash, preprocess_version);
    let managed = trusted_root.managed(&format!(
        "cache/preprocessed/{preprocess_version}/{mode_dir}/{fingerprint}.png"
    ))?;
    Ok(trusted_root.root().join(managed.as_path()))
}

fn preprocess_mode_dir_name(mode: &OcrImagePreprocessMode) -> &'static str {
    match mode {
        OcrImagePreprocessMode::Original => "original",
        OcrImagePreprocessMode::CleanGrayscale => "clean_grayscale",
        OcrImagePreprocessMode::HandwritingEnhanced => "handwriting_enhanced",
        OcrImagePreprocessMode::HighContrast => "high_contrast",
        OcrImagePreprocessMode::HighContrastBw => "high_contrast_bw",
    }
}

fn stable_preprocess_fingerprint(
    input_image_path: &Path,
    mode: &OcrImagePreprocessMode,
    content_hash: u64,
    preprocess_version: &str,
) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input_image_path.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for byte in preprocess_mode_dir_name(mode).as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for byte in preprocess_version.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for byte in content_hash.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn stable_preprocess_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn write_png_atomic(
    trusted_root: &TrustedProjectRoot,
    path: &Path,
    image: &DynamicImage,
) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(|| {
        app_error(
            AppErrorCode::FileWriteFailed,
            "OCR preprocess cache yolu geçersiz.",
            None,
            Some("The preprocess output path must have a parent directory.".to_string()),
        )
    })?;
    trusted_root.ensure_managed_directory(parent)?;
    let mut bytes = Vec::new();
    {
        let mut cursor = Cursor::new(&mut bytes);
        image
            .write_to(&mut cursor, ImageFormat::Png)
            .map_err(|error| {
                app_error(
                    AppErrorCode::OcrFailed,
                    "Preprocess çıktısı kaydedilemedi.",
                    Some(error.to_string()),
                    Some("Delete the cached preprocess image and try again.".to_string()),
                )
            })?;
    }
    let managed = trusted_root.managed_for_path(path)?;
    trusted_root.atomic_write_bytes(&managed, &bytes)
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
    use image::{ImageBuffer, Rgba};
    use std::fs;

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("rubrika-preprocess-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn original_mode_returns_input_path() {
        let root = temp_root();
        let image_path = root.join("input.png");
        let image = ImageBuffer::from_pixel(8, 8, Rgba([240, 240, 240, 255]));
        DynamicImage::ImageRgba8(image).save(&image_path).unwrap();

        let service = OcrImagePreprocessService;
        let result = service
            .preprocess_image(&root, &image_path, OcrImagePreprocessMode::Original)
            .unwrap();

        assert_eq!(
            result.output_image_path,
            std::fs::canonicalize(&image_path)
                .unwrap()
                .to_string_lossy()
        );
        assert!(!result.diagnostics.applied);
        assert_eq!(
            result.diagnostics.preprocess_version,
            OCR_PREPROCESS_VERSION
        );
    }

    #[test]
    fn clean_grayscale_produces_cached_output() {
        let root = temp_root();
        let image_path = root.join("input.png");
        let mut image = ImageBuffer::from_pixel(24, 24, Rgba([220, 220, 220, 255]));
        for y in 6..18 {
            for x in 4..20 {
                image.put_pixel(x, y, Rgba([40, 40, 40, 255]));
            }
        }
        DynamicImage::ImageRgba8(image).save(&image_path).unwrap();

        let service = OcrImagePreprocessService;
        let result = service
            .preprocess_image(&root, &image_path, OcrImagePreprocessMode::CleanGrayscale)
            .unwrap();

        assert_ne!(result.output_image_path, image_path.to_string_lossy());
        assert!(Path::new(&result.output_image_path).exists());
        assert!(result.diagnostics.applied);
        assert!(result.diagnostics.output_bytes > 0);
        assert_eq!(
            result.diagnostics.preprocess_version,
            OCR_PREPROCESS_VERSION
        );
    }

    #[test]
    fn handwriting_enhanced_produces_cached_output() {
        let root = temp_root();
        let image_path = root.join("handwriting.png");
        let mut image = ImageBuffer::from_pixel(32, 24, Rgba([242, 242, 242, 255]));
        for y in 7..17 {
            for x in 5..27 {
                image.put_pixel(x, y, Rgba([56, 56, 56, 255]));
            }
        }
        DynamicImage::ImageRgba8(image).save(&image_path).unwrap();

        let service = OcrImagePreprocessService;
        let result = service
            .preprocess_image(
                &root,
                &image_path,
                OcrImagePreprocessMode::HandwritingEnhanced,
            )
            .unwrap();

        assert_ne!(result.output_image_path, image_path.to_string_lossy());
        assert!(Path::new(&result.output_image_path).exists());
        assert!(result.diagnostics.applied);
        assert_eq!(
            result.diagnostics.mode,
            OcrImagePreprocessMode::HandwritingEnhanced
        );
        assert_eq!(
            result.diagnostics.preprocess_version,
            OCR_PREPROCESS_VERSION
        );
    }

    #[test]
    fn high_contrast_and_bw_produce_cached_output() {
        let root = temp_root();
        let image_path = root.join("contrast.png");
        let mut image = ImageBuffer::from_pixel(28, 28, Rgba([230, 230, 230, 255]));
        for y in 10..18 {
            for x in 6..22 {
                image.put_pixel(x, y, Rgba([42, 42, 42, 255]));
            }
        }
        DynamicImage::ImageRgba8(image).save(&image_path).unwrap();

        let service = OcrImagePreprocessService;
        let high_contrast = service
            .preprocess_image(&root, &image_path, OcrImagePreprocessMode::HighContrast)
            .unwrap();
        let bw = service
            .preprocess_image(&root, &image_path, OcrImagePreprocessMode::HighContrastBw)
            .unwrap();

        assert!(Path::new(&high_contrast.output_image_path).exists());
        assert!(Path::new(&bw.output_image_path).exists());
        assert_eq!(bw.diagnostics.mode, OcrImagePreprocessMode::HighContrastBw);
        assert_eq!(bw.diagnostics.preprocess_version, OCR_PREPROCESS_VERSION);
    }

    #[test]
    fn missing_input_returns_meaningful_error() {
        let root = temp_root();
        let service = OcrImagePreprocessService;
        let error = service
            .preprocess_image(
                &root,
                &root.join("missing.png"),
                OcrImagePreprocessMode::CleanGrayscale,
            )
            .unwrap_err();

        assert_eq!(error.code, AppErrorCode::FileReadFailed);
        assert!(error.message.contains("OCR görüntüsü"));
    }
}
