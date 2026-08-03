use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::ImageError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::{ModelInputImage, ModelInputImageKind};
use crate::platform::file_access::durable_rename;
use crate::platform::project_paths::TrustedProjectRoot;

const DEFAULT_LONG_EDGE_MAX: u32 = 1800;
const DEFAULT_JPEG_QUALITY: u8 = 88;
pub const MODEL_INPUT_CACHE_SCHEMA_VERSION: u32 = 1;
pub const RESIZE_POLICY_VERSION: &str = "no_upscale_lanczos3_v1";
pub const JPEG_ENCODER_VERSION: &str = "image-rs-jpeg-0.25-v1";

#[derive(Clone)]
pub struct ModelInputImageService {
    long_edge_max: u32,
    jpeg_quality: u8,
    cache_locks: Arc<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>>,
    manifest_locks: Arc<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>>,
    #[cfg(test)]
    encoder_invocations: Arc<std::sync::atomic::AtomicU64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInputCacheRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInputCacheOptions {
    #[serde(default)]
    pub crop_regions: Vec<ModelInputCacheRegion>,
    #[serde(default = "default_alignment_transform")]
    pub alignment_transform: String,
    #[serde(default = "default_preprocess_mode")]
    pub preprocess_mode: String,
    #[serde(default = "default_resize_policy_version")]
    pub resize_policy_version: String,
}

impl Default for ModelInputCacheOptions {
    fn default() -> Self {
        Self {
            crop_regions: Vec::new(),
            alignment_transform: default_alignment_transform(),
            preprocess_mode: default_preprocess_mode(),
            resize_policy_version: default_resize_policy_version(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelInputBatchMetadata {
    pub kind: ModelInputImageKind,
    pub document_id: String,
    pub created_at: String,
    pub long_edge_max: u32,
    pub jpeg_quality: u8,
    #[serde(default)]
    pub cache_schema_version: u32,
    #[serde(default)]
    pub transaction_id: String,
    #[serde(default = "default_resize_policy_version")]
    pub resize_policy_version: String,
    #[serde(default)]
    pub encoder_version: String,
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
            cache_locks: Arc::new(Mutex::new(HashMap::new())),
            manifest_locks: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(test)]
            encoder_invocations: Arc::new(std::sync::atomic::AtomicU64::new(0)),
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
        self.prepare_inputs_with_options(
            project_root,
            kind,
            document_id,
            sources,
            &ModelInputCacheOptions::default(),
        )
    }

    pub fn prepare_inputs_with_options(
        &self,
        project_root: &Path,
        kind: ModelInputImageKind,
        document_id: &str,
        sources: &[(u32, PathBuf)],
        options: &ModelInputCacheOptions,
    ) -> Result<Vec<ModelInputImage>, AppError> {
        let trusted_root =
            TrustedProjectRoot::from_canonical_root(project_root.to_path_buf(), false)?;
        let output_relative = trusted_root.managed(&format!(
            "cache/model_inputs/{}/{document_id}",
            kind_folder(&kind)
        ))?;
        let output_dir = trusted_root.root().join(output_relative.as_path());
        let directory_lock = self.lock_for(&self.cache_locks, &output_dir)?;
        let _directory_guard = directory_lock.lock().map_err(|error| {
            app_error(
                AppErrorCode::FileWriteFailed,
                "Model input cache klasörü kilitlenemedi.",
                Some(format!("cache directory lock failed: {error}")),
                Some("Model input cache işlemini yeniden deneyin.".to_string()),
            )
        })?;
        trusted_root.ensure_managed_directory(&output_dir)?;
        let manifest_path =
            trusted_root.prepare_write_target(&trusted_root.managed(&format!(
                "cache/model_inputs/{}/{document_id}/model_inputs.json",
                kind_folder(&kind)
            ))?)?;
        let cached_output_hashes = load_cached_output_hashes(&manifest_path)?;

        let created_at = chrono::Utc::now().to_rfc3339();
        let transaction_id = Uuid::new_v4().to_string();
        let mut images = Vec::with_capacity(sources.len());
        let mut total_source_bytes = 0u64;
        let mut total_output_bytes = 0u64;
        let mut total_base64_approx_bytes = 0u64;

        for (page_number, source_path) in sources {
            let source_managed = trusted_root.relative_for_existing(source_path)?;
            let safe_source_path = trusted_root.resolve_existing_file(&source_managed)?;
            let metadata = self.prepare_single_image(
                PrepareSingleImageInput {
                    output_dir: &output_dir,
                    kind: kind.clone(),
                    document_id,
                    page_number: *page_number,
                    source_path: &safe_source_path,
                    long_edge_max: self.long_edge_max,
                    jpeg_quality: self.jpeg_quality,
                    created_at: &created_at,
                    transaction_id: &transaction_id,
                    options,
                },
                &cached_output_hashes,
            )?;
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
            cache_schema_version: MODEL_INPUT_CACHE_SCHEMA_VERSION,
            transaction_id,
            resize_policy_version: options.resize_policy_version.clone(),
            encoder_version: JPEG_ENCODER_VERSION.to_string(),
            total_source_bytes,
            total_output_bytes,
            total_base64_approx_bytes,
            images: images.clone(),
        };
        let manifest_lock = self.lock_for(&self.manifest_locks, &manifest_path)?;
        let _manifest_guard = manifest_lock.lock().map_err(|error| {
            app_error(
                AppErrorCode::FileWriteFailed,
                "Model input metadata kilitlenemedi.",
                Some(format!("manifest lock failed: {error}")),
                Some("Model input cache işlemini yeniden deneyin.".to_string()),
            )
        })?;
        write_manifest(&trusted_root, &manifest_path, &manifest)?;
        Ok(images)
    }

    fn lock_for(
        &self,
        registry: &Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
        path: &Path,
    ) -> Result<Arc<Mutex<()>>, AppError> {
        let mut registry = registry.lock().map_err(|error| {
            app_error(
                AppErrorCode::FileWriteFailed,
                "Model input cache kilitlenemedi.",
                Some(format!("cache lock registry failed: {error}")),
                Some("Model input cache işlemini yeniden deneyin.".to_string()),
            )
        })?;
        Ok(registry
            .entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone())
    }

    #[cfg(test)]
    fn encoder_invocations(&self) -> u64 {
        self.encoder_invocations
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn manifest_path(
        project_root: &Path,
        kind: &ModelInputImageKind,
        document_id: &str,
    ) -> Result<PathBuf, AppError> {
        Ok(model_inputs_dir(project_root, kind, document_id)?.join("model_inputs.json"))
    }

    pub fn load_manifests(project_root: &Path) -> Result<Vec<ModelInputBatchMetadata>, AppError> {
        let trusted_root =
            TrustedProjectRoot::from_canonical_root(project_root.to_path_buf(), false)?;
        let root_managed = trusted_root.managed("cache/model_inputs")?;
        let root_path = trusted_root.root().join(root_managed.as_path());
        if !root_path.exists() {
            return Ok(vec![]);
        }
        let root = trusted_root.resolve_existing_directory(&root_managed)?;

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
            let kind_path = kind_entry.path();
            let kind_file_type = kind_entry.file_type().map_err(|error| {
                app_error(
                    AppErrorCode::FileReadFailed,
                    "Model input cache okunamadı.",
                    Some(error.to_string()),
                    Some("Check project cache permissions.".to_string()),
                )
            })?;
            if kind_file_type.is_symlink() {
                return Err(app_error(
                    AppErrorCode::ManagedPathSymlinkEscape,
                    "Model input cache symlink içeriyor.",
                    Some(kind_path.to_string_lossy().to_string()),
                    Some("Rebuild the model input cache.".to_string()),
                ));
            }
            if !kind_file_type.is_dir() {
                continue;
            }
            for doc_entry in std::fs::read_dir(&kind_path).map_err(|error| {
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
                let doc_path = doc_entry.path();
                let doc_file_type = doc_entry.file_type().map_err(|error| {
                    app_error(
                        AppErrorCode::FileReadFailed,
                        "Model input cache okunamadı.",
                        Some(error.to_string()),
                        Some("Check project cache permissions.".to_string()),
                    )
                })?;
                if doc_file_type.is_symlink() {
                    return Err(app_error(
                        AppErrorCode::ManagedPathSymlinkEscape,
                        "Model input cache symlink içeriyor.",
                        Some(doc_path.to_string_lossy().to_string()),
                        Some("Rebuild the model input cache.".to_string()),
                    ));
                }
                if !doc_file_type.is_dir() {
                    continue;
                }
                let manifest_path = doc_path.join("model_inputs.json");
                if !manifest_path.exists() {
                    continue;
                }
                let manifest_managed = trusted_root.relative_for_existing(&manifest_path)?;
                let manifest_path = trusted_root.resolve_existing_file(&manifest_managed)?;
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
                Self::validate_manifest_paths(&trusted_root, &manifest)?;
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

    pub fn validate_manifest_paths(
        trusted_root: &TrustedProjectRoot,
        manifest: &ModelInputBatchMetadata,
    ) -> Result<(), AppError> {
        for image in &manifest.images {
            validate_model_image_path(trusted_root, &image.source_image_path)?;
            validate_model_image_path(trusted_root, &image.output_image_path)?;
        }
        Ok(())
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
    transaction_id: &'a str,
    options: &'a ModelInputCacheOptions,
}

impl ModelInputImageService {
    fn prepare_single_image(
        &self,
        input: PrepareSingleImageInput<'_>,
        cached_output_hashes: &HashMap<String, String>,
    ) -> Result<ModelInputImage, AppError> {
        let PrepareSingleImageInput {
            output_dir,
            kind,
            document_id,
            page_number,
            source_path,
            long_edge_max,
            jpeg_quality,
            created_at,
            transaction_id,
            options,
        } = input;
        if !source_path.exists() {
            return Err(app_error(
                AppErrorCode::FileReadFailed,
                "Model girdisi için kaynak görüntü bulunamadı.",
                Some(source_path.to_string_lossy().to_string()),
                Some("Preview or rendered page image is missing.".to_string()),
            ));
        }

        let (source_sha256, source_bytes) = sha256_file(source_path).map_err(|error| {
            app_error(
                AppErrorCode::FileReadFailed,
                "Kaynak görüntü okunamadı.",
                Some(error.to_string()),
                Some("Check the source image path.".to_string()),
            )
        })?;
        let (source_width, source_height) = image::image_dimensions(source_path)
            .map_err(map_image_error("Kaynak görüntü boyutları okunamadı."))?;
        let cache_key =
            model_input_cache_key(&source_sha256, options, long_edge_max, jpeg_quality)?;
        let output_path = output_dir.join(format!("page_{page_number:03}_{cache_key}.jpg"));
        let output_path_key = output_path.to_string_lossy().to_string();
        let expected_output_sha256 = cached_output_hashes
            .get(&output_path_key)
            .map(String::as_str);
        let output_lock = self.lock_for(&self.cache_locks, &output_path)?;
        let _output_guard = output_lock.lock().map_err(|error| {
            app_error(
                AppErrorCode::FileWriteFailed,
                "Model giriş JPEG kilitlenemedi.",
                Some(format!("output lock failed: {error}")),
                Some("Model input cache işlemini yeniden deneyin.".to_string()),
            )
        })?;
        let (output_width, output_height) =
            resize_dimensions(source_width, source_height, long_edge_max);

        if let Some((output_sha256, output_bytes)) =
            valid_cached_jpeg(&output_path, expected_output_sha256)?
        {
            return Ok(ModelInputImage {
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
                source_sha256: Some(source_sha256),
                output_sha256: Some(output_sha256),
                cache_key: Some(cache_key),
                cache_transaction_id: Some(transaction_id.to_string()),
                cache_hit: true,
            });
        }

        let image =
            image::open(source_path).map_err(map_image_error("Kaynak görüntü açılamadı."))?;
        let (output_image, output_width, output_height) =
            resize_image(image, source_width, source_height, long_edge_max);
        let temp_path = output_dir.join(format!(
            ".page_{page_number:03}_{cache_key}.jpg.tmp-{}",
            Uuid::new_v4()
        ));
        let encode_result =
            encode_jpeg_atomically(&temp_path, &output_path, &output_image, jpeg_quality);
        if let Err(error) = encode_result {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
        #[cfg(test)]
        self.encoder_invocations
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let (output_sha256, output_bytes) = sha256_file(&output_path).map_err(|error| {
            app_error(
                AppErrorCode::FileReadFailed,
                "Model giriş JPEG boyutu okunamadı.",
                Some(error.to_string()),
                Some("Check the generated image file.".to_string()),
            )
        })?;

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
            source_sha256: Some(source_sha256),
            output_sha256: Some(output_sha256),
            cache_key: Some(cache_key),
            cache_transaction_id: Some(transaction_id.to_string()),
            cache_hit: false,
        })
    }
}

fn resize_image(
    image: image::DynamicImage,
    source_width: u32,
    source_height: u32,
    long_edge_max: u32,
) -> (image::DynamicImage, u32, u32) {
    let (output_width, output_height) =
        resize_dimensions(source_width, source_height, long_edge_max);
    if output_width == source_width && output_height == source_height {
        return (image, source_width, source_height);
    }
    let resized = image.resize(output_width, output_height, FilterType::Lanczos3);
    (resized, output_width, output_height)
}

fn resize_dimensions(source_width: u32, source_height: u32, long_edge_max: u32) -> (u32, u32) {
    let long_edge = source_width.max(source_height);
    if long_edge <= long_edge_max {
        return (source_width, source_height);
    }
    let scale = long_edge_max as f32 / long_edge as f32;
    (
        ((source_width as f32) * scale).round().max(1.0) as u32,
        ((source_height as f32) * scale).round().max(1.0) as u32,
    )
}

pub fn model_input_cache_key(
    source_sha256: &str,
    options: &ModelInputCacheOptions,
    long_edge_max: u32,
    jpeg_quality: u8,
) -> Result<String, AppError> {
    let key_material = serde_json::json!({
        "sourceSha256": source_sha256,
        "orderedCropRegions": options.crop_regions.clone(),
        "alignmentTransform": options.alignment_transform.clone(),
        "preprocessMode": options.preprocess_mode.clone(),
        "resizePolicyVersion": options.resize_policy_version.clone(),
        "longEdgeMax": long_edge_max,
        "jpegQuality": jpeg_quality,
        "encoderVersion": JPEG_ENCODER_VERSION,
    });
    let serialized = serde_json::to_vec(&key_material).map_err(|error| {
        app_error(
            AppErrorCode::FileWriteFailed,
            "Model input cache anahtarı oluşturulamadı.",
            Some(format!("cache key serialization failed: {error}")),
            Some("Model input cache işlemini yeniden deneyin.".to_string()),
        )
    })?;
    let mut hasher = Sha256::new();
    hasher.update(serialized);
    Ok(hex::encode(hasher.finalize()))
}

fn sha256_file(path: &Path) -> std::io::Result<(String, u64)> {
    let file = File::open(path)?;
    let bytes = file.metadata()?.len();
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok((hex::encode(hasher.finalize()), bytes))
}

fn valid_cached_jpeg(
    path: &Path,
    expected_output_sha256: Option<&str>,
) -> Result<Option<(String, u64)>, AppError> {
    if expected_output_sha256.is_none() {
        return Ok(None);
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(app_error(
                AppErrorCode::FileReadFailed,
                "Model input JPEG cache okunamadı.",
                Some(error.to_string()),
                Some("Model input cache işlemini yeniden deneyin.".to_string()),
            ))
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(app_error(
            AppErrorCode::ManagedPathSymlinkEscape,
            "Model giriş JPEG yazma hedefi güvenli değil.",
            Some(path.to_string_lossy().to_string()),
            Some("Model input cache işlemini yeniden deneyin.".to_string()),
        ));
    }
    if metadata.len() < 2 {
        return Ok(None);
    }
    let mut file = File::open(path).map_err(|error| {
        app_error(
            AppErrorCode::FileReadFailed,
            "Model input JPEG cache açılamadı.",
            Some(error.to_string()),
            Some("Model input cache işlemini yeniden deneyin.".to_string()),
        )
    })?;
    let mut header = [0u8; 2];
    file.read_exact(&mut header).map_err(|error| {
        app_error(
            AppErrorCode::FileReadFailed,
            "Model input JPEG cache okunamadı.",
            Some(error.to_string()),
            Some("Model input cache işlemini yeniden deneyin.".to_string()),
        )
    })?;
    if header != [0xff, 0xd8] {
        return Ok(None);
    }
    let (hash, bytes) = sha256_file(path).map_err(|error| {
        app_error(
            AppErrorCode::FileReadFailed,
            "Model input JPEG cache doğrulanamadı.",
            Some(error.to_string()),
            Some("Model input cache işlemini yeniden deneyin.".to_string()),
        )
    })?;
    if expected_output_sha256.is_some_and(|expected| expected != hash) {
        return Ok(None);
    }
    Ok(Some((hash, bytes)))
}

fn encode_jpeg_atomically(
    temp_path: &Path,
    output_path: &Path,
    image: &image::DynamicImage,
    jpeg_quality: u8,
) -> Result<(), AppError> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)
        .map_err(|error| {
            app_error(
                AppErrorCode::FileWriteFailed,
                "Model giriş JPEG geçici dosyası oluşturulamadı.",
                Some(error.to_string()),
                Some("Check project cache permissions.".to_string()),
            )
        })?;
    let mut writer = BufWriter::new(file);
    {
        let mut encoder = JpegEncoder::new_with_quality(&mut writer, jpeg_quality);
        encoder
            .encode_image(image)
            .map_err(map_image_error("Model giriş JPEG dosyası yazılamadı."))?;
    }
    writer.flush().map_err(|error| {
        app_error(
            AppErrorCode::FileWriteFailed,
            "Model giriş JPEG dosyası tamamlanamadı.",
            Some(error.to_string()),
            Some("Check project cache permissions.".to_string()),
        )
    })?;
    let file = writer.into_inner().map_err(|error| {
        app_error(
            AppErrorCode::FileWriteFailed,
            "Model giriş JPEG dosyası kapatılamadı.",
            Some(error.to_string()),
            Some("Check project cache permissions.".to_string()),
        )
    })?;
    file.sync_all().map_err(|error| {
        app_error(
            AppErrorCode::FileWriteFailed,
            "Model giriş JPEG dosyası diske eşitlenemedi.",
            Some(error.to_string()),
            Some("Check project cache permissions.".to_string()),
        )
    })?;
    durable_rename(temp_path, output_path).map_err(|error| {
        app_error(
            AppErrorCode::FileWriteFailed,
            "Model giriş JPEG cache yayımlanamadı.",
            Some(error.to_string()),
            Some("Model input cache işlemini yeniden deneyin.".to_string()),
        )
    })
}

fn default_alignment_transform() -> String {
    "identity_v1".to_string()
}

fn default_preprocess_mode() -> String {
    "source".to_string()
}

fn default_resize_policy_version() -> String {
    RESIZE_POLICY_VERSION.to_string()
}

fn base64_approx_bytes(bytes: u64) -> u64 {
    if bytes == 0 {
        return 0;
    }
    bytes.div_ceil(3) * 4
}

fn model_inputs_dir(
    project_root: &Path,
    kind: &ModelInputImageKind,
    document_id: &str,
) -> Result<PathBuf, AppError> {
    let root = TrustedProjectRoot::from_canonical_root(project_root.to_path_buf(), false)?;
    let managed = root.managed(&format!(
        "cache/model_inputs/{}/{document_id}",
        kind_folder(kind)
    ))?;
    Ok(root.root().join(managed.as_path()))
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
    trusted_root: &TrustedProjectRoot,
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
    let managed = trusted_root.managed_for_path(manifest_path)?;
    trusted_root
        .atomic_write(&managed, &content)
        .map_err(|error| {
            app_error(
                AppErrorCode::ProjectSaveFailed,
                "Model input metadata yazılamadı.",
                Some(error.to_string()),
                Some("Check model input cache permissions.".to_string()),
            )
        })
}

/// A cache manifest is derived metadata, not project truth. If it is missing
/// or malformed, the next prepare call simply rebuilds the JPEGs. A readable
/// manifest contributes only the expected output hashes used to detect a
/// retained-JPEG-header corruption without decoding on the hit path.
fn load_cached_output_hashes(path: &Path) -> Result<HashMap<String, String>, AppError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(error) => {
            return Err(app_error(
                AppErrorCode::FileReadFailed,
                "Model input metadata okunamadı.",
                Some(error.to_string()),
                Some("Model input cache işlemini yeniden deneyin.".to_string()),
            ))
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(HashMap::new());
    }
    let content = fs::read_to_string(path).map_err(|error| {
        app_error(
            AppErrorCode::FileReadFailed,
            "Model input metadata okunamadı.",
            Some(error.to_string()),
            Some("Model input cache işlemini yeniden deneyin.".to_string()),
        )
    })?;
    let Ok(manifest) = serde_json::from_str::<ModelInputBatchMetadata>(&content) else {
        return Ok(HashMap::new());
    };
    Ok(manifest
        .images
        .into_iter()
        .filter_map(|image| {
            image
                .output_sha256
                .map(|hash| (image.output_image_path, hash))
        })
        .collect())
}

fn validate_model_image_path(
    trusted_root: &TrustedProjectRoot,
    raw_path: &str,
) -> Result<(), AppError> {
    let managed = trusted_root.adapt_legacy_document_path(raw_path)?;
    trusted_root.resolve_existing_file(&managed).map(|_| ())
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
        .unwrap()
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

    #[test]
    fn cache_hit_reuses_jpeg_without_reencoding_and_key_includes_options() {
        let root = temp_root();
        let source = root.join("source-cache.png");
        write_test_png(&source, 900, 700);
        let service = ModelInputImageService::default();

        let first = service
            .prepare_inputs(
                &root,
                ModelInputImageKind::QuestionText,
                "doc-cache",
                &[(1, source.clone())],
            )
            .unwrap();
        let first_bytes = std::fs::read(&first[0].output_image_path).unwrap();
        assert!(!first[0].cache_hit);
        assert_eq!(service.encoder_invocations(), 1);

        let second = service
            .prepare_inputs(
                &root,
                ModelInputImageKind::QuestionText,
                "doc-cache",
                &[(1, source.clone())],
            )
            .unwrap();
        assert!(second[0].cache_hit);
        assert_eq!(first[0].cache_key, second[0].cache_key);
        assert_eq!(
            first_bytes,
            std::fs::read(&second[0].output_image_path).unwrap()
        );
        assert_eq!(service.encoder_invocations(), 1);

        let options = ModelInputCacheOptions {
            preprocess_mode: "handwriting_enhanced_v1".to_string(),
            ..Default::default()
        };
        let changed = service
            .prepare_inputs_with_options(
                &root,
                ModelInputImageKind::QuestionText,
                "doc-cache",
                &[(1, source)],
                &options,
            )
            .unwrap();
        assert_ne!(first[0].cache_key, changed[0].cache_key);
        assert_eq!(service.encoder_invocations(), 2);
    }

    #[test]
    fn missing_or_corrupt_jpeg_is_rebuilt() {
        let root = temp_root();
        let source = root.join("source-corrupt.png");
        write_test_png(&source, 900, 700);
        let service = ModelInputImageService::default();
        let first = service
            .prepare_inputs(
                &root,
                ModelInputImageKind::Rubric,
                "doc-corrupt",
                &[(1, source.clone())],
            )
            .unwrap();
        std::fs::write(&first[0].output_image_path, b"\xff\xd8corrupt-cache").unwrap();

        let rebuilt = service
            .prepare_inputs(
                &root,
                ModelInputImageKind::Rubric,
                "doc-corrupt",
                &[(1, source)],
            )
            .unwrap();
        assert!(!rebuilt[0].cache_hit);
        assert_eq!(service.encoder_invocations(), 2);
        assert_eq!(
            std::fs::read(&rebuilt[0].output_image_path).unwrap()[..2],
            [0xff, 0xd8]
        );
    }

    #[test]
    fn concurrent_same_key_has_one_writer_and_no_temp_collision() {
        let root = temp_root();
        let source = root.join("source-concurrent.png");
        write_test_png(&source, 1000, 800);
        let service = Arc::new(ModelInputImageService::default());
        let mut handles = Vec::new();
        for _ in 0..4 {
            let service = Arc::clone(&service);
            let root = root.clone();
            let source = source.clone();
            handles.push(std::thread::spawn(move || {
                service
                    .prepare_inputs(
                        &root,
                        ModelInputImageKind::StudentOcr,
                        "doc-concurrent",
                        &[(1, source)],
                    )
                    .unwrap()
            }));
        }
        let results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(service.encoder_invocations(), 1);
        assert!(results
            .iter()
            .all(|images| Path::new(&images[0].output_image_path).exists()));
        let manifest = std::fs::read_to_string(
            ModelInputImageService::manifest_path(
                &root,
                &ModelInputImageKind::StudentOcr,
                "doc-concurrent",
            )
            .unwrap(),
        )
        .unwrap();
        let manifest: ModelInputBatchMetadata = serde_json::from_str(&manifest).unwrap();
        assert!(!manifest.transaction_id.is_empty());
        assert!(manifest
            .images
            .iter()
            .all(|image| image.cache_transaction_id.as_deref()
                == Some(manifest.transaction_id.as_str())));
        let temp_count =
            std::fs::read_dir(root.join("cache/model_inputs/student_ocr/doc-concurrent"))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
                .count();
        assert_eq!(temp_count, 0);
    }
}
