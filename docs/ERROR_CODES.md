# Error Codes

Rust uses one central error type to ensure no raw exceptions leak to the UI.

```rust
pub struct AppError {
    pub code: AppErrorCode,
    pub message: String,
    pub recoverable: bool,
    pub suggested_action: Option<String>,
    pub technical_details: Option<String>,
    pub correlation_id: String,
}
```

## Error Categories & Teacher-Friendly Messages

| Error Code | Meaning | Teacher-Friendly Message |
| --- | --- | --- |
| `PROJECT_NOT_FOUND` | Project file could not be located | "Proje dosyası bulunamadı." |
| `PROJECT_LOAD_FAILED` | Project format invalid or corrupted | "Proje yüklenirken bir sorun oluştu." |
| `PROJECT_SAVE_FAILED` | Failed to write to disk | "Proje kaydedilemedi. Lütfen disk alanınızı kontrol edin." |
| `DOCUMENT_IMPORT_FAILED` | File read or copy error | "Belge içe aktarılamadı." |
| `DOCUMENT_NOT_FOUND` | Requested document is missing from the open project | "Belge bulunamadı." |
| `PDF_DOCUMENT_NOT_FOUND` | PDF document or stored file missing | "Sınav PDF'i bulunamadı." |
| `PDF_PAGE_COUNT_FAILED` | PDF page count tool failed | "PDF sayfa sayısı okunamadı." |
| `PDF_RENDER_FAILED` | PDF could not be processed | "PDF dosyası okunamadı, lütfen farklı bir PDF deneyin." |
| `PDF_PREVIEW_NOT_FOUND` | PDF preview cache or metadata missing | "PDF önizlemeleri bulunamadı." |
| `PDF_PREVIEW_NOT_READY` | Preview cache or image files are not ready | "Soru metni çıkarılmadan önce PDF sayfa önizlemeleri oluşturulmalıdır." |
| `PDF_PREVIEW_JOB_FAILED` | PDF preview job failed | "PDF önizleme işi başarısız oldu." |
| `PDF_UNSUPPORTED_FORMAT` | PDF tool reported an unsupported format | "Bu PDF biçimi desteklenmiyor." |
| `PDF_RENDERER_NOT_FOUND` | PDF renderer tools are not installed | "PDF önizleme aracı bulunamadı. Poppler kurulu olmayabilir. Terminalde `brew install poppler` komutunu çalıştırıp tekrar deneyin." |
| `PDF_RENDERER_START_FAILED` | PDF renderer binary failed to spawn | "PDF önizleme aracı başlatılamadı." |
| `PDF_RENDERER_PERMISSION_DENIED` | Permission denied while running PDF renderer | "PDF önizleme aracı çalıştırma izni yok." |
| `PDF_RENDERER_OUTPUT_MISSING` | PDF renderer completed but produced no output | "PDF önizleme çıktısı oluşmadı." |
| `PDF_RENDERER_UNSUPPORTED` | Unsupported PDF feature or format error | "PDF önizleme aracı bu PDF dosyasını desteklemiyor." |
| `FILE_READ_FAILED` | File read failed | "Dosya okunamadı." |
| `FILE_WRITE_FAILED` | File write failed | "Dosya kaydedilemedi." |
| `CROP_REGION_MISSING` | Required crop template missing | "Soru alanları işaretlenmemiş." |
| `WORKFLOW_BLOCKED` | Attempted action not allowed in current state | "Bu işlem şu an yapılamaz." |
| `MODEL_SERVER_NOT_RUNNING` | llama.cpp sidecar unreachable | "Yapay zeka motoru çalışmıyor veya başlatılamadı." |
| `MODEL_HEALTH_FAILED` | Model healthcheck failed | "Yapay zeka motoru yanıt vermiyor." |
| `MODEL_TIMEOUT` | Model request exceeded time limit | "İşlem zaman aşımına uğradı." |
| `MODEL_RESPONSE_EMPTY` | Model returned no content | "Beklenmeyen boş bir sonuç alındı." |
| `MODEL_RESPONSE_INVALID_JSON` | Model output could not be parsed | "Sistem bu yanıtı okuyamadı (teknik bir format hatası)." |
| `MODEL_RESPONSE_INVALID_SCHEMA` | Model output shape is invalid | "Sistem bu yanıtın biçimini doğrulayamadı." |
| `MODEL_RESPONSE_REASONING_ONLY` | Model provided thought process but no answer | "Model sadece düşünce süreci üretti, asıl yanıt eksik." |
| `MODEL_SERVER_LOST_DURING_REQUEST` | Model connection dropped while a request was active | "Yapay zeka motoru istek sırasında bağlantıyı kaybetti." |
| `MODEL_SERVER_CRASHED_DURING_REQUEST` | Managed model process exited during a request | "Yapay zeka motoru istek sırasında kapandı." |
| `MODEL_REQUEST_TIMEOUT` | Diagnostic replay or import timed out after a request was initiated | "İstek zaman aşımına uğradı." |
| `MODEL_OUTPUT_RETRY_FAILED` | Strict JSON retry still failed | "Model çıktısı yeniden denemede de çözülemedi." |
| `OCR_FAILED` | OCR step failed completely | "Metin tanıma (OCR) işlemi başarısız oldu." |
| `SCORING_FAILED` | Scoring step failed completely | "Değerlendirme (Puanlama) işlemi başarısız oldu." |
| `ANALYSIS_NOT_READY` | No finalized scores are available for analysis | "Analiz için önce öğrenci puanlarını kaydedin." |
| `ANALYSIS_FAILED` | Analysis artifact or Gemma report could not be produced | "Sınav analizi tamamlanamadı; grafikler varsa korunmuştur." |
| `QEP_NOT_FROZEN` | Attempt to score without frozen QEP | "Değerlendirme paketi (QEP) onaylanmadan puanlama yapılamaz." |
| `RUBRIC_MISSING` | Required rubric data missing | "Cevap anahtarı eksik." |
| `QUESTION_TEXT_MISSING` | Required question text missing | "Soru metni eksik." |
| `QUESTION_TEXT_EXTRACTION_FAILED` | Question text extraction failed after retries | "Soru metni çıkarılamadı." |
| `QUESTION_TEXT_SUGGESTION_NOT_FOUND` | Question text suggestion or target question missing | "Soru metni önerisi bulunamadı." |
| `QUESTION_TEXT_CONFIRM_FAILED` | Question text confirm/edit failed | "Soru metni onaylanamadı." |
| `QUESTION_TEXT_PARTIAL_SUCCESS` | Extraction partially succeeded | "Soru metni çıkarımı kısmen tamamlandı." |
| `RUBRIC_JSON_INVALID` | Rubric JSON could not be parsed | "Rubrik JSON okunamadı." |
| `RUBRIC_JSON_PARSE_FAILED` | Rubric JSON could not be extracted or parsed | "Rubrik JSON çıktısı çözülemedi." |
| `RUBRIC_JSON_SCHEMA_UNSUPPORTED` | Rubric JSON schema not recognized | "Rubrik JSON biçimi desteklenmiyor." |
| `RUBRIC_SCHEMA_VALIDATION_FAILED` | Rubric JSON schema validation failed | "Rubrik JSON biçimi doğrulanamadı." |
| `RUBRIC_QUESTION_NOT_FOUND` | Rubric question id could not be matched | "Rubrik sorusu bulunamadı." |
| `RUBRIC_PLACEHOLDER_DETECTED` | Placeholder content detected in rubric | "Rubrikte taslak metin bulundu." |
| `RUBRIC_MAX_SCORE_MISSING` | Rubric max score is missing | "Max puan eksik." |
| `RUBRIC_CRITERIA_SCORE_MISMATCH` | Rubric criteria total does not match max score | "Kriter puanları toplamı uyumsuz." |
| `RUBRIC_CONFIRM_FAILED` | Rubric confirm failed due to validation errors | "Rubrik onaylanamadı." |
| `RUBRIC_NOT_READY` | Question text or rubric prep is not ready | "Rubrik hazırlığı henüz hazır değil." |
| `QUESTION_COVERAGE_INCOMPLETE` | Question coverage is incomplete | "Soru kapsamı eksik." |
| `QUESTION_LAST_ITEM_MISSING` | Last question is missing from coverage | "Son soru eksik." |
| `STUDENT_SCAN_NOT_FOUND` | Student scan PDF could not be found | "Öğrenci cevap PDF’i bulunamadı." |
| `STUDENT_SCAN_PREVIEW_NOT_READY` | Student scan preview cache is not ready | "Öğrenci PDF önizlemeleri hazır değil." |
| `STUDENT_GROUPING_NOT_READY` | Student grouping has not been created yet | "Öğrenci gruplaması henüz oluşturulmadı." |
| `STUDENT_GROUPING_INVALID` | Student grouping input is invalid | "Öğrenci gruplaması geçersiz." |
| `STUDENT_SUBMISSION_NOT_FOUND` | Student submission could not be found | "Öğrenci submission'ı bulunamadı." |
| `STUDENT_IDENTITY_INVALID` | Student identity is incomplete or invalid | "Öğrenci kimliği geçersiz." |
| `OCR_NOT_READY` | OCR cannot start yet | "OCR hazırlığı tamam değil." |
| `PERMISSION_DENIED` | File system permission error | "Dosya erişim izni reddedildi." |
| `UNKNOWN_ERROR` | Fallback catch-all | "Bilinmeyen bir hata oluştu." |

## Diagnostics

Technical codes and details may appear **only** in Developer / Diagnostics / Raw JSON panels, never directly to the teacher. Raw exceptions are mapped here.

## Diagnostic Expectations
- Diagnostic CLI failures should report the most specific matching error code.
- Import crash reports should distinguish transport loss, timeout, empty output, invalid JSON, and invalid schema.
