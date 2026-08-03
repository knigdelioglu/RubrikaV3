# RubrikaV3 - Project Map

Bu doküman RubrikaV3 projesinin yüksek seviye mimarisini, klasör yapısını, ana dosyalarını ve temel veri akışlarını açıklamaktadır.

## A. Genel Mimari

RubrikaV3, modern bir Tauri uygulamasıdır. Backend tarafı (domain logic, dosya işlemleri, model etkileşimi) Rust ile yazılmış olup, Frontend tarafı (kullanıcı arayüzü, state yönetimi) React ve TypeScript kullanılarak geliştirilmiştir.

Uygulamanın genel veri akışı şöyledir:

```text
Frontend React/TypeScript (örn. `src/pages/StudentAnswerOcrPage.tsx`)
→ Tauri commands (örn. `src-tauri/src/commands/student_answer_ocr_commands.rs`)
→ Rust services (örn. `src-tauri/src/services/student_answer_ocr_service.rs`)
→ Domain models (örn. `src-tauri/src/domain/student.rs`, `src-tauri/src/domain/workflow.rs`)
→ ProjectStore persistence (örn. `src-tauri/src/services/project_store.rs` ile diskte atomik JSON kaydı)
→ Jobs (örn. `src-tauri/src/jobs/job_manager.rs` ile asenkron arka plan işlemleri)
→ ModelRuntime / llama-server (örn. `src-tauri/src/services/llama_server_gateway.rs` üzerinden model çağrıları)
```

**Temel Kural:** Frontend **asla** kendi kendine iş akışı (workflow) kararını vermez veya `llama-server` ile doğrudan konuşmaz. Tüm domain kuralları Rust tarafındadır. Frontend sadece "Snapshot" alır ve UI gösterir.

Konuşma sınavı özelinde `SpeakingExamService` tek Gemma 4 12B text-only runtime ile segment cleanup ve evidence-level evaluation çağrılarını yürütür; cleanup gate, scoring policy ve provenance backend'dedir.

Assessment organization için canonical zincir şöyledir:

```text
Project setup (`/project/:projectId/classes`)
  → SchoolClass / SchoolClassService
  → TeachingAssignment filtrelemesi
  → AssessmentOrganizationPage list-first create mode
  → AssessmentActivity (tek ortak sınav)
  → Canonical Exam Workspace (`/project/:projectId/activities/:activityId/:step`)
  → ClassApplication (activity + schoolClassId tekilliği)
  → SpeakingAttempt / StudentSubmissions / StudentAnswerOcrRecords
```

`CanonicalExamWorkspacePage` ve `CanonicalExamWorkspaceHeader` sınav açıldığında global menüyü değiştirmeden ana içerik alanında sınav türüne (Yazılı, Dinleme, Konuşma) özel 5 adımlı canonical çalışma alanını gösterir. Adım durumları (`not_started`, `ready`, `in_progress`, `needs_review`, `completed`, `blocked`) yalnız `WorkflowSnapshot` ve backend artifact durumundan türetilir. Sınıf filtresi yalnız `activity.classApplications` kayıtlarını sunar; sınıf değişiminde geçici görünüm state'i temizlenir. `SpeechExamPage` ve diğer üretim sayfaları bu adımlar altında wrapper/adapter olarak kullanılır.

---

## B. Klasör Haritası

### Frontend (`src/`)
* **`src/`**: React uygulamasının kök dizini. Uygulamanın giriş noktası (`main.tsx`) burada bulunur.
* **`src/pages/`**: Uygulamanın tam sayfa (route) bileşenleri. (Örn: `StudentAnswerOcrPage.tsx`, `WorkflowPage.tsx`). Sayfalar veriyi çeker ve bileşenleri render eder.
* **`src/components/`**: Yeniden kullanılabilir React bileşenleri. (Örn: `common/LoadingButton.tsx`, `pdf/PdfPageViewer.tsx`). Sayfalardan bağımsızdırlar.
* **`src/api/`**: Rust (Tauri) tarafı ile iletişim kuran istemci kodu. `commands.ts` tüm Tauri çağrılarını (invoke) sarmalar. `types.ts` ve `errors.ts` ise backend'in döndürdüğü tipleri (DTO) tanımlar.
* **`src/utils/`**: UI tarafında kullanılan yardımcı fonksiyonlar. (Örn: `labels.ts` ile durum kodlarının Türkçeye çevrilmesi).

