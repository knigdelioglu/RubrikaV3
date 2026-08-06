//! Pure image-geometry helpers for the OCR pipeline — Faz 7 (TD-21).
//!
//! Deskew, registration deviation measurement and DPI normalization are pure
//! functions: image/bounding data goes in, an image or a measurement comes out.
//! No model call and no file I/O happens here; callers own the cache paths.
//!
//! Design rules (mirrored from `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` TD-21):
//! - Small skew (-3°..+3°) is corrected; a skew at/above 8° rejects the input
//!   with a typed `DeskewOutOfRange` error instead of a silent "fix".
//! - Registration deviation is measured against the expected answer-region
//!   grid (the same normalized-bbox math the crop service uses); a deviation
//!   above the threshold rejects the input with `RegistrationOutOfRange`.
//! - OCR rendering normalizes to a fixed target DPI (300) at the source; the
//!   provided pure functions validate and compute that normalization.

use image::{DynamicImage, GrayImage, Luma};
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::student::NormalizedBBox;
use crate::services::student_answer_crop_service::crop_rect_normalized;

/// OCR render target DPI (fixed). Crop/preview renders are normalized toward
/// this value before OCR; see `normalize_dpi`.
pub const OCR_RENDER_TARGET_DPI: u32 = 300;
/// Lowest accepted render DPI for OCR inputs.
pub const OCR_MIN_ACCEPTED_DPI: u32 = 96;
/// Highest accepted render DPI for OCR inputs.
pub const OCR_MAX_ACCEPTED_DPI: u32 = 600;

/// A skew of this magnitude or more rejects the input (typed error).
pub const DESKEW_MAX_ACCEPTED_ANGLE: f32 = 8.0;
/// Default declared deskew operating range for callers.
pub const DESKEW_DEFAULT_MAX_ANGLE: f32 = 3.0;
/// Skew below this magnitude is treated as "already level" (no-op).
pub const DESKEW_APPLY_MIN_ANGLE: f32 = 0.1;

/// Default registration deviation threshold (fraction of the page diagonal).
pub const DEFAULT_MAX_REGISTRATION_DEVIATION: f32 = 0.12;

/// Scanned ink threshold: pixels darker than this count as content.
const INK_THRESHOLD: u8 = 200;

// ---------------------------------------------------------------------------
// DPI normalization
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct DpiNormalization {
    pub source_dpi: u32,
    pub target_dpi: u32,
    pub scale_factor: f32,
    pub output_width: u32,
    pub output_height: u32,
    /// True when the source and target DPI differ by more than a rounding
    /// epsilon (i.e. the caller should rescale).
    pub adjusted: bool,
}

/// Converts a PDF render scale into a DPI value (PDF base is 72pt).
pub fn render_scale_to_dpi(scale: f32) -> u32 {
    (scale * 72.0).round().max(1.0) as u32
}

/// Validates that a render DPI is within the accepted OCR range.
pub fn validate_dpi_in_range(dpi: u32, min_dpi: u32, max_dpi: u32) -> bool {
    (min_dpi..=max_dpi).contains(&dpi)
}

/// Pure normalization: computes the scale factor and target dimensions needed
/// to bring `source_dpi` up (or down) to `target_dpi` at `width`x`height`.
pub fn normalize_dpi(
    source_dpi: u32,
    target_dpi: u32,
    width: u32,
    height: u32,
) -> DpiNormalization {
    let source = source_dpi.max(1) as f32;
    let target = target_dpi.max(1) as f32;
    let scale_factor = target / source;
    let output_width = ((width as f32) * scale_factor).round().max(1.0) as u32;
    let output_height = ((height as f32) * scale_factor).round().max(1.0) as u32;
    let adjusted = (scale_factor - 1.0).abs() > 0.001;
    DpiNormalization {
        source_dpi,
        target_dpi,
        scale_factor,
        output_width,
        output_height,
        adjusted,
    }
}

// ---------------------------------------------------------------------------
// Deskew
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct DeskewResult {
    /// Estimated skew angle in degrees (the correction applied, positive =
    /// content tilted clockwise and rotated counter-clockwise to level).
    pub angle_degrees: f32,
    /// True when a rotation was actually applied.
    pub applied: bool,
    pub image: DynamicImage,
}

