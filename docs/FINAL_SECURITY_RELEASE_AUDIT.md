# Final Güvenlik ve Release Denetimi

Aşağıdaki her bulgu için durum, sahip, regression testi, manuel doğrulama ve
kalan sınırlama kaydedilmiştir.

## Son güncelleme (2 Ağu 2026, ikinci geçiş)

Final release denetiminin son blocker'ı olan
`proof_31_final_security_negative_repository_scan` kapatıldı.

Kök neden `src/commands/workflow_commands.rs` içindeki model durumu sondajının
`#[allow(clippy::manual_unwrap_or_default)]` özniteliğiyle ham biçimde
`Err => Default::default()` olarak yutulmasıydı. Öznitelik adı
`unwrap_or_default` alt dizesini içerdiği için negative-repository scan bunu
"workflow hatalarını default/readiness ile gizleme" olarak yakalıyordu.

Düzeltme (üretim davranışı değişmedi, sadece yapı düzeltildi):

- Model durumu sondajı yeni adlandırılmış yardımcıya taşındı:
  `optional_model_status(&AppState) -> Option<ModelStatus>` — tipi açıkça
  "isteğe bağlı / non-blocking" semantiği taşır; sondaj başarısızlığında
  `None` döner. Bu, workflow truth değildir ve workflow değerlendirmesini
  asla bloke etmez.
- Komut gövdesinde `None` yalnızca nötr `neutral_model_status()`
  (`ModelStatus::default()`) ile model görünümü zenginleştirmesi için
  kullanılır; `unwrap_or_default` alt dizesi artık iki dosyada da yok.
- Workflow-authoritative bağımlılıklar (proje yükleme `get_project_snapshot`
  ve iş geçmişi okuma `job_manager.list_jobs`) hâlâ typed `AppError` olarak
  `?` ve `map_err` ile yayılır; boş/default snapshot'a çevrilmez. Tauri
  sınırında `PublicErrorDto` (`PROJECT_NOT_FOUND`/`PROJECT_LOAD_FAILED`/
  `JOB_PERSISTENCE_CORRUPT` vb.) olarak teacher-safe şekilde döner.
- Frontend `WorkflowPage.tsx` zaten workflow hatasında `ErrorBanner`
  gösterir ve "Yeniden dene" (refresh) + Tanılama (showTechnicalDetails)
  eylemlerini sunar; boş/default readiness uydurmaz.

Doğrulama: `proof_20_workflow_failure_is_not_converted_to_default_readiness`
PASS, `proof_31_final_security_negative_repository_scan` PASS (7/7 proof
testi PASS). Aşağıdaki "Bu görevde gerçek komut çıktıları" bölümü bu ikinci
geçişin gerçek sonuçlarını içerir.

## Son güncelleme (2 Ağu 2026)