### Backend (`src-tauri/src/`)
* **`src-tauri/src/`**: Rust uygulamasının kök dizini. `main.rs` ve `lib.rs` (uygulama başlatma ve konfigürasyon) burada yer alır.
* **`src-tauri/src/domain/`**: Çekirdek iş kuralları (Core Domain). Veri yapıları (`Project`, `Student`, `WorkflowSnapshot`), enumlar (`AppErrorCode`, `WorkflowStage`) buradadır. Dış dünyadan (dosya sistemi, ağ) habersizdirler.
* **`src-tauri/src/services/`**: Domain kurallarını işleten, dış dünyayla konuşan katman. (Örn: `project_store.rs`, `llama_server_gateway.rs`, `workflow_engine.rs`). Yeni bir "iş yapan" kod buraya eklenir.
* **`src-tauri/src/commands/`**: Frontend'in çağırdığı Tauri endpoint'leri. Bunlar sadece API denetleyicileri gibidir; HTTP controller gibi çalışırlar. Doğrudan iş yapmazlar, ilgili `service`'i çağırırlar.
* **`src-tauri/src/jobs/`**: Uzun süren (long-running) asenkron işlemlerin yönetimi (`JobManager`, `JobSnapshot`). OCR veya PDF oluşturma gibi işlemler arayüzü kilitlemesin diye buradan yönetilir.
* **`src-tauri/src/bin/`**: Harici binary CLI dosyaları (örn: `rubrika.rs` CLI araçları veya diagnostics için).
* **`src-tauri/src/platform/`**: Dosya yolları (`paths.rs`) ve işletim sistemine özel dosya erişim detayları.
  `project_paths.rs`, `TrustedProjectRoot` ve `ManagedProjectPath` ile proje içi tüm read/write containment, legacy path adaptation, symlink policy ve atomic temp doğrulamasının tek sahibidir.
* **`src-tauri/src/diagnostics.rs`**: Uygulamanın sağlık durumu ve diagnostik raporlama araçları.

---

## C. Ana Dosya Listesi

