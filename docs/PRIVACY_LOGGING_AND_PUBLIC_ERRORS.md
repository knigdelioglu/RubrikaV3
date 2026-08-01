# Gizlilik, Güvenli Loglama ve Public Error Sözleşmesi

## Hassas veri sınıfları

| Sınıf | Örnekler | Log/Diagnostic'te izin |
| --- | --- | --- |
| `PublicDiagnostic` | job id, correlation id, error code, sayaçlar | Evet |
| `OperationalMetadata` | page_count, status, duration, revision | Evet |
| `SensitiveMetadata` | öğrenci adı/numarası/sınıf ilişkisi, öğretmen notları | Hayır |
| `StudentContent` | OCR ham metni, cevap, transcript, belge metni, ses yolu | Hayır |
| `ModelPayload` | prompt/system prompt, request/response body | Hayır |
| `Secret` | token, API key, home path, harici kaynak path | Hayır |

## Merkezi sınırlar

- `AppError` internal `technical_details` taşıyabilir ancak Tauri sınırında
  `AppError::to_public()` ile `PublicErrorDto`'ya dönüşür.
- `PublicErrorDto` yalnızca `code`, `safe_message`, `recovery_action`,
  `correlation_id`, `retryable`, `details_available` içerir. Raw path, SQL,
  serde body, HTTP yanıtı, PID, argv ve öğrenci içeriği serileştirilmez.
- `AppError` için özel `Serialize` uygulaması yalnız public DTO üretir;
  `Deserialize` hem public hem legacy şekli kabul eder (job rehydration).
- Frontend `AppError` tipi `safeMessage` / `recoveryAction` / `retryable` /
  `detailsAvailable` kullanır; `technicalDetails` alanı yoktur.

## Kalıcı loglar

Production lib kaynaklarında `println!`, `eprintln!`, `dbg!` yoktur
(proof_31 tarafından taranır). Model gateway hata mesajları yalnız güvenli
özet içerir: `status=502 received_bytes=32768 content_type=text/html`.

## Sentinel leak testi

Sentineller: `STUDENT_SECRET_9f4a`, `OCR_SECRET_17ce`,
`TRANSCRIPT_SECRET_41bd`, `PROMPT_SECRET_a821`, `MODEL_SECRET_47bf`,
`HOME_SECRET_PATH`. `proof_18` hata/cancel/backup/audit akışlarını çalıştırıp
proje log/audit/backup dosyalarını tarar; sıfır leak beklenir.
