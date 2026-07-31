# RubrikaV3 - Sembol Haritası (Symbol Map)

Bu doküman, projede önemli işlevleri yerine getiren Rust ve TypeScript sembollerini (Command'ler, Servisler, Model Struct'ları) listeler. Projede aradığınız bir yapı veya işleyişi bu tabloda bularak ilgili kod dosyasına gidebilirsiniz.

## A. Frontend Command Wrappers (TS API)

### Assessment organization canonical symbols

| Sembol | Dosya | Tür | Ne yapar? | Kim çağırır? |
|---|---|---|---|---|
| `AssessmentOrganizationPage` | `src/pages/AssessmentOrganizationPage.tsx` | Page | Liste-first sınav yönetimi yapar; kullanıcı aksiyonuyla açılan create mode aktif TeachingAssignment kayıtlarından ders/sınıf seçenekleri üretir ve tek AssessmentActivity altında ClassApplication kayıtları oluşturur. | Router |
| `CanonicalExamWorkspacePage` | `src/pages/CanonicalExamWorkspacePage.tsx` | Page | Sınav türüne özel 5 adımlı canonical sınav çalışma alanını sunar; üretim sayfalarını adımlar altında wrapper/adapter olarak çalıştırır. | Router |
| `CanonicalExamWorkspaceHeader` | `src/components/workspace/CanonicalExamWorkspaceHeader.tsx` | Component | Sınav workspace başlığını, tür rozetini, bağlı sınıfları, sınıf filtresini ve 5 adımlı erişilebilir adım çubuğunu çizer. | `CanonicalExamWorkspacePage` |
| `SpeechExamPage` | `src/pages/SpeechExamPage.tsx` | Page | `assessmentActivityId` bağlamında yalnız activity class applications listesinden speaking execution yürütür. | Router |
| `createAssessmentActivity` / `getAssessmentActivity` / `getAssessmentClassApplications` / `getClassApplicationStudents` | `src/api/commands.ts` | Func | Activity ve canonical sınıf uygulaması command wrapper’ları. | Organization/Speech pages |
| `deriveExamStepStatuses` / `resolveNextExamStep` / `getExamStepDefinitions` | `src/app/examWorkspace.ts` | Func | Sınav türüne özel adımları, backend-derived adım durumlarını ve Devam et aksiyon hedefini hesaplar. | Workspace pages/components |
| `getAssessmentSequenceOptions` | `src/api/commands.ts` | Func | Eğitim yılı, ders, dönem ve tür için backend’in hesapladığı kullanılabilir sınav sıralarını döndürür. | `AssessmentOrganizationPage` |
| `startSpeakingExamAttempt` | `src/api/commands.ts` | Func | Activity + class application + student referanslarıyla attempt başlatır. | `SpeechExamPage` |
| `ClassesPage` | `src/pages/ClassesPage.tsx` | Page | Canonical proje kurulumu; sınıfları ve ders–sınıf görevlendirmelerini yönetir, sınav organizasyonuna geçiş sunar. | Router |
| Sembol | Dosya | Tür | Ne yapar? | Öğrenilecek Konu |
|---|---|---|---|---|
| `openProject` | `src/api/commands.ts` | Func | Projeyi açmak için Tauri komutunu çağırır. | Tauri invoke sarmalayıcı (Wrapper) |
| `getWorkflowSnapshot` | `src/api/commands.ts` | Func | Backend'den mevcut durum ağacını (Workflow) çeker. | DTO ve TypeScript tipleri (`WorkflowSnapshot`) |
| `startStudentAnswerOcr` | `src/api/commands.ts` | Func | OCR asenkron işini tetikler. `jobId` döner. | Promise tabanlı asenkron Tauri invoke |
| `preprocessOcrImage` | `src/api/commands.ts` | Func | OCR öncesi image preprocess preview / cache sonucunu alır. | Promise tabanlı asenkron Tauri invoke |
| `startScoringJob` | `src/api/commands.ts` | Func | Notlandırma job'unu tetikler, force rerun seçeneğini backend'e taşır. | Promise tabanlı asenkron Tauri invoke |
| `getStudentAnswerOcrActionableIssueEntries` / `getStudentAnswerOcrActionableIssueEntriesForQuestion` / `getStudentAnswerOcrIssueKinds` / `getStudentAnswerOcrIssueKindsForQuestion` / `getStudentAnswerOcrIssueSummary` / `getStudentAnswerOcrIssueHighlightBoxes` / `getStudentAnswerOcrTextHighlights` / `getStudentAnswerOcrTextHighlightsForQuestion` / `applyStudentAnswerOcrSuggestedCorrection` | `src/pages/studentAnswerOcrUi.ts` | Func | OCR issue review dashboard için sadece actionable/somut issue kartları, deterministic candidate extraction, overlay bbox, inline text highlight fallback ve tek-parça suggestion apply üretir; partial-answer genellemesi üretmez. | `StudentAnswerOcrIssueReviewPage.tsx` | `StudentAnswerOcrRecord`, `Question` |
| `getStudentAnswerOcrIssueReviewModelInputRef` | `src/pages/studentAnswerOcrUi.ts` | Func | OCR issue model check için tercih edilen crop / preview görüntü referansını sıralı fallback ile seçer. | `StudentAnswerOcrIssueReviewPage.tsx`, testler | `StudentAnswerOcrRecord` |
| `rebuildStudentAnswerOcrIssues` | `src/api/commands.ts` | Func | Mevcut OCR kayıtlarını model çağırmadan deterministic issue analyzer'dan geçirir. | `StudentAnswerOcrIssueReviewPage.tsx` | `rebuild_student_answer_ocr_issues` |
| `suggestOcrIssueCorrectionWithModel` | `src/api/commands.ts` | Func | Seçili OCR issue için Gemma vision check isteğini backend'e yollar; otomatik düzeltme yapmaz. | `StudentAnswerOcrIssueReviewPage.tsx` | `suggest_ocr_issue_correction_with_model` |
| `compareScoringRecords` | `src/pages/scoringViewModel.ts` | Func | Scoring kayıtlarını zaman damgasına göre sıralar. | Öğrenci özet view-model, active run dedupe |
| `dedupeScoringRecords` | `src/pages/scoringViewModel.ts` | Func | Aynı öğrenci+soru için sadece latest kaydı bırakır. | Öğrenci özet view-model, duplicate-safe toplam |
| `resolveActiveScoringRunId` | `src/pages/scoringViewModel.ts` | Func | Aktif/latest scoring run id'yi seçer. | ScoringPage, legacy fallback |
| `buildStudentSummary` | `src/pages/scoringViewModel.ts` | Func | Öğrenci özet kartı için toplam, badges ve active record görünümü üretir. | ScoringPage |
| `ExamPackageWorkspacePage` | `src/pages/ExamPackageWorkspacePage.tsx` | Component | Soru metni, rubrik ve paket dondurma panellerini canonical `/project/:projectId/exam/package` route’unda birleştirir. | TanStack Query, query-param deep-link, mevcut command wrapper’ları |
| `buildExamPackageQuestionItems` / `buildExamPackageWorkspaceSummary` | `src/pages/examPackageWorkspace.ts` | Func | Backend DTO’larını soru listesi ve paket özet sunumuna normalize eder; freeze readiness’i yalnız workflow snapshot’tan kopyalar. | Saf view-model, frontend testleri |
| `projectNavigation` / `getProjectArea` / `resolveLegacyProjectDestination` | `src/app/projectRoutes.ts` | Func/Var | 5-menülü global öğretmen navigasyonunu (Ana Sayfa, Sınavlar, Sınıflar ve Öğrenciler, Raporlar, Ayarlar), active area haritalamasını ve query parametrelerini koruyan legacy redirect helper'larını sağlar. | `AppLayout.tsx`, `App.tsx`, testler |