| Dosya | Katman | Sorumluluk | Girdi | Çıktı | Çağıranlar | Çağırdıkları | Öğrenilecek Rust/TS konusu |
| ----- | ------ | ---------- | ----- | ----- | ---------- | ------------ | -------------------------- |
| `src-tauri/src/domain/project.rs` | Domain | Proje veri yapısını tanımlar (`Project`, `Section`, `ExamPackageFreeze`). | - | - | Tüm servisler | `student.rs`, `workflow.rs` | Serde serializasyon, Option kullanımı |
| `src-tauri/src/domain/workflow.rs` | Domain | İş akışı aşamalarını (`WorkflowStage`) ve engelleyici nedenleri (`BlockingReason`) tanımlar. | - | - | `workflow_engine.rs` | - | Rust Enum'ları ve pattern matching |
| `src-tauri/src/domain/errors.rs` | Domain | Hata tipleri (`AppError`, `AppErrorCode`). UI'a gösterilecek yapılandırılmış hatalar. | - | - | Tüm uygulama | - | Rust'ta Error trait'i, `Result` döndürme |
| `src-tauri/src/services/workflow_engine.rs` | Service | Proje durumuna bakıp bir sonraki `WorkflowSnapshot`'ı hesaplar. Frontend'in "ne yapmalıyım" kararı burada alınır. | `Project` ref | `WorkflowSnapshot` | `project_store.rs` | - | Saf fonksiyon tasarımı, referans kullanımı |
| `src-tauri/src/services/project_store.rs` | Service | Projenin diske yazılması ve okunması (JSON). `project.json` atomik olarak yazılır. | `Project` struct | Diske yazma, okuma Result | Tüm komut ve servisler | `workflow_engine.rs` | Dosya I/O, atomik fsync |
| `src-tauri/src/platform/project_paths.rs` | Platform/security boundary | Kullanıcı tarafından seçilen canonical proje kökü, managed relative path parsing, read/write containment, symlink policy ve legacy path adaptation. | Selected project path, managed path | `TrustedProjectRoot`, `ManagedProjectPath`, safe filesystem targets | `ProjectStore`, document/PDF/OCR/cache/artifact services | `file_access.rs` | Path canonicalization, containment, atomic staging |
| `src-tauri/src/services/llama_server_gateway.rs` | Service | `llama.cpp` sunucusuna HTTP üzerinden prompt atar ve yanıtı parse eder. | `StudentAnswerOcrRequest` vb. | `StudentAnswerOcrResult` | OCR servisleri | `reqwest::Client` | Async I/O, HTTP Client, timeout |
| `src-tauri/src/services/model_runtime_service.rs` | Service | Model sunucusunun (llama-server) çalışıp çalışmadığını, sağlığını ve portunu kontrol eder. | `ModelRuntimeRequest` | `ModelRuntimeStatus` | OCR / Soru servisleri | `model_process_manager.rs` | Process yönetimi, async health check |
| `src-tauri/src/services/ocr_image_preprocess_service.rs` | Service | OCR öncesi crop görüntülerini temiz gri ton / yüksek kontrast / BW opsiyonuyla preprocess eder ve ayrı cache üretir. | `project_root`, `image_path`, `mode` | Preprocessed image path + diagnostics | `student_answer_ocr_service.rs`, `student_answer_ocr_commands.rs` | `image`, project cache | Görüntü ön işleme, deterministic cache |
| `src-tauri/src/services/student_answer_ocr_service.rs` | Service | Öğrenci kağıdını OCR modeline gönderir, preprocess edilmiş crop'u kullanır, dönen sonucu parse eder ve kaydeder. | `project_id`, `StudentSubmission` | Arka plan işi (`JobId`) | `student_answer_ocr_commands.rs` | `llama_server_gateway.rs`, `JobManager`, `ocr_image_preprocess_service.rs` | Async görev arka plana atma (spawn) |
| `src-tauri/src/jobs/job_manager.rs` | Job | Arka plan işlemlerinin durumunu tutar ve Frontend'e event fırlatır. | Job id, progress | Tauri Events | Uzun süren servisler | Tauri `AppHandle` | Thread-safe state (Mutex/RwLock) |
| `src-tauri/src/commands/model_commands.rs` | Command | Frontend'den gelen model konfigürasyon/durum isteklerini karşılar. | UI parametreleri | DTO veya Hata (`AppError`) | Frontend (`api/commands.ts`) | `model_runtime_service.rs` | Tauri state injection, `Result` DTO dönüşü |
| `src/api/commands.ts` | API (TS) | Tauri invoke sarmalayıcıları. | TypeScript tipleri | Promise | React Sayfaları | Tauri `invoke` | Async/await, Typed API |
| `src/pages/DocumentsPage.tsx` | UI (React) | Sınav, cevap anahtarı ve öğrenci PDF’lerini yükleme, preview işi başlatma ve sayfaları aynı çalışma alanında inceleme yüzeyidir. | Document, PdfPreviewStatusSnapshot, JobSnapshot | Birleşik Belgeler workspace’i | Canonical ve compatibility route’ları | `documentWorkspace.ts`, `commands.ts`, ortak PDF viewer bileşenleri | Presentation model, query refresh, erişilebilir responsive workspace |
| `src/pages/documentWorkspace.ts` | UI presentation | Üç belge rolünü öğretmen etiketlerine normalize eder ve mevcut import/preview command ayrımını exhaustive biçimde yönlendirir; readiness hesaplamaz. | Mevcut backend DTO’ları | DocumentWorkspaceItem ve command yönlendirmesi | `DocumentsPage.tsx`, frontend testleri | `api/types.ts` | Discriminated union, exhaustive switch |
| `src/components/pdf/DocumentPreviewViewer.tsx` | UI component | `PdfPageViewer`, `PageNavigation` ve `ZoomControls` bileşenlerini Belgeler içindeki erişilebilir inceleme alanında birleştirir. | PdfPagePreview[] | Sayfa gezinme ve zoom yüzeyi | `DocumentsPage.tsx` | Ortak PDF bileşenleri | Composition, keyboard interaction, responsive overflow |
| `src/pages/ExamPackageWorkspacePage.tsx` | UI (React) | Soru metni, zengin rubrik ve paket dondurma yüzeylerini tek canonical çalışma alanında birleştirir; dondurma readiness bilgisini yalnız backend workflow snapshot’tan okur. | Project, Workflow, Rubric ve Job snapshot’ları | Canonical Sınav Paketi workspace’i | `/project/:projectId/exam/package` | Mevcut question/rubric command wrapper’ları, `examPackageWorkspace.ts` | Query-param deep-link, mutation güvenliği, erişilebilir responsive workspace |
| `src/pages/examPackageWorkspace.ts` | UI presentation | Soru listesi ve özet DTO’larını normalize eder, güvenli deep-link fallback’i sağlar; domain readiness hesabı yapmaz. | Mevcut backend DTO’ları | Soru listesi/özet presentation modelleri | `ExamPackageWorkspacePage.tsx`, frontend testleri | `api/types.ts` | Saf view-model, strict TypeScript |
| `src/pages/StudentAnswerOcrPage.tsx` | UI (React) | OCR sayfasını çizer. Veriyi TanStack Query ile alır. | Backend Snapshots | React JSX | React Router | `commands.ts`, Tauri Events | TanStack Query, React hooks, State yönetimi |
| `src/pages/ScoringPage.tsx` | UI (React) | Notlandırma ekranını öğrenci bazlı özet + açılır detay olarak çizer; backend aktif/latest run snapshot’ını gösterir. | Backend Snapshots | React JSX | React Router | `scoringViewModel.ts`, `commands.ts`, Tauri Events | Accordion UI, duplicate-safe read model, TanStack Query |