/// Estimates the skew angle (in degrees) of a grayscale page/crop.
///
/// Uses the projection-profile method: for each candidate rotation the image is
/// rotated around its center and the variance of the per-row ink profile is
/// measured. Text rows produce the sharpest profile peaks when level, so the
/// angle maximizing the variance is the correction angle. The scan is coarse
/// first (±12°, 2° steps) then refined (0.25° steps) around the best coarse
/// value, so gross skew can be detected without scanning every fine angle.
pub fn estimate_skew_angle(image: &GrayImage) -> f32 {
    let work = downscale_for_skew(image);
    let coarse = best_projection_angle(&work, -12.0, 12.0, 2.0);
    let lo = (coarse - 2.0).max(-12.0);
    let hi = (coarse + 2.0).min(12.0);
    best_projection_angle(&work, lo, hi, 0.25)
}

/// Deskews an OCR input.
///
/// - `|angle| >= 8°` → `DeskewOutOfRange` (input rejected, no silent fix).
/// - `|angle| > max_angle_degrees` → `DeskewOutOfRange` (outside the declared
///   operating range).
/// - `|angle| < DESKEW_APPLY_MIN_ANGLE` → no-op (`applied = false`).
/// - otherwise the image is rotated by the estimated angle.
pub fn deskew_image(
    image: &DynamicImage,
    max_angle_degrees: f32,
) -> Result<DeskewResult, AppError> {
    let gray = image.grayscale().to_luma8();
    let angle = estimate_skew_angle(&gray);
    if angle.abs() >= DESKEW_MAX_ACCEPTED_ANGLE || angle.abs() > max_angle_degrees {
        return Err(deskew_out_of_range_error(angle, max_angle_degrees));
    }
    if angle.abs() < DESKEW_APPLY_MIN_ANGLE {
        return Ok(DeskewResult {
            angle_degrees: angle,
            applied: false,
            image: image.clone(),
        });
    }
    let rotated = DynamicImage::ImageLuma8(rotate_gray(&gray, angle));
    Ok(DeskewResult {
        angle_degrees: angle,
        applied: true,
        image: rotated,
    })
}

/// Rotates a grayscale image by `angle_degrees` around its center, keeping the
/// same canvas dimensions (out-of-bounds samples become white).
pub fn rotate_gray(image: &GrayImage, angle_degrees: f32) -> GrayImage {
    let (width, height) = image.dimensions();
    if angle_degrees.abs() < f32::EPSILON || width == 0 || height == 0 {
        return image.clone();
    }
    let theta = angle_degrees.to_radians();
    let (sin, cos) = theta.sin_cos();
    let center_x = width as f32 / 2.0;
    let center_y = height as f32 / 2.0;
    let mut output = GrayImage::new(width, height);
    let max_x = width.saturating_sub(1) as f32;
    let max_y = height.saturating_sub(1) as f32;
    for y in 0..height {
        let dy = y as f32 - center_y;
        for x in 0..width {
            let dx = x as f32 - center_x;
            let sx = center_x + dx * cos - dy * sin;
            let sy = center_y + dx * sin + dy * cos;
            if sx < 0.0 || sy < 0.0 || sx > max_x || sy > max_y {
                output.put_pixel(x, y, Luma([255]));
                continue;
            }
            let x0 = sx.floor() as u32;
            let y0 = sy.floor() as u32;
            let fx = sx - x0 as f32;
            let fy = sy - y0 as f32;
            let p00 = image.get_pixel(x0, y0).0[0] as f32;
            let p10 = image.get_pixel((x0 + 1).min(width - 1), y0).0[0] as f32;
            let p01 = image.get_pixel(x0, (y0 + 1).min(height - 1)).0[0] as f32;
            let p11 = image
                .get_pixel((x0 + 1).min(width - 1), (y0 + 1).min(height - 1))
                .0[0] as f32;
            let top = p00 + (p10 - p00) * fx;
            let bottom = p01 + (p11 - p01) * fx;
            let value = (top + (bottom - top) * fy).clamp(0.0, 255.0);
            output.put_pixel(x, y, Luma([value.round() as u8]));
        }
    }
    output
}