## B. Tauri Commands (Rust API Katmanı)

| `get_assessment_sequence_options` / `create_assessment_activity` / `update_assessment_activity` | `assessment_organization_commands.rs` | Commands | Sıra önerilerini hesaplar; ortak sınav tanımını ve speaking configuration snapshot’ını kaydeder. | `AssessmentOrganizationPage` |
| `get_assessment_activity` / `get_assessment_class_applications` / `get_class_application_students` | `assessment_organization_commands.rs` | Commands | Activity’ye bağlı canonical class application ve merkezi roster read modelini döndürür. | `SpeechExamPage` |
| `add_assessment_class_application` / `remove_assessment_class_application` | `assessment_organization_commands.rs` | Commands | Sınıf uygulaması ekler veya attempt varsa servis blocker’ı döndürür. | Organization UI |
| `start_speaking_exam_attempt` | `speaking_exam_commands.rs` | Command | Activity/application ownership ve student/class membership doğrulamasıyla capture başlatır. | `SpeechExamPage` |
| Sembol | Dosya | Tür | Ne yapar? | Kim çağırır? | Neyi çağırır? |
|---|---|---|---|---|---|
| `start_student_answer_ocr` | `student_answer_ocr_commands.rs` | Command | Frontend'in OCR başlatma isteğini karşılar. | UI (`commands.ts`) | `StudentAnswerOcrService::start` |
| `rebuild_student_answer_ocr_issues` | `student_answer_ocr_commands.rs` | Command | Mevcut OCR kayıtlarını deterministic analyzer ile yeniden işler. | UI (`commands.ts`) | `StudentAnswerOcrService::rebuild_issues` |
| `start_scoring_job` | `scoring_commands.rs` | Command | Notlandırma job'unu başlatır. | UI (`commands.ts`) | `ScoringService::start` |
| `update_scoring_record` | `scoring_commands.rs` | Command | Öğretmen manuel puan/onay düzeltmesini kaydeder. | UI (`commands.ts`) | `ScoringService::update_scoring_record` |
| `evaluate_workflow` (Eğer varsa) / `get_workflow_snapshot` | `workflow_commands.rs` | Command | UI'ın workflow yenileme isteğini döndürür. | UI (`commands.ts`) | `ProjectStore`, `workflow_engine::evaluate_workflow` |
| `start_model_server` | `model_commands.rs` | Command | llama.cpp sunucusunu portta başlatır. | UI | `ModelProcessManager` |
| `confirm_all_rubrics` | `rubric_commands.rs` | Command | Soru/rubrik backend doğrulamasını çalıştırır, geçerli rubrikleri onaylar ve mevcut sözleşme uyarınca `ExamPackageFreeze` snapshot’ını oluşturur. | Canonical Sınav Paketi workspace’i | `RubricService::confirm_all_rubrics` |