### Belgeler workspace ve route ilişkisi

```text
/project/:projectId/exam/documents (canonical)
  → DocumentsPage
  → documentWorkspace presentation modeli
  → PdfPageViewer + PageNavigation + ZoomControls

/documents (legacy) ───────────────┐
/pdf-preview (legacy + query) ─────┼→ canonical Belgeler route’u
/project/:projectId/exam/preview ──┘   (documentId, documentType ve page korunur)
```

`PdfPreviewPage.tsx` yalnız ince compatibility wrapper’dır; bağımsız ana kullanıcı yüzeyi değildir. Ortak PDF viewer bileşenleri crop, OCR review ve graded review gibi diğer tüketiciler için korunur. Preview job’ları sayfadan bağımsız global işlem merkezinde görünmeye devam eder.

### Sınav Paketi workspace ve route ilişkisi

```text
/project/:projectId/exam/package (canonical)
  → ExamPackageWorkspacePage
  → soru listesi + question/rubric/freeze query-param sekmeleri
  → mevcut question_text, rubric ve confirm_all_rubrics command’ları

/project/:projectId/exam/questions ─────┐
/question-text ─────────────────────────┤→ canonical ?tab=question
/project/:projectId/exam/rubrics ──────┤
/rubric-preparation ────────────────────┤→ canonical ?tab=rubric
/project/:projectId/exam/package-review ┤
/exam-package-review ───────────────────┘→ canonical ?tab=freeze
```

Compatibility yönlendirmeleri `projectId`, `questionId`, `criterionId`, review mode ve diğer query parametrelerini korur. `AppLayout` üretim menüsünde bu alan tek `Sınav Paketi` maddesi olarak görünür. Soru metni/rubrik model işleri hem seçili panelde hem route’tan bağımsız global işlem merkezinde aynı job snapshot’larıyla gösterilir.

---

## D. Veri Akışı Örnekleri

### 1. Student Answer OCR (Uzun süren iş)