fn best_projection_angle(image: &GrayImage, start: f32, end: f32, step: f32) -> f32 {
    let mut best = 0.0f32;
    let mut best_score = f32::NEG_INFINITY;
    let mut angle = start;
    while angle <= end {
        let rotated = rotate_gray(image, angle);
        let score = row_profile_variance(&rotated);
        if score > best_score {
            best_score = score;
            best = angle;
        }
        angle += step;
    }
    best
}

fn row_profile_variance(image: &GrayImage) -> f32 {
    let (width, height) = image.dimensions();
    let count = (height as usize).max(1);
    let mut rows = vec![0f32; count];
    for y in 0..height {
        let mut sum = 0u64;
        for x in 0..width {
            sum += 255 - u64::from(image.get_pixel(x, y).0[0]);
        }
        rows[y as usize] = sum as f32;
    }
    let mean = rows.iter().sum::<f32>() / count as f32;
    rows.iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f32>()
        / count as f32
}

fn downscale_for_skew(image: &GrayImage) -> GrayImage {
    const MAX_DIM: u32 = 256;
    let (width, height) = image.dimensions();
    let long_edge = width.max(height);
    if long_edge <= MAX_DIM {
        return image.clone();
    }
    let scale = MAX_DIM as f32 / long_edge as f32;
    let new_width = ((width as f32) * scale).round().max(1.0) as u32;
    let new_height = ((height as f32) * scale).round().max(1.0) as u32;
    image::imageops::resize(
        image,
        new_width,
        new_height,
        image::imageops::FilterType::Triangle,
    )
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct RegistrationMeasurement {
    /// Mean ink-center offset from each expected region's center, as a fraction
    /// of the page diagonal.
    pub deviation: f32,
    /// Worst per-region deviation.
    pub max_deviation: f32,
    pub sampled_regions: usize,
    pub empty_regions: usize,
    pub total_regions: usize,
    pub per_region: Vec<f32>,
}

/// Measures how well the ink content of a rendered page aligns with the
/// expected answer-region grid.
///
/// For every expected region the region is cropped with the same normalized
/// bbox math used by the production crop service (`crop_rect_normalized`), the
/// ink bounding box inside the crop is located, and the ink-center offset from
/// the crop center is collected as a 2D vector. Handwriting naturally lands at
/// arbitrary positions inside a region, so per-region offsets scatter; a
/// *misregistered* page shifts every region in the same direction. The reported
/// `deviation` is therefore the mean offset vector (the systematic shift)
/// normalized by the page diagonal — random handwriting position cancels out
/// and only a consistent grid offset registers as a failure.
pub fn measure_registration_deviation(
    page: &GrayImage,
    expected_regions: &[NormalizedBBox],
) -> RegistrationMeasurement {
    let (page_width, page_height) = page.dimensions();
    let page_diagonal = ((page_width as f32).powi(2) + (page_height as f32).powi(2))
        .sqrt()
        .max(1.0);
    let mut per_region = Vec::with_capacity(expected_regions.len());
    let mut offset_x: Vec<f32> = Vec::with_capacity(expected_regions.len());
    let mut offset_y: Vec<f32> = Vec::with_capacity(expected_regions.len());
    let mut empty_regions = 0usize;
    for bbox in expected_regions {
        let (crop_x, crop_y, crop_w, crop_h, _clamped, _margin) =
            crop_rect_normalized(bbox, page_width, page_height);
        let crop_center_x = crop_x as f32 + crop_w as f32 / 2.0;
        let crop_center_y = crop_y as f32 + crop_h as f32 / 2.0;
        match ink_bbox_in_rect(page, crop_x, crop_y, crop_w, crop_h) {
            Some((min_x, min_y, max_x, max_y)) => {
                let ink_center_x = (min_x + max_x) as f32 / 2.0;
                let ink_center_y = (min_y + max_y) as f32 / 2.0;
                offset_x.push(ink_center_x - crop_center_x);
                offset_y.push(ink_center_y - crop_center_y);
                let distance = ((ink_center_x - crop_center_x).powi(2)
                    + (ink_center_y - crop_center_y).powi(2))
                .sqrt();
                per_region.push(distance / page_diagonal);
            }
            None => empty_regions += 1,
        }
    }
    let sampled = offset_x.len();
    let (deviation, max_deviation) = if sampled > 0 {
        let mean_x = offset_x.iter().sum::<f32>() / sampled as f32;
        let mean_y = offset_y.iter().sum::<f32>() / sampled as f32;
        let systematic = (mean_x.powi(2) + mean_y.powi(2)).sqrt() / page_diagonal;
        let max = per_region.iter().copied().fold(0.0, f32::max);
        (systematic, max)
    } else {
        (0.0, 0.0)
    };
    RegistrationMeasurement {
        deviation,
        max_deviation,
        sampled_regions: sampled,
        empty_regions,
        total_regions: expected_regions.len(),
        per_region,
    }
}

/// Rejects a registration measurement whose mean deviation exceeds
/// `max_deviation`. A page with no measurable ink content is not a registration
/// failure (blank inputs remain valid).
pub fn validate_registration(
    measurement: &RegistrationMeasurement,
    max_deviation: f32,
) -> Result<(), AppError> {
    if measurement.sampled_regions == 0 {
        return Ok(());
    }
    if measurement.deviation > max_deviation {
        return Err(registration_out_of_range_error(
            measurement.deviation,
            max_deviation,
        ));
    }
    Ok(())
}

fn ink_bbox_in_rect(
    image: &GrayImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Option<(u32, u32, u32, u32)> {
    let mut min_x: Option<u32> = None;
    let mut min_y: Option<u32> = None;
    let mut max_x: Option<u32> = None;
    let mut max_y: Option<u32> = None;
    for yy in y..y + height {
        for xx in x..x + width {
            if image.get_pixel(xx, yy).0[0] < INK_THRESHOLD {
                min_x = Some(min_x.map_or(xx, |current: u32| current.min(xx)));
                min_y = Some(min_y.map_or(yy, |current: u32| current.min(yy)));
                max_x = Some(max_x.map_or(xx, |current: u32| current.max(xx)));
                max_y = Some(max_y.map_or(yy, |current: u32| current.max(yy)));
            }
        }
    }
    match (min_x, min_y, max_x, max_y) {
        (Some(min_x), Some(min_y), Some(max_x), Some(max_y)) => Some((min_x, min_y, max_x, max_y)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

fn deskew_out_of_range_error(angle_degrees: f32, max_angle_degrees: f32) -> AppError {
    AppError {
        code: AppErrorCode::DeskewOutOfRange,
        message: "Taranan sayfa gerekli düzleştirme açısı aralığının dışında.".to_string(),
        recoverable: true,
        suggested_action: Some(
            "Sayfayı daha az eğik tarayın veya tarama kaynağını kontrol edin.".to_string(),
        ),
        technical_details: Some(format!(
            "estimated_skew_degrees={angle_degrees:.2}; max_accepted_degrees={max_angle_degrees:.2}"
        )),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

fn registration_out_of_range_error(deviation: f32, max_deviation: f32) -> AppError {
    AppError {
        code: AppErrorCode::RegistrationOutOfRange,
        message: "Taranan sayfa beklenen cevap bölgeleriyle hizalanmıyor.".to_string(),
        recoverable: true,
        suggested_action: Some(
            "Tarama hizalamasını kontrol edin veya cevap bölgesi şablonunu yeniden seçin."
                .to_string(),
        ),
        technical_details: Some(format!(
            "registration_deviation={deviation:.4}; max_deviation={max_deviation:.4}"
        )),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white_gray(width: u32, height: u32) -> GrayImage {
        GrayImage::from_pixel(width, height, Luma([255]))
    }

    /// Synthetic "printed text": dark horizontal bars spanning the width.
    fn text_bars_image(width: u32, height: u32) -> GrayImage {
        let mut image = white_gray(width, height);
        let mut y = 40;
        while y + 8 < height {
            for yy in y..(y + 8).min(height) {
                for xx in 0..width {
                    image.put_pixel(xx, yy, Luma([40]));
                }
            }
            y += 45;
        }
        image
    }

    #[test]
    fn render_scale_two_maps_to_144_dpi() {
        assert_eq!(render_scale_to_dpi(2.0), 144);
        assert_eq!(render_scale_to_dpi(1.0), 72);
    }

    #[test]
    fn normalize_dpi_upscales_low_dpi_input() {
        let result = normalize_dpi(144, OCR_RENDER_TARGET_DPI, 1191, 1684);
        assert!(result.adjusted);
        assert!((result.scale_factor - 300.0 / 144.0).abs() < 1e-3);
        assert_eq!(
            result.output_width,
            ((1191.0f32 * 300.0 / 144.0).round()) as u32
        );
        assert_eq!(
            result.output_height,
            ((1684.0f32 * 300.0 / 144.0).round()) as u32
        );
        assert_eq!(result.target_dpi, OCR_RENDER_TARGET_DPI);
    }

    #[test]
    fn normalize_dpi_is_noop_when_already_at_target() {
        let result = normalize_dpi(300, 300, 1000, 1400);
        assert!(!result.adjusted);
        assert_eq!(result.output_width, 1000);
        assert_eq!(result.output_height, 1400);
        assert_eq!(result.scale_factor, 1.0);
    }

    #[test]
    fn dpi_range_validation_accepts_ocr_renders_and_rejects_extremes() {
        assert!(validate_dpi_in_range(
            144,
            OCR_MIN_ACCEPTED_DPI,
            OCR_MAX_ACCEPTED_DPI
        ));
        assert!(validate_dpi_in_range(
            300,
            OCR_MIN_ACCEPTED_DPI,
            OCR_MAX_ACCEPTED_DPI
        ));
        assert!(!validate_dpi_in_range(
            72,
            OCR_MIN_ACCEPTED_DPI,
            OCR_MAX_ACCEPTED_DPI
        ));
        assert!(!validate_dpi_in_range(
            1200,
            OCR_MIN_ACCEPTED_DPI,
            OCR_MAX_ACCEPTED_DPI
        ));
    }

    #[test]
    fn level_text_image_has_near_zero_skew() {
        let bars = text_bars_image(400, 320);
        let angle = estimate_skew_angle(&bars);
        assert!(
            angle.abs() < 0.5,
            "level image should report ~0 skew, got {angle}"
        );
        let result =
            deskew_image(&DynamicImage::ImageLuma8(bars), DESKEW_DEFAULT_MAX_ANGLE).unwrap();
        assert!(!result.applied, "level image must be a no-op");
    }

    #[test]
    fn rotated_text_image_is_deskewed_back_to_level() {
        let bars = text_bars_image(400, 320);
        let tilted = rotate_gray(&bars, 2.0);
        let angle = estimate_skew_angle(&tilted);
        assert!(
            (angle - -2.0).abs() < 0.5,
            "tilted image should estimate ~-2 degrees, got {angle}"
        );

        let result = deskew_image(
            &DynamicImage::ImageLuma8(tilted.clone()),
            DESKEW_DEFAULT_MAX_ANGLE,
        )
        .unwrap();
        assert!(result.applied, "a 2 degree skew must be corrected");
        assert!((result.angle_degrees + 2.0).abs() < 0.5);

        let straight = result.image.grayscale().to_luma8();
        let residual = estimate_skew_angle(&straight);
        assert!(
            residual.abs() < 0.5,
            "deskewed image should be level, residual {residual}"
        );
    }

    #[test]
    fn gross_skew_at_least_8_degrees_is_rejected() {
        let bars = text_bars_image(400, 320);
        let tilted = rotate_gray(&bars, 9.0);
        let error =
            deskew_image(&DynamicImage::ImageLuma8(tilted), DESKEW_DEFAULT_MAX_ANGLE).unwrap_err();
        assert_eq!(error.code, AppErrorCode::DeskewOutOfRange);
    }

    #[test]
    fn skew_outside_declared_operating_range_is_rejected() {
        let bars = text_bars_image(400, 320);
        let tilted = rotate_gray(&bars, 5.0);
        let error =
            deskew_image(&DynamicImage::ImageLuma8(tilted), DESKEW_DEFAULT_MAX_ANGLE).unwrap_err();
        assert_eq!(error.code, AppErrorCode::DeskewOutOfRange);
    }

    #[test]
    fn rotation_preserves_canvas_dimensions() {
        let bars = text_bars_image(300, 220);
        let rotated = rotate_gray(&bars, 2.0);
        assert_eq!(rotated.dimensions(), bars.dimensions());
    }

    fn region(x: f32, y: f32, width: f32, height: f32) -> NormalizedBBox {
        NormalizedBBox {
            x,
            y,
            width,
            height,
        }
    }

    fn ink_block(image: &mut GrayImage, cx: f32, cy: f32, half: u32) {
        let (w, h) = image.dimensions();
        let center_x = (cx * w as f32) as i32;
        let center_y = (cy * h as f32) as i32;
        for dy in -(half as i32)..=(half as i32) {
            for dx in -(half as i32)..=(half as i32) {
                let px = center_x + dx;
                let py = center_y + dy;
                if px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
                    image.put_pixel(px as u32, py as u32, Luma([30]));
                }
            }
        }
    }

    #[test]
    fn aligned_regions_pass_registration_validation() {
        let mut page = white_gray(800, 1100);
        let regions = [region(0.1, 0.1, 0.6, 0.2), region(0.1, 0.5, 0.6, 0.25)];
        // Ink centered inside each expected region.
        for bbox in &regions {
            ink_block(
                &mut page,
                bbox.x + bbox.width / 2.0,
                bbox.y + bbox.height / 2.0,
                30,
            );
        }
        let measurement = measure_registration_deviation(&page, &regions);
        assert_eq!(measurement.sampled_regions, 2);
        assert_eq!(measurement.empty_regions, 0);
        validate_registration(&measurement, DEFAULT_MAX_REGISTRATION_DEVIATION)
            .expect("aligned regions must validate");
    }

    #[test]
    fn shifted_content_fails_registration_validation() {
        let mut page = white_gray(800, 1100);
        let regions = [region(0.1, 0.1, 0.6, 0.2), region(0.1, 0.5, 0.6, 0.25)];
        // Both answers written against the left edge of their region: a
        // systematic grid offset, not random handwriting position.
        for bbox in &regions {
            ink_block(&mut page, bbox.x + 0.02, bbox.y + bbox.height / 2.0, 20);
        }
        let measurement = measure_registration_deviation(&page, &regions);
        let error = validate_registration(&measurement, DEFAULT_MAX_REGISTRATION_DEVIATION)
            .expect_err("systematically shifted content must fail registration");
        assert_eq!(error.code, AppErrorCode::RegistrationOutOfRange);
    }

    #[test]
    fn random_handwriting_positions_do_not_fail_registration() {
        let mut page = white_gray(800, 1100);
        let regions = [region(0.1, 0.1, 0.6, 0.2), region(0.1, 0.5, 0.6, 0.25)];
        // Answers placed at arbitrary, scattered positions inside the regions:
        // high scatter but no systematic shift.
        ink_block(&mut page, 0.62, 0.11, 18);
        ink_block(&mut page, 0.14, 0.71, 22);
        let measurement = measure_registration_deviation(&page, &regions);
        assert_eq!(measurement.sampled_regions, 2);
        validate_registration(&measurement, DEFAULT_MAX_REGISTRATION_DEVIATION)
            .expect("scattered handwriting must not fail registration");
    }

    #[test]
    fn blank_page_is_not_a_registration_failure() {
        let page = white_gray(800, 1100);
        let regions = [region(0.1, 0.1, 0.6, 0.2), region(0.1, 0.5, 0.6, 0.25)];
        let measurement = measure_registration_deviation(&page, &regions);
        assert_eq!(measurement.sampled_regions, 0);
        assert_eq!(measurement.empty_regions, 2);
        validate_registration(&measurement, DEFAULT_MAX_REGISTRATION_DEVIATION)
            .expect("blank page must not be rejected");
    }
}