## C. Services (İş Mantığı Katmanı)
| Sembol | Dosya | Tür | Ne yapar? | Kim çağırır? | Neyi çağırır? |
|---|---|---|---|---|---|
| `ProjectStore` | `project_store.rs` | Struct | JSON projesini okur ve diske yazar; legacy schema uyumluluğu için normalize+deserialize yapar ve serde path diagnostics üretir. | Tüm komutlar | OS (File System), `workflow_engine` |
| `WorkflowEngine` | `workflow_engine.rs` | Module/Func | Projenin hangi aşamada olduğunu hesaplar (Deterministic). | `ProjectStore` | - |
| `StudentAnswerCropService` | `student_answer_crop_service.rs` | Struct | Öğrenci cevap crop artifact'larını üretir, crop bbox ve render diagnostics döndürür. | `StudentAnswerOcrService` | `PdfPreviewService`, `ProjectStore` |
| `OcrImagePreprocessService` | `ocr_image_preprocess_service.rs` | Struct | OCR/crop image'larını `original`, `clean_grayscale`, `handwriting_enhanced`, `high_contrast`, `high_contrast_bw` profilleriyle preprocess eder; ayrı cache ve versioned output üretir. | `StudentAnswerOcrService`, `preprocess_ocr_image` | `image`, project cache |
| `StudentAnswerCropTemplate` | `student.rs` / `project.rs` | Struct | Proje içinde kalıcı manuel öğrenci cevap crop şablonunu tutar. | `StudentAnswerCropService` | `ProjectStore` |
| `StudentAnswerOcrService` | `student_answer_ocr_service.rs` | Struct | OCR asenkron görevini yönetir, modeli döndürür. | Commands | `LlamaServerGateway`, `JobManager` |
| `rebuild_issues` | `student_answer_ocr_service.rs` | Func | Persisted OCR kayıtlarını model çağrısı olmadan deterministic issue metadata ile günceller. | `rebuild_student_answer_ocr_issues` | `apply_deterministic_critical_term_analysis` |
| `suggest_ocr_issue_correction_with_model` | `student_answer_ocr_service.rs` | Func | Seçili OCR issue için issue crop + limited context ile strict JSON model check yapar; sonucu persist etmez. | `student_answer_ocr_commands.rs` | `ModelRuntimeService`, `StudentAnswerCropService`, `ModelInputImageService`, `LlamaServerGateway` |
| `apply_deterministic_critical_term_analysis` | `student_answer_ocr_service.rs` | Func | OCR sonrası rubric/expected-answer tabanlı kritik terim belirsizliği üretir ve model metadata'sı yoksa da warning oluşturur. | `StudentAnswerOcrService` | `analyze_critical_term_uncertainty` |
| `analyze_critical_term_uncertainty` | `student_answer_ocr_service.rs` | Func | Kritik terim adaylarını çıkarır, near-match heuristics ile belirsizlik tespit eder. | `apply_deterministic_critical_term_analysis` | `collect_critical_term_candidates` |
| `critical_keyword_ocr_uncertain` | `student_answer_ocr_service.rs`, `llama_server_gateway.rs`, `scoring_service.rs` | String code | OCR kritik terim belirsizliği için canonical warning kodu. | OCR/scoring | Uyarı merge, teacher review |
| `ScoringService` | `scoring_service.rs` | Struct | Öğretmen onaylı OCR ve frozen paket ile notlandırma job'unu yönetir. | Commands | `LlamaServerGateway`, `ModelRuntimeService`, `JobManager`, `ProjectStore` |
| `reconcile_scoring_award` | `scoring.rs` | Func | Model toplam puanını kriter toplamı ile doğrular, gerekirse düzeltir ve teacher-facing reconciliation diagnostics üretir. | `ScoringService` | `ScoringReconciliationDiagnostics` |
| `scoring_active_run_id` | `scoring.rs` | Func | Aktif/latest scoring run id'yi seçer; project state ve history fallback'ini yönetir. | `ScoringService`, `ScoringPage`, `diagnostics.rs` | `latest_scoring_run_id` |
| `scoring_active_records` | `scoring.rs` | Func | Aktif/latest run içindeki scoring kayıtlarını soru bazında dedupe eder. | `ScoringService`, `ScoringPage`, `diagnostics.rs` | `ScoringRecord` |
| `scoring_active_record_count` | `scoring.rs` | Func | Aktif/latest scoring kayıt sayısını döndürür. | `diagnostics.rs`, `workflow_engine.rs` | `ScoringRecord` |
| `scoring_duplicate_result_count` | `scoring.rs` | Func | History içindeki duplicate scoring kayıtlarını sayar. | `diagnostics.rs`, `ScoringPage` | `ScoringRecord` |
| `LlamaServerGateway` | `llama_server_gateway.rs` | Struct | Modeli HTTP üzerinden çağırır (ModelGateway trait implemantasyonu). | Servisler | `reqwest`, Model sunucusu |
| `ModelProcessManager` | `model_process_manager.rs` | Struct | OS seviyesinde llama.cpp process'ini açar/kapatır. | `ModelRuntimeService` | İşletim Sistemi |