```text
UI: `StudentAnswerOcrPage.tsx` üzerindeki "Başlat" butonu
→ command: `start_student_answer_ocr` (`api/commands.ts` -> `commands/student_answer_ocr_commands.rs`)
→ service: `StudentAnswerOcrService::start` (Sadece işi başlatır ve hemen döner)
→ job: `JobManager::start_job` (Yeni bir `JobId` oluşturur ve listeye ekler)
→ geri dönen snapshot: `StartStudentAnswerOcrOutput { job_id, status }`
(Arka planda):
→ service `StudentAnswerOcrService::run`: Model sunucusunu kontrol eder, görselleri hazırlar.
→ ModelGateway (`llama_server_gateway.rs`): Modele istek atar.
→ domain model: Parse edilen sonuçlarla `StudentAnswerOcrRecord` oluşturulur.
→ ProjectStore: Yeni records projeye eklenir, workflow güncellenir ve `project.json` kaydedilir.
→ job: `JobManager::succeed` (veya `fail`) çağrılarak UI uyarılır.
```

### 2. OCR Review/Approve

```text
UI: `StudentAnswerOcrPage.tsx` üzerindeki "Onayla" butonu
→ command: `mark_student_answer_ocr_reviewed` (`commands/student_answer_ocr_commands.rs`)
→ service: `StudentAnswerOcrService::mark_student_answer_reviewed`
→ domain model: Proje yüklenir, `StudentAnswerOcrRecord` bulunur, `needs_review = false`, `status = TeacherApproved` yapılır.
→ service: `workflow_engine::evaluate_workflow` çağrılarak yeni stage hesaplanır.
→ ProjectStore: Değişen proje diske kaydedilir.
→ geri dönen snapshot: Güncellenmiş `StudentAnswerOcrRecord` (UI ardından `getProjectSnapshot` yeniler).
```

### 3. Model Çalıştırma / Hazırlık (Workflow Readiness)

```text
UI: `WorkflowPanel.tsx` -> "Soru metnini çıkar" butonu tıklanır
→ command: `start_question_text_extraction`
→ service: `QuestionTextService::start_extraction`
→ ModelRuntimeService: `acquire_ready_runtime_lease(profile, consumer, operation, correlation_id)` çağrılır. Llama sunucusu kapalıysa `model_process_manager` üzerinden çalıştırılır.
→ job: Arka plana görev atılır. Eğer model hazır olmazsa hata JobEvent olarak döner.
```

### 4. OCR issue correction model check

```text
UI: `StudentAnswerOcrIssueReviewPage.tsx` üzerindeki "Gemma ile öneriyi kontrol et" butonu
→ command: `suggest_ocr_issue_correction_with_model`
→ service: `StudentAnswerOcrService::suggest_ocr_issue_correction_with_model`
→ crop service: işaretli bbox varsa daha sıkı issue crop üretir
→ model runtime: `acquire_ready_runtime_lease` ile vision kapasiteli model health + identity doğrulamasından sonra çağrılır
→ ModelGateway (`llama_server_gateway.rs`): strict JSON dönen issue correction isteği atanır
→ geri dönen snapshot: öğretmen onayı gerektiren öneri DTO'su (OCR metni otomatik güncellenmez)
```
## Faz 2 — transactional ProjectStore persistence

`ProjectStore` canonical persistence writer'dır. Proje mutation'ları `mutate` ile proje kökü bazlı lock altında güncel dosyadan okunur, invariant kontrolünden sonra `storage_revision` bir kez artırılır ve Faz 1 atomic replacement yardımcısıyla yazılır.

- `Project.storage_revision`: backend-authoritative monotonik revision; legacy projelerde güvenli başlangıç değeri `0`.
- `ProjectSnapshot`: project, revision, content fingerprint ve trusted root metadata'sını birlikte taşır.
- `commit_job`: uzun işlerin snapshot + narrow commit yoludur; yalnız işin sahip olduğu alanları merge eder ve source generation uyuşmazlığını `stale` olarak döndürür.
- Per-project lock registry farklı trusted root'ların paralel çalışmasına izin verir; global mutex kullanılmaz.
- Production servislerinde blind `save_project(full_project)` yoktur. Fixture/migration uyumluluğu dışındaki yazımlar `mutate` veya `commit_job` üzerinden gider.