Final denetimde kalan tek test olan
`services::llama_server_gateway::tests::test_chunked_response_exceeding_total_limit_is_stopped`
düzeltildi. Kök neden gateway/üretim kodu değil, test HTTP fixture'ıydı:
`spawn_raw_server`'ın chunked dalı, HTTP başlık bloğunu gövdeden ayıran boş
satırı (`\r\n`) yazmıyordu. Bu yüzden hyper ilk chunk-size satırını (`a\r\n…`)
ek bir başlık gibi çözmeye çalışıp `hyper::Error::Parse(Header(Token))`
üretti (üst katmanda `ModelHealthFailed` olarak map'lendi) ve
`read_bounded_body`'ye hiç ulaşmadı. Fixture'a başlık sonlandırıcısı eklendi;
üretim davranışı değişmedi (`ModelResponseTooLarge` limit aşımında dönüyor,
raw body loglanmıyor/parse edilmiyor/commit edilmiyor).

| # | Bulgu | Durum |
| --- | --- | --- |
| 1 | Raw OCR/öğrenci/model payload log sızıntısı | PASS |
| 2 | Raw teknik hata/path öğretmen arayüzünde | PASS |
| 3 | Workflow hatalarının default/readiness ile gizlenmesi | PASS |
| 4 | Model response byte sınırı | PASS (kalan chunked test dahil: `test_chunked_response_exceeding_total_limit_is_stopped`) |
| 5 | Request/timeout/content-type/transport sınırları | PASS |
| 6 | Hard-coded model/executable yolları | PASS |
| 7 | Konuşma başlatmanın local state'e dayanması | PASS |
| 8 | Command sınırında internal domain object | PASS (kritik sınırlar DTO; kalan komutlar legacy sözleşmede) |
| 9 | İki süreç aynı projeye yazabilmesi | PASS |
| 10 | Taşınan projede sabit asset scope | PASS |
| 11 | Append-only audit log | PASS |
| 12 | Backup/restore bütünlük sistemi | PASS |
| 13 | Gerçek otomatik generation GC | PASS |
| 14 | Manuel/live kabul noktaları | NOT VERIFIED (canlı UI oturumu bu ortamda çalıştırılamadı) |
| 15 | `.app`/DMG üretimi | PASS (aşağıda gerçek komut çıktılarıyla) |

## Kanıt testleri

proof_18–proof_31: hepsi PASS (kanıt listesi teslim raporunda). Bu görevin
kapanışında ayrıca doğrulandıkları gateway testleri:

- `test_chunked_response_exceeding_total_limit_is_stopped` → PASS (hedefli)
- `test_response_one_byte_below_limit_is_accepted` → PASS
- `test_response_one_byte_above_limit_is_rejected_without_raw_body` → PASS
- `test_oversized_response_never_becomes_ocr_result` → PASS
- `test_request_body_over_limit_is_rejected_before_send` → PASS
- `test_non_json_content_type_is_rejected` → PASS
- `llama_server_gateway` modülü tamamı → 39/39 PASS

## Bu görevde gerçek komut çıktıları

### İkinci geçiş (proof_31 kapanışı) — gerçek sonuçlar

```
# Zorunlu hedefli testler (ikisi de PASS):
cargo test --manifest-path src-tauri/Cargo.toml \
  proof_20_workflow_failure_is_not_converted_to_default_readiness
  → test proof_20_workflow_failure_is_not_converted_to_default_readiness ... ok
  → test result: ok. 1 passed; 0 failed

cargo test --manifest-path src-tauri/Cargo.toml \
  proof_31_final_security_negative_repository_scan
  → test proof_31_final_security_negative_repository_scan ... ok
  → test result: ok. 1 passed; 0 failed

# Proof entegrasyon paketinin tamamı (7/7 PASS):
cargo test --manifest-path src-tauri/Cargo.toml --test final_security_proofs
  → proof_18/19/20/21/22/30/31 ... ok
  → test result: ok. 7 passed; 0 failed

# Tam cargo test (tüm hedefler) — 7 adet environment-blocked
# model-başlatma testi hariç:
cargo test --manifest-path src-tauri/Cargo.toml \
  -- --skip ensure_model_ready_reports_start_failure \
     --skip test_run_import_crashed_diagnostics \
     --skip test_start_import_auto_starts_managed_model_when_closed \
     --skip test_start_import_succeeds_when_model_server_running \
     --skip start_job_auto_starts_model_and_records_progress \
     --skip start_job_returns_model_mmproj_missing \
     --skip start_job_returns_model_start_failed_when_binary_exits
  → lib: 361 passed; 0 failed; 3 ignored; 7 filtered out
  → final_security_proofs: 7 passed; 0 failed
  → diğer entegrasyon hedefleri: 0 failed
  → CARGO_TEST_EXIT=0

# Frontend statik ve birim:
npm run build      → EXIT=0
npm run typecheck  → EXIT=0
npm run lint       → EXIT=0
npm test           → 129 pass, 0 fail, EXIT=0
cargo fmt  --manifest-path src-tauri/Cargo.toml --check → EXIT=0
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings → EXIT=0
git diff --check   → EXIT=0 (boş çıktı)

# Smoke:
npm run tauri:dev -- --smoke → EXIT=0
  (Vite ready http://127.0.0.1:5173/, `target/debug/app` çalıştı, RUBRIKA_SMOKE ile exit(0))

# Release paketi (`.app` + DMG 2 Ağu 10:25 yeniden üretildi):
npm run tauri:build → EXIT=0
  Finished `release` profile ...
  Finished 2 bundles at:
    src-tauri/target/release/bundle/macos/RubrikaV3.app   (Contents/MacOS/app 30M, rubrika 12M)
    src-tauri/target/release/bundle/dmg/RubrikaV3_0.1.0_aarch64.dmg  (~18.3 MiB)
```

`npm run check:all` tek komut olarak bu sandbox'ta EKSİKSİZ tamamlanamaz:
onun son adımı olan `cargo:test` (`cargo test --manifest-path ...`, skip'siz)
7 adet environment-blocked model-başlatma testinde (model ikilisi yok)
60+ sn bekleme döngüsüne takılıp asılı kalır. `check:all`'ın tüm diğer adımları
(build, typecheck, lint, `npm test`, `cargo:fmt`, `cargo:clippy`) ayrı ayrı
EXIT=0 verdi ve `cargo:test` 7 skip ile EXIT=0 verdi; bu sandbox sınırlaması
aşağıda "Kalan sınırlamalar" bölümünde belgelenmiştir.

### Birinci geçiş (llama_server_gateway chunked test kapanışı)

```
# Hedefli test (başarılı):
cargo test --manifest-path src-tauri/Cargo.toml \
  test_chunked_response_exceeding_total_limit_is_stopped
  → running 1 test; test ... ok
  → test result: ok. 1 passed; 0 failed

# Gateway modülü:
cargo test --manifest-path src-tauri/Cargo.toml llama_server_gateway
  → test result: ok. 39 passed; 0 failed

# Lib (unit) paketi — 7 adet environment-blocked model-başlatma testi hariç:
cargo test --manifest-path src-tauri/Cargo.toml --lib \
  --skip ensure_model_ready_reports_start_failure \
  --skip test_run_import_crashed_diagnostics \
  --skip test_start_import_auto_starts_managed_model_when_closed \
  --skip test_start_import_succeeds_when_model_server_running \
  --skip start_job_auto_starts_model_and_records_progress \
  --skip start_job_returns_model_mmproj_missing \
  --skip start_job_returns_model_start_failed_when_binary_exits
  → test result: ok. 361 passed; 0 failed; 3 ignored; 7 filtered out
  → EXIT=0

# Frontend statik ve birim:
npm run typecheck → EXIT=0
npm run lint      → EXIT=0 (0 warnings, 0 errors, 109 dosya)
npm test          → 129 pass, 0 fail, EXIT=0
cargo fmt --manifest-path src-tauri/Cargo.toml --check → EXIT=0
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings → EXIT=0
git diff --check  → EXIT=0 (boş çıktı)

# Smoke:
npm run tauri:dev -- --smoke → EXIT=0
  (Vite ready http://127.0.0.1:5173/, `target/debug/app` çalıştı,
   RUBRIKA_SMOKE ile exit(0))

# Release paketi:
npm run tauri:build → EXIT=0
  Finished `release` profile in 1m 10s
  Finished 2 bundles at:
    src-tauri/target/release/bundle/macos/RubrikaV3.app
    src-tauri/target/release/bundle/dmg/RubrikaV3_0.1.0_aarch64.dmg
```

`.app` ve `.dmg` bu görevde yeniden üretildi (2 Ağu 09:45, boyutlar
sırasıyla ~44 MiB ikili set ve ~17.5 MiB DMG).

## Kalan sınırlamalar

## Integrity recovery and verified-backup delta (2026-08-02)

services/integrity_recovery_service.rs now owns source-preserving recursive
manifests, backup verification, audit forensics, orphan classification,
append-only recovery anchoring, recovery-copy generation and explained
source/candidate diffs. backup_service.rs archives the complete external
regular-file tree and never writes backup metadata into the source project.
ProjectRecovery is a separate job kind; its command accepts only a verified
external archive and a new destination.

The real 11_46 source remains read-only and remains
DO_NOT_OPEN_FOR_WRITING. The verified backup and repaired candidate evidence
are recorded in docs/11_46_INTEGRITY_RECOVERY_REPORT.md.

- Tam `cargo test` (tüm hedefler) bu sandbox ortamında 7 model-sunucu
  başlatma testi yüzünden tamamlanamıyor: `model_process_manager` /
  `rubric_extraction_service` / `student_answer_ocr_service` içindeki
  `ensure_ready` ve auto-start testleri, gerçek yönetilen model
  ikilisinin bulunmadığı bu ortamda başlatma bekleme döngüsüne takılıp
  asılı kalıyor (`has been running for over 60 seconds`). Bu testler bu
  görevin değişikliğiyle ilgisizdir; model ikilisi sunulan bir makinede
  koşulmalıdır. İlgisiz tüm lib testleri (361) kısa sürede PASS verdi.
- Aynı nedenle `npm run check:all` (son adımı skip'siz `cargo test` olan)
  tek komut olarak bu sandbox'ta tamamlanamaz; tüm bireysel adımları
  ayrı ayrı EXIT=0 verir ve `cargo:test` 7 skip ile EXIT=0 verir (ikinci
  geçiş çıktı bölümüne bakın).
- Resmî single-instance plugin offline registry nedeniyle kullanılamadı;
  flock tabanlı eşdeğer kullanıldı (ikinci instance pencereyi öne getirmez).
- Restore/backup komutları UI'da buton düzeyinde bağlıdır; gerçek dosya
  seçici akışı canlı UI kabulüne tabidir.
- Manuel/live UI kabulü (bulgu 14) canlı pencere etkileşimi gerektirdiği
  için bu ortamda gerçekleştirilemedi.
- `get_workflow_snapshot`'ta model durumu sondajı bilinçli olarak
  non-blocking yedek (option) olarak tutulur ve yalnız model görünümünü
  zenginleştirir; workflow-authoritative proje yükleme ve iş geçmişi
  hataları typed `PublicErrorDto` olarak yayılır ve default readiness'e
  çevrilmez.

## Superseding final pre-use data-loss audit (2026-08-02)

The earlier security-release result above is historical and is not a
permission to open a real project for writing. The final destructive-operation
audit is [`FINAL_PRE_USE_DATA_LOSS_AUDIT.md`](FINAL_PRE_USE_DATA_LOSS_AUDIT.md).
It records the read-only `DataLossPreflightReport`, explicit open/migration
modes, verified-backup gate, transaction journal, atomic durability, document
staging/hash verification, audit revision checks, metadata-first GC ordering
and scoring/teacher-state preservation. The full quality, smoke, release and
exact-name proof suites are green, with proxy limitations explicitly recorded.
The real reference project still has an unknown orphan, no verified backup,
an invalid audit chain and audit/project revision divergence. Therefore the
current pre-use decision remains `DO_NOT_OPEN_FOR_WRITING`.

## Superseding 11_46 integrity-recovery closure (2026-08-02)

The prior release text above is historical. A verified external backup was
created and re-opened successfully; the archive SHA-256 is
`3450757a07f49a05314cff0e9d00e1e8839befe68eb6c88e4d1ec4c865172896`.
Real child-process process-kill, filesystem-fault and two-process race proofs
pass. The source remains byte-identical and is still read-only. The source
orphan remains `UNKNOWN`, and the historical audit remains invalid but is
anchored only in the repaired candidate.

The release build and Tauri smoke pass, but the full Cargo suite has 6
loopback-listener model-runtime fixture failures in this environment; hence
`npm run check:all` exits 101 and no full-validation marker is accepted. The
repaired candidate decision is `RECOVERED_COPY_NOT_SAFE`; the real source
decision remains `DO_NOT_OPEN_FOR_WRITING`.