## D. Domain Structs & Enums (Veri ve Kurallar Katmanı)
| Sembol | Dosya | Tür | Ne yapar? | Öğrenilecek Konu |
|---|---|---|---|---|
| `Project` | `project.rs` | Struct | Merkezi proje (Aggregrate root) verisi. Her şey buna bağlıdır. | Serde Serialize/Deserialize, `mut` (mutable) referanslar |
| `WorkflowSnapshot` | `workflow.rs` | Struct | Frontend'e giden iş akışı DTO'su (Durum, engeller, butonlar). | - |
| `WorkflowStage` | `workflow.rs` | Enum | Olası tüm proje aşamaları (Örn: `ScoringDone`). | Enum Pattern Matching |
| `BlockingReason` | `workflow.rs` | Enum | Geçişi engelleyen nedenler (Örn: `QuestionTextMissing`). | - |
| `AppError` | `errors.rs` | Struct | Merkezi, yapılandırılmış Hata objesi. | Rust Error Handling, Trait'ler |
| `AppErrorCode` | `errors.rs` | Enum | Uygulamada çıkabilecek her türlü (Örn: `ModelTimeout`) hata kodu. | - |
| `OcrImagePreprocessMode` | `student.rs` / `types.ts` | Enum/Type | OCR preprocess modunu tutar (`original`, `clean_grayscale`, `handwriting_enhanced`, `high_contrast`, `high_contrast_bw`, uyumluluk için `high_contrast_bw_optional`). | - |
| `OcrImagePreprocessDiagnostics` | `student.rs` / `types.ts` | Struct/Type | Preprocess cache, version, boyut, bytes, fallback ve hata notlarını saklar. | - |
| `StudentAnswerOcrRecord` | `student.rs` | Struct | Öğrencinin bir soruya verdiği cevap, preprocess metadata'sı, kritik terim belirsizliği, önerili düzeltme ve model yorumu. | - |
| `ScoringRecord` | `scoring.rs` | Struct | Her öğrenci+soru için kalıcı puanlama kaydı, gerekçe, trace ve öğretmen inceleme durumu. | - |
| `latestScoringRunId` | `project.rs` / `types.ts` | Alan | Projedeki aktif/latest scoring run kimliğini tutar. | - |
| `scoringViewModel` | `src/pages/scoringViewModel.ts` | Module | ScoringPage için active/latest run ve öğrenci özet view-model helper’larını toplar. | ScoringPage, testler |
| `ScoringReconciliationDiagnostics` | `scoring.rs` | Struct | Model toplam puanı, kriter toplamı ve backend düzeltme notlarını saklar. | - |
| `ScoringReconciliationOutcome` | `scoring.rs` | Struct | Reconciliation sonrası düzeltilmiş puan, uyarı ve diagnostics paketini taşır. | - |
| `ScoringCriterionScore` | `scoring.rs` | Struct | Kriter bazlı puan ve gerekçe kaydı. | - |
| `ScoringReviewStatus` | `scoring.rs` | Enum | Notlandırma kaydının öğretmen inceleme durumunu tutar. | - |
| `JobKind` | `job.rs` | Enum | Hangi uzun işin yapıldığını belirtir (Örn: OCR, Render). | - |
| `JobSnapshot` | `job.rs` | Struct | UI'a gönderilen Job (arka plan işi) durumu (Progress). | - |
| `ExamPackageFreeze` | `project.rs` | Struct | QEP kilit verisi. Puanlama (Scoring) için zorunlu. | - |