Ayrıntılı persistence akışı, ownership matrisi ve conflict sözleşmesi için [`docs/PROJECTSTORE_CONCURRENCY.md`](PROJECTSTORE_CONCURRENCY.md) okunmalıdır.

## Faz 3 — generation safety

- `Project.student_answer_ocr_generations`, `OcrGenerationStatus` ve `OcrTeacherReviewStatus` OCR geçmişini active flat projection'dan ayırır.
- `StudentAnswerOcrService::start` yalnız candidate queue eder; `commit_ocr_generation` source fingerprint doğrulaması ile dar commit yapar. `accept/reject_student_answer_ocr_generation` öğretmen karşılaştırma akışını yönetir.
- `PdfPreviewService` active preview'ı silmeden `outputs/previews/<document_id>/.staging/<generation_id>` altında render eder ve doğrulanmış immutable generation manifest'ini `active_generation_id` pointer'ı ile etkinleştirir.
- `StudentScanService::delete_student_submission` ve `SchoolClassService::remove_student_scan_batch` dependency scan + `commit_job` ile OCR/scoring/job/artifact ilişkilerini kontrol eder.
- `diagnostics.rs` generation, orphan staging ve blocked deletion sayaçlarını PII olmadan raporlar.

Ayrıntılı yaşam döngüsü, recovery, retention/GC ve silme sırası için [`docs/GENERATION_AND_ROLLBACK_SAFETY.md`](GENERATION_AND_ROLLBACK_SAFETY.md) okunmalıdır.

## Faz 4 — model runtime ownership

Model process lifecycle'ının tek sahibi `src-tauri/src/services/model_process_manager.rs`'dir.
`ProcessInspector` (`src-tauri/src/platform/process_inspector.rs`) persisted PID
recovery ve verified stop için native process identity sağlar. `ModelRuntimeService`
lease facade'ıdır; model tüketicileri `acquire_ready_runtime_lease` ile aynı runtime instance'a
bağlanır ve global iş-sonu stop çağrısı yapmaz.

Detaylı identity, Child handle, restart recovery, profile transition, idle
shutdown ve draining sözleşmesi için [`docs/MODEL_RUNTIME_OWNERSHIP.md`](MODEL_RUNTIME_OWNERSHIP.md)
okunmalıdır.

## Faz 5 / 5B / 5C — production job lifecycle and cancellation

Tüm asenkron arka plan işlerinin tek yürütücüsü `src-tauri/src/jobs/job_manager.rs`'dir.
7 durumlu makine (`Queued`, `Running`, `Succeeded`, `Partial`, `Failed`, `Cancelled`, `Interrupted`), `CancellationToken` bazlı iptal, `JobTaskGuard` panik koruması, `retry_job` canonical yeniden deneme, `cleanup_job_history` retention ve `shutdown_all_jobs` kontrollü kapama backend tarafından garanti edilir.
Faz 5C kapsamında 10 domain servisinin tüm uzun işlemlerine (DocumentImport, PdfPreviewRender, QuestionTextExtraction, RubricPdfImport, ExamPackageBuild, StudentAnswerOcr, StudentIdentityOcr, Scoring, SpeakingEvaluation, AssessmentAnalysis) cancellation checkpoint'leri eklenmiş ve 16 production proof testi ile doğrulanmıştır.

Ayrıntılı job envanteri, iptal noktaları ve UI event sözleşmesi için [`docs/JOB_LIFECYCLE_AND_CANCELLATION.md`](JOB_LIFECYCLE_AND_CANCELLATION.md) okunmalıdır.

## Faz 6 — final security, privacy, audit, backup/restore ve GC

## Integrity recovery map (2026-08-02)

- integrity_recovery_service.rs: read-only source manifest, archive
  verification, audit/orphan forensics, recovery anchor and explained diff.
- backup_service.rs: complete external verified regular-file archive,
  per-entry hash verification, atomic staging/restore.
- transaction_journal.rs: incomplete/ambiguous transaction classification;
  ambiguous state blocks writing.
- bin/rubrika.rs: backup-create, backup-verify, restore-copy, verify-restore,
  audit-forensics, classify-orphans, recover-copy, recovery-diff and
  read-only preflight.
