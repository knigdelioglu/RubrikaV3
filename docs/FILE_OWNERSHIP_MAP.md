# RubrikaV3 - File Ownership Map (Nerede Ne Değiştirilir?)

Bu doküman, spesifik bir işi yaparken hangi dosyaları değiştirmeniz gerektiğini (ve hangilerine kesinlikle dokunmamanız gerektiğini) gösteren bir referans haritasıdır.

| Yapmak istediğim şey | Önce bakılacak dosyalar | Muhtemel değişecek dosyalar | Dokunulmaması gereken yerler | Test komutları |
| -------------------- | ----------------------- | --------------------------- | ---------------------------- | -------------- |
| **OCR promptunu değiştirmek** | `student_answer_ocr_service.rs`, `llama_server_gateway.rs` | `llama_server_gateway.rs` (Prompt yapısı `extract_student_answer_ocr` içinde) | UI bileşenleri, `Student` domain modeli, Frontend (TypeScript) | `cargo test`, `npm run typecheck` |
| **OCR önizleme (crop) sorununu çözmek** | `student_answer_ocr_service.rs` (Kırpma mantığı), `PdfPageViewer.tsx` | `student_answer_ocr_service.rs` (Crop hesaplama kodları), `pdf_service.rs` (Render) | `llama_server_gateway.rs` (Model gateway görsellerle ilgilenmez, sadece URL bekler) | `cargo test` |
| **OCR image preprocess / crop enhancement** | `ocr_image_preprocess_service.rs`, `student_answer_ocr_service.rs`, `CropTemplatePage.tsx`, `StudentAnswerOcrPage.tsx` | `OcrImagePreprocessService`, preprocess cache/version yazımı, OCR input seçimi, preprocess mode metadata, preview toggle | `llama_server_gateway.rs` (sadece hazırlanmış görüntüler), scoring/QEP gate | `cargo test`, `npm run typecheck` |
| **Student OCR crop servisi / answer region düzeltmek** | `student_answer_crop_service.rs`, `student_answer_ocr_service.rs`, `PdfPageViewer.tsx` | `student_answer_crop_service.rs` (crop bbox, preview eşleme, fallback işaretleme), `student_answer_ocr_service.rs` (servis kullanımı), `StudentAnswerOcrPage.tsx` (uyarı gösterimi) | `llama_server_gateway.rs` (sadece hazırlanmış görüntü URL'leri), QEP/scoring akışı | `cargo test`, `npm run typecheck` |
| **Manuel OCR crop şablonu eklemek/düzeltmek** | `student_answer_crop_service.rs`, `domain/student.rs`, `CropTemplatePage.tsx` | `StudentAnswerCropTemplate` modeli, `save_student_answer_crop_template` komutu, `PdfPageViewer` çizim UI'ı | QEP/scoring gate, model prompt/parser | `cargo test`, `npm run typecheck`, `npm test` |
| **Model path / port config değiştirmek** | `model_config_service.rs`, `model_process_manager.rs` | `model_config_service.rs` (JSON konfigürasyon kaydı) | `llama_server_gateway.rs` (Gateway sadece verilen URL'ye istek atar) | `cargo test` |
| **İş akışı (Workflow) sırasını veya engellerini düzeltmek** | `workflow_engine.rs`, `workflow.rs` | `workflow_engine.rs` (Sıralı if/else blokları) | React sayfaları (Örn: UI üzerinden butonu aktifleştirmeye çalışmayın!) | `cargo test` (Özellikle workflow birim testleri) |
| **Yeni bir asenkron Job eklemek** | `job_manager.rs`, `job.rs` | `job.rs` (`JobKind` enum), komutu başlatan servis dosyası | - | `cargo test` |
| **Job persistence, rehydration, cancellation ve duplicate prevention** | `job_manager.rs`, `job.rs`, `JOB_LIFECYCLE_AND_CANCELLATION.md` | `job_manager.rs`, `job_commands.rs`, `JobSnapshot` DTO, cancellation token checkpoints, GlobalJobCenter UI | Model runtime lease mimarisi, transactional ProjectStore | `cargo test`, `npm run typecheck` |
| **Yeni bir Tauri Command eklemek** | `commands.ts` (Frontend API), `commands/` klasörü altındaki uygun dosya (Örn: `project_commands.rs`), `main.rs` | Yeni command fonksiyonu (Rust), Frontend'e sarmalayıcı (TS), `main.rs` (İlklendirme) | Domain modelleri | `npm run typecheck`, `cargo clippy` |
| **Frontend sayfasına yeni bir alan (UI öğesi) eklemek** | İlgili sayfa (Örn: `StudentAnswerOcrPage.tsx`), `types.ts` | Sayfa dosyası, `types.ts` (Eğer API'den ek veri geliyorsa) | Backend Domain / Service (Eğer sadece görsel/UI state bir alan ise) | `npm run lint` |
| **Domain modeline (Örn: `Project` veya `Student`) yeni bir alan eklemek** | `domain/project.rs`, `types.ts` (TS), `project_store.rs` | İlgili Rust domain dosyası, `types.ts` (TypeScript eşleniği), ProjectStore serde/default uyumluluğu | Backend mantığı bozulmadıkça servislerin kendisi | `cargo test`, `npm run typecheck` |
| **Scoring (Puanlama) geçidini (gate) değiştirmek** | `workflow_engine.rs`, ilgili puanlama servisi | `workflow_engine.rs` (QEP Frozen kontrolü) | **KESİNLİKLE** bypass edilmemeli veya zayıflatılmamalıdır (Kural 2.9) | `cargo test` |
| **Kullanıcıya gösterilen yeni bir hata mesajı eklemek** | `domain/errors.rs` | `errors.rs` (`AppErrorCode`), ilgili service (Hatanın fırlatıldığı yer) | UI `ErrorBanner.tsx` (Mesajı otomatik gösterir, oraya dokunmaya gerek yoktur) | `cargo test` |
| **OCR review raw output düzeltmek** | `student_answer_ocr_service.rs`, `StudentAnswerOcrPage.tsx` | Servis parse bölümü, API DTO eklemesi (`types.ts`), uncertainty metadata (`uncertainSpans`, `suggestedCorrections`, `criticalTermWarnings`), UI gösterme bölümü | Raw JSON diagnostics alanı tamamen silinmemelidir. | `npm run typecheck` |
| **OCR sorun inceleme dashboard'u eklemek** | `StudentAnswerOcrIssueReviewPage.tsx`, `studentAnswerOcrUi.ts`, `PdfPageViewer.tsx`, `App.tsx`, `AppLayout.tsx`, `NextActions.tsx`, `types.ts`, `labels.ts`, `student_answer_ocr_commands.rs`, `student_answer_ocr_service.rs` | Actionable OCR issue filtering, somut ifade kartları, deterministic issue candidate extraction, inline text highlight fallback, crop bbox overlay label'ları, öğretmen aksiyonları, sidebar navigasyonu, `rebuild_student_answer_ocr_issues` ile deterministic refresh | Backend OCR save/approve akışı, QEP/scoring gate, partial-answer genellemesi | `npm run typecheck`, `npm run lint`, `cargo test` |
| **Gemma ile OCR issue correction check eklemek** | `StudentAnswerOcrIssueReviewPage.tsx`, `studentAnswerOcrUi.ts`, `types.ts`, `commands.ts`, `student_answer_ocr_commands.rs`, `student_answer_ocr_service.rs`, `student_answer_crop_service.rs`, `llama_server_gateway.rs`, `model_runtime_service.rs` | Strict JSON issue suggestion DTO, issue crop seçimi, teacher-visible model check panel, scope genişletme engeli, otomatik uygulama yok | OCR save/approve akışı, QEP/scoring gate, answerText otomatik değişimi | `npm run typecheck`, `npm test`, `cargo test` |
| **OCR kritik terim belirsizliği deterministic analyzer eklemek** | `student_answer_ocr_service.rs`, `llama_server_gateway.rs`, `scoring_service.rs` | Deterministic post-process analyzer, warning merge mantığı, scoring reconciliation bağlamı | Preprocess akışı, QEP gate, answerText otomatik değiştirme | `cargo test`, `npm run typecheck` |
| **Printed question leak detection değiştirmek** | `student_answer_ocr_service.rs` | `llama_server_gateway.rs` (OCR prompt kuralları), Servis kontrolü | `domain` içindeki OCR status enumu (Eğer yeni bir durum gerekmiyorsa) | `cargo test` |
| **Model autostart düzeltmek** | `model_process_manager.rs`, `model_runtime_service.rs` | `model_process_manager.rs` (Process spawn logiği) | Gateway (Sadece HTTP atar, process yönetmez) | `cargo test` |
| **Doctor/inspect model alanı eklemek** | `diagnostics.rs`, `rubrika.rs` (CLI bin) | İlgili CLI argümanı veya diagnostics struct'ı | `project.rs` (Core domain) | `cargo build` |
| **Student identity doğrulamak** | `student_scan_service.rs`, `StudentIdentityEditor.tsx` | `student_scan_service.rs` | - | `cargo test` |
| **Student identity crop/OCR/persistence** | `StudentIdentityPage.tsx`, `CropTemplatePage.tsx`, `PdfPageViewer.tsx`, `commands.ts`, `types.ts`, `student.rs`, `project.rs`, `student_answer_crop_service.rs`, `llama_server_gateway.rs`, `workflow_engine.rs` | Kimlik crop template, `start_student_identity_ocr`, OCR önerisi persistence, workflow blocker | Student Answer OCR `manual_template` davranışı, QEP/scoring frozen gate | `npm run typecheck`, `cargo test` |
| **Scoring readiness blocker değiştirmek** | `workflow_engine.rs` | `evaluate_workflow_inner` if-else sırası | UI (Kararı backend verir) | `cargo test` |
| **Scoring backend/job/UI** | `ScoringPage.tsx`, `scoringViewModel.ts`, `commands.ts`, `types.ts`, `scoring_service.rs`, `scoring_commands.rs`, `scoring.rs`, `workflow_engine.rs` | `src/pages/ScoringPage.tsx`, `src/pages/scoringViewModel.ts`, `src/api/commands.ts`, `src/api/types.ts`, `src-tauri/src/services/scoring_service.rs`, `src-tauri/src/commands/scoring_commands.rs`, `src-tauri/src/domain/scoring.rs`, `src-tauri/src/services/workflow_engine.rs` | QEP frozen gate, OCR diagnostics, raw model output, score reconciliation / criterion sum validation, `scoringRunId` / `latestScoringRunId` run ayrımı, rerun dedupe, öğrenci bazlı özet UI, ProjectStore dışı kalıcı yazım | `npm run typecheck`, `cargo test` |
| **Yeni error code eklemek** | `errors.rs`, `labels.ts` (Frontend) | `AppErrorCode` (Rust) ve `labels.ts` (Türkçe karşılığı) | Hatanın fırlatıldığı noktadaki asıl veri türleri. | `cargo test`, `npm run lint` |
| **Yeni frontend API type eklemek** | `types.ts`, Rust DTO'ları | TS: `types.ts`, Rust: İlgili struct üzerine `#[derive(Serialize, Deserialize)]` | Katı Domain nesneleri (Frontend DTO'ları core entity olmak zorunda değildir, clone'lanmış DTO'lar kullanılabilir). | `npm run typecheck` |
| **Project.json geriye dönük uyumluluk / yükleme onarımı** | `project_store.rs`, `domain/project.rs`, `domain/student.rs`, `domain/scoring.rs`, `domain/model.rs` | `project_store.rs` içinde iki aşamalı JSON normalize+deserialize akışı, serde path diagnostics, enum fallback/default alanları | UI'da “JSON bozuk” diye yanıltıcı tek mesaj, veri silen otomatik repair | `cargo test`, `cargo clippy` |
| **AI Studio tasarım referansından frontend modernizasyonu** | İlgili `src/pages/*.tsx`, `src/design-reference/ai-studio/*.tsx` | İlgili frontend sayfası. Tasarım aktarılır. | Mock stateler, backend/workflow kurguları. Sadece görsel yapı referans alınmalıdır. | `npm run lint`, `npm run typecheck` |
| **Frontend navigation/app shell erişimi** | `src/app/App.tsx`, `src/app/AppLayout.tsx`, `src/app/projectRoutes.ts` | 5-menülü global sol navigasyon (Ana Sayfa, Sınavlar, Sınıflar ve Öğrenciler, Raporlar, Ayarlar), top-header ders alanı sunumu, global mode toggle kaldırılması | Geçici state yönetimi. Navigasyon yalnızca aktif proje bağlamında çalışır. | `npm run typecheck`, `npm run lint`, `npm test` |
| **Workflow UI modernizasyonu** | `WorkflowPage.tsx`, `WorkflowPanel.tsx`, `NextActions.tsx` | Workflow ekranının görsel düzeni. | İş akışı state'leri backend snapshot'tan beslenir. Frontend'de sahte durumlar oluşturulamaz. | `npm run typecheck`, `npm run lint` |
| **Canonical Sınav Paketi workspace’ini değiştirmek** | `ExamPackageWorkspacePage.tsx`, `examPackageWorkspace.ts`, `projectRoutes.ts`, `App.tsx`, `RubricQuestionCard.tsx`, `NextActions.tsx` | Soru listesi/view-model, question/rubric/freeze sekmeleri, query-param deep-link, compatibility redirect ve package summary sunumu | `WorkflowSnapshot.summary.readiness.examPackageFreeze`, `confirm_all_rubrics` backend gate’i, `ExamPackageFreeze` hash içeriği, ProjectStore persistence ve scoring gate frontend’de yeniden hesaplanamaz/değiştirilemez | `npm test`, `npm run build`, `cargo test` |
| **Student PDF preview UI modernizasyonu** | `StudentScansPage.tsx`, `PdfPreviewPage.tsx`, `PdfPageViewer.tsx`, `AppLayout.tsx`, `commands.ts`, `types.ts` | Öğrenci PDF önizleme ekranının UI modernizasyonu. | Gerçek PdfPageViewer ve preview cache akışı korunmalıdır. | `npm run typecheck`, `npm run lint` |

| **Proje/document path security** | `src-tauri/src/platform/project_paths.rs`, `project_store.rs`, `document.rs`, `document_service.rs`, `pdf_preview_service.rs`, `diagnostics.rs` | `TrustedProjectRoot`, `ManagedProjectPath`, canonical root session, managed relative document storage, containment/symlink/overwrite gates, doctor counters | Frontend absolute path, `Project.root_path` üzerinden save hedefi, UI-only validation, QEP/scoring gates | `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `npm run check:all` |
| **Model runtime ownership / process safety** | `model_process_manager.rs`, `model_runtime_service.rs`, `platform/process_inspector.rs`, `domain/model.rs`, `domain/errors.rs` | Verified `Child` ownership, persisted identity, startup single-flight, lease registry, profile compatibility, idle shutdown, draining, exit recovery | OCR/scoring/rubric/speaking/analysis servislerinde doğrudan process start/stop, PID-only kill, UI readiness hesabı | `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `npm run check:all` |

---

## Genel Kurallar

| **Source-preserving integrity recovery** | services/integrity_recovery_service.rs, backup_service.rs, transaction_journal.rs, bin/rubrika.rs | External verified backup, recursive byte manifest, forensic audit/orphan reports, append-only RecoveryAnchor and isolated repaired candidate | Real source path; source project lock/audit/project.json/metadata/GC/repair writes | final_data_loss_proofs, preflight, backup-verify, verify-restore |
| **Recovery-copy UI/API boundary** | backup_commands.rs, commands.ts, types.ts, AppLayout.tsx | Job-based dry-run/recovery-copy request and teacher-facing write blocker | Direct source repair, source migration, silent success | npm run typecheck, Rust command contract |

1. **İş (Domain) Mantığı UI'da Olmaz:** Eğer bir butonun "ne zaman aktif olacağına" dair bir kural ekliyorsanız, bunu `WorkflowPanel.tsx` gibi React dosyalarında yazmayın. Kuralı `workflow_engine.rs` dosyasına ekleyin, UI sadece `workflowSnapshot`'ı okusun.
2. **Kırılgan Hatalar (Panic/Unwrap) Yok:** Rust tarafında asla `.unwrap()` veya `panic!()` kullanmayın. Her zaman `Result<T, AppError>` döndürün ve `?` operatörünü kullanın.
3. **Model JSON'ına Güvenmeyin:** Modelden (Gemma) dönen cevaba asla tam bir JSON'mış gibi %100 güvenmeyin. Parse işlemleri (örneğin `llama_server_gateway.rs` içindeki `extract_assistant_content` ve string temizleme) hata tolere edebilir yapıda olmalıdır. Her ihtimalde raw output'u saklayın.
4. **Placeholder Data Gerçek Veri Değildir:** Arayüzdeki yer tutucu metinler, projenin durumunu 'hazır' hale getirmez. Durum her zaman explicitly (`status: "missing"`, `"suggested"`, `"confirmed"`) tutulur.

PDF preview navigation/document type ayrımı: AppLayout.tsx, App.tsx, PdfPreviewPage.tsx, DocumentsPage.tsx, StudentGroupingPage.tsx

Soru metni/rubrik/paket UI modernizasyonu: `ExamPackageWorkspacePage.tsx`, `examPackageWorkspace.ts`, `RubricQuestionCard.tsx`, `projectRoutes.ts`, `NextActions.tsx`; backend sözleşmesi değişecekse ayrıca `rubric_commands.rs`, `rubric_service.rs`, `workflow_engine.rs`.

Student Answer OCR UI modernizasyonu: StudentAnswerOcrPage.tsx, StudentAnswerOcrIssueReviewPage.tsx, studentAnswerOcrUi.ts, PdfPageViewer.tsx, commands.ts, types.ts

Model status UI modernizasyonu: ModelStatusPage.tsx, commands.ts, types.ts, model_runtime_service.rs

Student identity verification UI/backend persistence: StudentIdentityPage.tsx, AppLayout.tsx, commands.ts, types.ts, student.rs, workflow_engine.rs

Student identity crop/OCR/persistence: StudentIdentityPage.tsx, CropTemplatePage.tsx, PdfPageViewer.tsx, commands.ts, types.ts, student.rs, project.rs, student_answer_crop_service.rs, llama_server_gateway.rs, workflow_engine.rs

## Final pre-use audit ownership delta (2026-08-02)

| Alan | Güncel sahibi | Denetim sınırı |
| --- | --- | --- |
| Salt-okunur veri kaybı preflight | `src-tauri/src/diagnostics.rs` + `src-tauri/src/bin/rubrika.rs` | `DataLossPreflightReport`; migration/recovery/repair yok; gerçek proje üzerinde yalnız read-only çalışır |
| Document import activation | `src-tauri/src/services/document_service.rs` | `.importing` staging, kaynak/kopya hash eşitliği, trusted-root rename, metadata commit sonrası orphan cleanup |
| Generation GC ordering | `src-tauri/src/commands/generation_gc_commands.rs` + `generation_gc_service.rs` | Production command metadata-first; service-level direct caller metadata orderingini garanti etmez |
| Audit append serialization | `src-tauri/src/services/audit_service.rs` | Project OS lease + append/sync hash chain; tüm legacy command caller’larında audit error coupling ayrıca gözden geçirilmeli |
| Final release decision | `docs/FINAL_PRE_USE_DATA_LOSS_AUDIT.md` | Kalite/proof suite yeşil olsa da gerçek project preflight blocker’ları (unknown orphan, verified backup yok, invalid audit chain, audit/revision divergence) nedeniyle `DO_NOT_OPEN_FOR_WRITING` |

Superseding 11_46 result: the verified external backup and restore equality
are PASS; source bytes are unchanged; the source remains blocked by UNKNOWN
orphan plus invalid historical audit. Full Cargo/check:all is NOT_VERIFIED in
this environment, so the recovered copy is not promoted to safe-to-open.
## Faz 2 persistence ownership

| Sorumluluk | Tek sahibi | Yazma sınırı |
| --- | --- | --- |
| Proje JSON serialization, revision, fingerprint, per-root lock, atomic replace | `src-tauri/src/services/project_store.rs` | `mutate` / `commit_job`; frontend veya servis doğrudan `project.json` yazmaz |
| Kısa domain mutation | İlgili domain service + `ProjectStore::mutate` | Güncel entity ID'si üzerinde dar closure; await/model/render yok |
| Uzun job commit'i | İlgili job service + `ProjectStore::commit_job` | Snapshot sonrası yalnız owned fields ve source-generation precondition |
| Conflict DTO ve öğretmen mesajı | `src-tauri/src/domain/errors.rs` + API error mapping | Revision/external/entity stale teknik hash veya path sızdırmaz |
| Persistence diagnostics | `ProjectStore::persistence_diagnostics` + `diagnostics.rs` | Sayaçlar runtime diagnostics'te; öğrenci payload'ı eklenmez |

Production writer envanteri ve servis başına owned fields [`docs/PROJECTSTORE_CONCURRENCY.md`](PROJECTSTORE_CONCURRENCY.md) içindedir. `save_project(Project)` yalnız `#[cfg(test)]` fixture uyumluluk yoludur.

## Faz 3 generation ownership

| Alan | Tek sahibi | Güvenli yazma sınırı |
| --- | --- | --- |
| OCR generation geçmişi ve active projection | `student_answer_ocr_service.rs` | Queue/status için `mutate`; model sonucu için `commit_job`; flat active kayıt yalnız validated candidate veya teacher accept sırasında güncellenir |
| Preview staging/generation dosyaları | `pdf_preview_service.rs` + `TrustedProjectRoot` | Staging'e render, manifest/page validation, immutable generation rename; active pointer yalnız `commit_job` sonrasında değişir |
| Submission dependency scan ve delete | `student_scan_service.rs` | Scan önce ve transaction closure içinde; dependency varsa `StudentSubmissionInUse`, cascade yok |
| Batch delete | `school_class_service.rs` | Single delete ile aynı OCR/scoring/job scan; metadata commit'ten sonra best-effort artifact cleanup |
| Generation diagnostics/GC adayları | `diagnostics.rs` ve ilgili servisler | Sadece PII'siz sayaçlar; active/pending/referenced generation silinmez |

## Faz 5 / 5B job ownership

| Sorumluluk | Tek sahibi | Güvenli yazma sınırı |
| --- | --- | --- |
| Job lifecycle state machine & persistence | `jobs/job_manager.rs` | 7 canonical states, `cancellation_token` map, atomic duplicate check |
| Job execution guard & panic protection | `JobTaskGuard` (`job_manager.rs`) | Background task drop sonrasında otomatik `Cancelled` veya `Failed` geçişi |
| Job retry & correlation chain | `JobManager::retry_job` | Terminal işten yeni job ID, `retry_of_job_id` takibi, taze `correlation_id` |
| History retention cleanup | `JobManager::cleanup_job_history` | Active ve retry-referenced işlerin korunması, top `N` terminal job saklanması |
| Controlled app shutdown | `JobManager::shutdown_all_jobs` | `accepting_new_jobs = false`, cancel signal ve `Interrupted` rehydration |

## Faz 6 security ownership

| Alan | Tek sahibi | Güvenli yazma sınırı |
| --- | --- | --- |
| Proje yazma kilidi (OS) | `platform/project_write_lease.rs` | `flock LOCK_EX|LOCK_NB`; lease olmadan `ProjectStore` writer açılmaz; crash'te OS release |
| App single-instance | `platform/single_instance.rs` | App-support dizininde OS flock; ikinci instance writer başlatmaz |
| Audit zinciri | `services/audit_service.rs` | Append-only JSONL + sha256 `previous_record_hash`; kritik karar yazılamazsa fake success yok |
| Backup/restore | `services/backup_service.rs` | Manifest+hash doğrulama, staging, atomic activation; `root_path` arşivden otorite alınmaz |
| Generation retention/GC | `services/generation_gc_service.rs` | Protected/referenced asla silinmez; bounded per-run; dry-run plan |
| Model transport limitleri | `services/llama_server_gateway.rs` | Streaming byte limit, timeout'lar, content-type doğrulaması; raw body loglanmaz |
| Managed asset serving | `platform/managed_asset.rs` + `managed-asset://` | Yalnız relative managed path; traversal/symlink reddi; 32 MiB bound; scope genişletilmez |
| Public error DTO | `domain/errors.rs` (`PublicErrorDto`) | `technical_details` Tauri sınırını geçmez; frontend yalnız safe fields alır |