## E. Diğer Özel Fonksiyonlar
| Sembol | Dosya | Tür | Ne yapar? |
|---|---|---|---|
| `extract_student_answer_ocr` | `llama_server_gateway.rs`| Func (Async) | Modeli OCR promptu ile çağırıp RAW yanıt alır. (Prompt inşası burada). |
| `suggest_student_answer_issue_correction` | `llama_server_gateway.rs` | Func (Async) | OCR issue için vision + limited context strict JSON öneri üretir ve scope taşmasını bloke eder. |
| `StudentIdentityPage` | `StudentIdentityPage.tsx` | Component | Öğrenci kimliklerini listeler ve düzenleme/doğrulama işlemlerini sağlar. |
| `StudentIdentityCropTemplate` | `student.rs` / `types.ts` | Type | Öğrenci kimlik alanı için cevap crop’larından ayrı kalıcı bbox şablonu. |
| `StudentIdentityOcrRecord` | `student.rs` / `types.ts` | Type | Modelin kimlik OCR önerisini, preprocess metadata'sını, raw çıktıyı ve crop diagnostiklerini saklar. |
| `highlightRegion` | `student.rs` / `types.ts` | Field | OCR uncertainty / suggestion / warning item'ı için opsiyonel normalized bbox taşır. | `StudentAnswerOcrIssueReviewPage.tsx`, `PdfPageViewer.tsx` |
| `StudentAnswerOcrIssueCorrectionRequest` | `model.rs` | Struct | OCR issue correction strict JSON check için görsel + sınırlı bağlam isteğini taşır. |
| `StudentAnswerOcrIssueCorrectionOutput` | `model.rs` | Struct | Gemma’nın issue correction önerisini, kararını ve güvenini taşır. |
| `start_student_identity_ocr` | `student_answer_ocr_commands.rs` | Command/Job | Kimlik crop görselleriyle ayrı backend OCR job başlatır. |
| `save_student_identity_crop_template` | `student_answer_ocr_commands.rs` | Command | Kimlik crop template’ini project state’e kaydeder. |
| `extract_student_identity_ocr` | `llama_server_gateway.rs` | Func (Async) | Modeli yalnız kimlik crop görseliyle çağırıp ad/no/sınıf JSON önerisi üretir. |
| `update_student_identity` | `student_scan_service.rs` | Func | Öğrenci ad/no/sınıf bilgilerini günceller ve kimlik eksikliğini kontrol eder. |
| `parse_assistant_content` (belirsiz) | `llama_server_gateway.rs`| Func | Gelen raw model metninden JSON çıkarmayı / onarmayı dener (Salvage). |
| `ensure_ready` | `model_runtime_service.rs` | Func (Async) | Bir job başlamadan önce modelin açık olduğundan emin olur. |
| `save_project` | `project_store.rs` | Func | Mutlak tekil doğruluk kaynağını (Project struct) JSON'a atomik olarak basar. |
| `deserialize_project` | `project_store.rs` | Func | `project.json` için iki aşamalı JSON normalize+deserialize çalıştırır, unknown preprocess mode fallback ve serde path hata detayları üretir. |
| `ScoringPage` | `ScoringPage.tsx` | Component | Notlandırma job'u, kayıt listesi, manuel düzeltme ve raw diagnostics gösterimi. |
| `preprocess_ocr_image` | `student_answer_ocr_commands.rs` | Command | Verilen görüntü için OCR preprocess sonucu ve diagnostics döndürür. | UI (`commands.ts`) | `OcrImagePreprocessService` |
| `getStudentAnswerOcrPreprocessVariantRef` | `studentAnswerOcrUi.ts` | Func | OCR kaydındaki belirli preprocess varyantının preview path'ini çözer. | `StudentAnswerOcrPage.tsx`, testler | `StudentAnswerOcrRecord.preprocessDiagnostics` |
- `CropTemplatePage`: Frontend React page for preparing manual bounding boxes (crop templates).
- `SpeakingExamService`: Speaking capture finalization, segment-preserving Gemma 4 12B cleanup, v4 positive/counter-evidence evaluation, deterministic ceiling reconciliation, repeatability cache and teacher approval persistence.
- `default_speaking_scoring_policy`: `speaking_scoring_policy_v2` frozen mandatory strong descriptors and whole-point level mapping.
- `deterministic_speaking_ceiling`: Transcript/metric evidence gates that preserve the model selection while producing the visible backend-applied level and reason.
- `speaking_evaluation_input_hash`: Transcript, segments, cleanup state, metrics, frozen rubric/policy, prompt, model and runtime repeatability key.