- tests/final_data_loss_proofs.rs: real SIGKILL child fixtures, filesystem
  fault fixtures and two-process lease/restore races.

Recovery writes are permitted only below a new destination created from a
verified external backup. The source project is never opened in writable mode
by these flows.

- `platform/project_write_lease.rs`: OS `flock` tabanlı `ProjectWriteLease` +
  process içi paylaşım (`acquire_or_share`); ikinci OS process
  `ProjectAlreadyOpen` alır. `platform/single_instance.rs`: app-level flock
  lease (offline registry nedeniyle resmî plugin yerine güvenli eşdeğer).
- `services/audit_service.rs`: append-only sha256 hash-zincirli audit
  (`logs/audit.jsonl`), tamper/silme tespiti, kritik kararlarda fake success
  yasağı.
- `services/backup_service.rs`: sınırlı `.rbackup` formatı (manifest + entry
  hash), traversal/symlink/duplicate/boyut korumaları, staging + atomic
  activation; restore arşivdeki `root_path`'e güvenmez.
- `services/generation_gc_service.rs`: dry-run plan + koruma seti (active,
  candidate, approved, son başarılı, referenced) + bounded cleanup +
  orphan staging; `run_generation_gc` komutu.
- `services/llama_server_gateway.rs`: streaming `max_response_body_bytes`
  (32 MiB), `max_request_body_bytes` (128 MiB), connect/first-byte/idle
  timeout'ları, content-type doğrulaması.
- `platform/managed_asset.rs` + `managed-asset://` protokolü: taşınan proje
  için backend-authoritative, containment/symlink kontrollü asset serving;
  asset scope genişletilmez.
- `diagnostics.rs` `SecurityDoctorSummary`: privacy, gateway, config,
  speaking, locking, asset, audit, backup ve GC sayaçları (PII'siz).

Detaylar: [`docs/PRIVACY_LOGGING_AND_PUBLIC_ERRORS.md`](PRIVACY_LOGGING_AND_PUBLIC_ERRORS.md),
[`docs/MODEL_GATEWAY_LIMITS_AND_CONFIGURATION.md`](MODEL_GATEWAY_LIMITS_AND_CONFIGURATION.md),
[`docs/PROJECT_LOCK_AND_PORTABLE_ASSETS.md`](PROJECT_LOCK_AND_PORTABLE_ASSETS.md),
[`docs/AUDIT_BACKUP_RESTORE_AND_RETENTION.md`](AUDIT_BACKUP_RESTORE_AND_RETENTION.md),
[`docs/FINAL_SECURITY_RELEASE_AUDIT.md`](FINAL_SECURITY_RELEASE_AUDIT.md).

## Final pre-use data-loss audit delta (2026-08-02)

`docs/FINAL_PRE_USE_DATA_LOSS_AUDIT.md` canonical pre-use karar kaynağıdır.
`rubrika preflight <project_path> [--json]` yalnız recursive inventory,
project parse/revision, active pointer, orphan artifact, audit-chain ve backup
doğrulaması yapar; migration/repair yazmaz. Backup ve document import yolları
project OS lease’i paylaşır; document import staging+hash+rename kullanır.
Generation GC command’ı metadata commit’inden sonra fiziksel cleanup yapar.
Exact-name proof suite’i, quality checks, smoke ve release build yeşildir;
proxy niteliğindeki process-kill/disk-full/race proof’ları raporda ayrıca
belirtilmiştir. Gerçek `11_46` projesinin preflight’ı unknown orphan, verified
backup yokluğu, invalid audit chain ve audit/project revision divergence
bulduğu için karar kesin olarak `DO_NOT_OPEN_FOR_WRITING` kalır.

Superseding 11_46 closure: verified backup and restore equality are PASS, but
the real source still has the UNKNOWN orphan and unrecovered historical audit.
The final source byte manifest is unchanged. Full Cargo/check:all is
NOT_VERIFIED because six model-runtime fixtures cannot bind loopback in this
environment; the repaired candidate remains `RECOVERED_COPY_NOT_SAFE`.
