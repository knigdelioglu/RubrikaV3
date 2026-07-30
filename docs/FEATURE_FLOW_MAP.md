# RubrikaV3 - Feature Flow Map

## Scoring sonrası puanlı kâğıt inceleme

- UI: bağımsız `/graded-exam-review` rotası, `src/pages/GradedExamReviewPage.tsx`, `src/components/scoring/ScoredExamReviewPanel.tsx`; Notlandırma sayfası yalnızca seçili öğrenciye derin bağlantı verir.
- TS API: `getGradedExamReview`
- Tauri command: `graded_exam_review_commands.rs`
- Service: `graded_exam_review_service.rs`
- Persistence: Yok; öğrenci PDF'i ve proje modeli değiştirilmez.
- Akış: öğretmen Kâğıt İnceleme modülünü açar → aktif scoring run'ındaki öğrenciler sınav sayfa sırasıyla kuyruklanır → `get_graded_exam_review` → aktif scoring kayıtları + öğrenci sayfa önizlemeleri + soru crop koordinatları → sayfa üzerinde model puanı rozetleri → önceki/sonraki öğrenci.
- Görüntüleme invariantı: `%100`, kaynak görsel piksel boyutu değil mevcut görüntü alanına sığdırılmış boyuttur. İlk açılışta yatay kaydırma gerektirmeden tüm sayfa görünür; yalnızca kullanıcı `%100` üstüne yakınlaştırırsa kaydırma açılır.
- Puan görünümü: puan işareti öğrenci yazısını örtebilecek bölgelerde saydamdır; kazanılan puan mavi, maksimum puan kırmızı gösterilir. Kriter puanları varsa toplamın yanında toplamsal dağılım korunur. Öğretmen yakınlaştırması öğrenci ve sayfa geçişlerinde korunur.
- Kontrol notları: review gereken soruların teknik nedenleri öğretmen-dostu yönlendirmelere çevrilir ve mümkün olduğunda kâğıdın gri yan boşluğunda gösterilir.
- Kritik invariant: model hatası sıfır puan olarak gösterilmez; güvenilir soru koordinatı yoksa puan rastgele yerleştirilmez ve öğretmene açık uyarı verilir. Workflow, QEP ve scoring davranışları değişmez.

Bu dokümanda projenin temel özelliklerinin uçtan uca akışı, ilgili dosyaları, riskli alanları (invariants) ve hata kodları yer almaktadır.

> **UI Modernizasyonu Notu:** Önce app shell/navigasyon erişimi stabilize edilir; ekran modernizasyonu aşamalı yapılır. AI Studio prototipleri tasarım referansı olarak kullanılır; workflow/backend kararları taşınmaz. Workflow ekranı AI Studio referansından görsel olarak modernize edildi; workflow kararları backend snapshot kaynaklı kalır.

---

## 1. Project open/load/save

### Amaç
Kullanıcının projeyi diske kaydetmesi, varolan projeyi açması ve başlatması.

### Giriş noktaları
- UI: `src/pages/HomePage.tsx`
- TS API: `src/api/commands.ts` (open_project, list_projects)
- Tauri command: `project_commands.rs`
- Service: `project_store.rs`
- Domain: `project.rs` (Project struct)
- Job: Yok
- Persistence: JSON serialization to `project.json`
- Diagnostics: Yok

### Akış
UI → command (open_project) → service (project_store.get_project_snapshot) → domain (Project load) → project_store (deserialization) → workflow/job (evaluate_workflow).

### Kritik invariants
- Atomik kaydetme yapılmalıdır. JSON yazılırken `.tmp` dosyasına yazılıp sonra rename edilmelidir.
- Yanlış veya eksik bir JSON durumunda proje silinmemeli, hata dönülmelidir.
- Geçerli ama legacy/şema-uyumsuz `project.json` dosyaları iki aşamalı normalize+deserialize akışıyla yüklenmeli; serde path bilgisi hata detayında görünmelidir.
- Yeni preprocess/scoring alanları eksikse defaultlarla yükleme sürmeli; unknown preprocess mode varsa tüm proje load fail olmamalıdır.

### Hata kodları
- `ProjectLoadFailed`, `ProjectSaveFailed`, `ProjectNotFound`

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/services/project_store.rs`
2. `src-tauri/src/domain/project.rs`

### Bozulabilecek şeyler
- Geriye dönük JSON uyumluluğu (Eski versiyonlarda kaydedilmiş JSON yeni struct alanları yüzünden yüklenemeyebilir).

---

## 2. PDF upload/import

### Amaç
Sınav PDF’i, cevap anahtarı/rubrik PDF’i ve öğrenci cevap PDF’inin tek Belgeler çalışma alanından sisteme dahil edilmesi ve aynı bağlamda incelenmesi.

### Giriş noktaları
- UI: `src/pages/DocumentsPage.tsx`
- UI presentation: `src/pages/documentWorkspace.ts`
- TS API: `src/api/commands.ts` (`import_exam_source_pdf`, `import_answer_key_pdf`, `import_student_scan_pdf`)
- Tauri command: `document_commands.rs`
- Service: `document_service.rs`
- Domain: `document.rs` (Document, DocumentRole)
- Job: Yok
- Persistence: `project_store.rs` (PDF dosyalarının kopyalanması)
- Diagnostics: Yok

### Akış
Belge rolü seçimi → native PDF seçimi → role-specific import command → `document_service.import` → `Document` oluşturma → `project_store` → query/workflow refresh → yeni belge Belgeler workspace’inde seçili kalır.

Yeni import başarısız olursa mevcut `Document` listesi silinmez veya local olarak boşaltılmaz. Silme ayrı, etki açıklamalı ve onay gerektiren `remove_document` kullanıcı eylemidir.

### Kritik invariants
- Kaynak (exam) PDF'leri öğrenci scan'i olarak kaydedilemez.
- Kopya dosyalar her zaman proje klasörü altındaki `documents/` içine alınır.

### Hata kodları
- `DocumentImportFailed`, `PermissionDenied`

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/services/document_service.rs`
2. `src-tauri/src/domain/document.rs`

### Bozulabilecek şeyler
- Dosya yolu izinleri, büyük PDF dosyalarının kopyalanmasında bellek şişmesi.

---

## 3. PDF preview rendering

### Amaç
Yüklenen üç belge rolünün sayfa görüntülerini arka planda üretmek ve öğretmenin route değiştirmeden aynı Belgeler workspace’inde incelemesini sağlamak.

### Giriş noktaları
- Canonical UI: `src/pages/DocumentsPage.tsx`
- Compatibility UI: `src/pages/PdfPreviewPage.tsx` (yalnız canonical Belgeler route’una redirect)
- Viewer: `src/components/pdf/DocumentPreviewViewer.tsx`, `PdfPageViewer.tsx`, `PageNavigation.tsx`, `ZoomControls.tsx`
- TS API: `src/api/commands.ts` (`start_pdf_preview_render`, `start_student_scan_preview_render`, status ve page-preview query’leri)
- Tauri command: `pdf_commands.rs`
- Service: `pdf_preview_service.rs`, `pdf_service.rs`
- Domain: `document.rs` (PdfPreviewState)
- Job: `job_manager.rs` (Arka planda render)
- Persistence: Önizleme resimlerinin cache klasörüne yazılması
- Diagnostics: Yok

### Akış
Belge workspace’i → rol bazlı preview command → `pdf_preview_service.start` → `JobManager` → sayfa render cache’i → job event → Documents query/status/page-preview refresh → viewer aynı ekranda hazır hâle gelir.

```text
exam_source ─┐
answer_key ──┼→ start_pdf_preview_render
student_scan ┘→ start_student_scan_preview_render

job event → local seçili-belge ilerlemesi + global işlem merkezi
job success → preview status ready → list_pdf_page_previews → embedded viewer
```

Canonical route `/project/:projectId/exam/documents` adresidir. `/documents`, `/pdf-preview` ve `/project/:projectId/exam/preview` compatibility yolları `documentId`, `documentType` ve `page` query bilgisini koruyarak canonical workspace’e ulaşır.

### Kritik invariants
- UI kesinlikle ana thread'de render beklememelidir, bu süreç yavaştır ve Job ile asenkron olmalıdır.
- Hatalı PDF sayfaları atlanabilmeli veya hatası kaydedilebilmelidir.
- Frontend preview readiness üretmez; yalnız `Document.preview`, `PdfPreviewStatusSnapshot` ve `JobSnapshot` verilerini öğretmen diline normalize eder.
- Aynı belge için queued/running veya pending preview varken ikinci preview işi başlatılmaz.

### Hata kodları
- `PdfRenderFailed`

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/services/pdf_preview_service.rs`
2. `src-tauri/src/services/pdf_service.rs`

### Bozulabilecek şeyler
- PDF render motoruna (poppler, pdfium vb.) olan bağımlılık sorunları. Çözünürlüğün (DPI) hatalı ayarlanması.

---

## 4. Question text extraction

### Amaç
Kaynak PDF üzerindeki soru kutularından metinlerin Llama Vision kullanılarak otomatik çıkarılması (veya manuel girilmesi).

### Giriş noktaları
- UI: canonical `src/pages/ExamPackageWorkspacePage.tsx?tab=question`; legacy sayfalar (`QuestionTextReviewPage.tsx` vb.) silinmiştir ve production route doğrudan canonical workspace’e yönlenir.
- TS API: `src/api/commands.ts` (start_question_text_extraction)
- Tauri command: `question_text_commands.rs`
- Service: `question_text_service.rs`, `model_gateway.rs`
- Domain: `question.rs` (TextFieldState), `model.rs` (QuestionTextExtractionRequest)
- Job: `job_manager.rs`
- Persistence: `project_store.rs`
- Diagnostics: Yok

### Akış
UI → command → service (question_text_service.start) → job → service (llama_server_gateway) → domain (TextFieldState oluşturma) → project_store → workflow.

### Kritik invariants
- Çıkarılan metin "Suggested" statüsündedir, öğretmen onaylamadan "Confirmed" (Kesin) hale gelmez.
- Model çalışmıyorsa çıkarım denenemez.

### Hata kodları
- `ModelServerNotRunning`, `ModelTimeout`, `ModelResponseInvalidJson`

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/services/question_text_service.rs`
2. `src-tauri/src/domain/question.rs`

### Bozulabilecek şeyler
- Llama'dan dönen JSON'ın parse edilememesi. Görsel kırpma (crop) koordinatlarının PDF DPI'ı ile eşleşmemesi.

---

## 5. Rubric import / answer key preparation

### Amaç
PDF veya Word/JSON formatındaki bir cevap anahtarından puanlama kriterlerinin (rubric) alınması ve modele uygun formata çevrilmesi.

### Giriş noktaları
- UI: canonical `src/pages/ExamPackageWorkspacePage.tsx?tab=rubric`; legacy sayfalar (`RubricPreparationPage.tsx` vb.) silinmiştir ve production route doğrudan canonical workspace’e yönlenir. Zengin rubrik alanları, kriterler, kısmi puan, sıfır puan koşulları ve yaygın yanlışlar korunur.
- TS API: `src/api/commands.ts` (start_rubric_pdf_import)
- Tauri command: `rubric_commands.rs`
- Service: `rubric_service.rs`, `rubric_extraction_service.rs`
- Domain: `rubric.rs` (RubricState, RubricCriterion)
- Job: `job_manager.rs`
- Persistence: `project_store.rs`
- Diagnostics: Yok

### Akış
UI → command → service (rubric_extraction_service) → job → service (llama_server_gateway) → domain (RubricState) → project_store → workflow.

### Kritik invariants
- Çıkarılan rubrik "Suggested" statüsündedir, otomatik "Confirmed" sayılmaz.
- Yer tutucu (placeholder) veriler gerçek veri gibi kaydedilemez, UI'da placeholder olarak kalır.

### Hata kodları
- `RubricMissing`, `ModelResponseInvalidJson`

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/services/rubric_extraction_service.rs`
2. `src-tauri/src/domain/rubric.rs`

### Bozulabilecek şeyler
- Farklı formattaki (tablo, düz metin) rubriklerin model tarafından yanlış JSON ile dönmesi.

---

## 6. Exam package freeze

### Amaç
Soru, rubrik ve diğer sınav parametrelerinin dondurularak puanlamaya (scoring) hazır, kilitli bir paket (QEP) haline getirilmesi.

### Giriş noktaları
- UI: canonical `src/pages/ExamPackageWorkspacePage.tsx?tab=freeze`; `ExamPackageReviewPage.tsx` silinmiştir ve production route’u tamamen canonical workspace sahiplenir.
- UI presentation: `src/pages/examPackageWorkspace.ts`, `src/utils/examPackageFreeze.ts`
- TS API: `src/api/commands.ts` (`start_exam_package_build`, `confirm_all_rubrics`)
- Tauri command: `exam_package_commands.rs` (build job), `rubric_commands.rs` (`confirm_all_rubrics` ile mevcut freeze sözleşmesi)
- Service: `exam_package_build_service.rs` (build), `rubric_service.rs` (validate/confirm/freeze)
- Domain: `project.rs` (ExamPackageFreeze, ExamPackageFreezeStatus)
- Job: Yok
- Persistence: `project_store.rs`
- Diagnostics: Yok

### Akış
Belgeler → canonical Sınav Paketi workspace → soru metni sekmesi (`edit/confirm_question_text`) → rubrik sekmesi (`update/confirm_question_rubric`) → paket özeti → backend `WorkflowSnapshot.summary.readiness.examPackageFreeze` → confirmation → `confirm_all_rubrics` → `RubricService` validation/confirmation → `ExamPackageFreeze::Frozen` → project_store → workflow/scoring gate.

### Kritik invariants
- Soru veya rubriklerde `Suggested` veya `Missing` bir şey varsa paket asla dondurulamaz.
- Dondurulmuş bir pakette daha sonra (soruda vs.) değişiklik yapılırsa, paket `Invalidated` durumuna düşer.
- Frontend rubrik toplamından veya status sayacından bağımsız freeze readiness üretmez; yalnız backend readiness alanını gate olarak kullanır.
- `questionId` ve `tab=question|rubric|freeze` deep-link’leri refresh/geri-ileri navigasyonunda korunur; legacy route’lar aynı query bağlamını canonical route’a taşır.
- Soru metni çıkarma, rubrik PDF çıkarma ve exam package build uzun işleri global job merkezinde route değişiminden bağımsız kalır.

### Hata kodları
- `QepNotFrozen`, `RubricMissing`, `QuestionTextMissing`

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/services/exam_package_build_service.rs`
2. `src-tauri/src/domain/project.rs`

### Bozulabilecek şeyler
- Paket dondurulduktan sonra projede bir değişiklik olduğunda paketin `Invalidated` olarak güncellenmesinin unutulması.

---

## 7. Student scan intake

### Amaç
Toplu şekilde taranmış öğrenci kağıtlarının sisteme eklenip parçalara bölünmesi.
(Not: UI modernize edildi; gerçek PdfPageViewer ve preview cache akışı korunur)

### Giriş noktaları
- UI: `src/pages/StudentScansPage.tsx`
- TS API: `src/api/commands.ts` (import_student_scan_pdf)
- Tauri command: `student_scan_commands.rs`
- Service: `student_scan_service.rs`
- Domain: `student.rs` (StudentSubmission)
- Job: Yok
- Persistence: `project_store.rs`
- Diagnostics: Yok

### Akış
UI → command → service → domain (StudentSubmission) → project_store → workflow.

### Kritik invariants
- Yüklenen belge asıl sınav (exam source) olarak etiketlenemez.

### Hata kodları
- `DocumentImportFailed`

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/services/student_scan_service.rs`

---

## 8. Student grouping

### Amaç
Yüzlerce sayfalık taranmış dokümanı "her X sayfa 1 öğrenci" kuralıyla mantıksal öğrenci gruplarına ayırmak.

### Giriş noktaları
- UI: `src/pages/StudentGroupingPage.tsx`
- TS API: `src/api/commands.ts` (create_student_page_groups)
- Tauri command: `student_scan_commands.rs`
- Service: `student_scan_service.rs`
- Domain: `student.rs` (PageGroup, PageGroupingMode)
- Job: Yok
- Persistence: `project_store.rs`
- Diagnostics: Yok

### Akış
UI → command → service (create_student_page_groups) → domain (Student, PageGroup oluşturma) → project_store → workflow.

### Kritik invariants
- Gruplama eksik sayfa bırakmamalıdır, tüm öğrenci taranan belgelerindeki (student scans) sayfalar bir öğrenciye (Student) atanmalıdır.

### Hata kodları
- `StudentScanPreviewNotReady`

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/services/student_scan_service.rs`

---

## 9. Student Answer OCR

*(Önceki FEATURE_FLOW_MAP dosyasında anlatılmıştır)*

### Not
- "Crop Şablonu" ayrı bir frontend adımıdır ve `CropTemplatePage.tsx` sayfasından yönetilir.
- Öğrenci cevap crop üretimi ayrı `StudentAnswerCropService` üzerinden yapılır; OCR servisi crop artifact'larını kullanır ve crop yoksa açık review nedeni üretir.
- Manuel `StudentAnswerCropTemplate` varsa OCR per-question crop’u bu şablondan üretir; template yoksa full-page fallback review-required olarak işaretlenir.
- OCR öncesi crop görüntüleri `OcrImagePreprocessService` ile preprocess edilir; orijinal crop korunur, preprocess çıktıları `cache/preprocessed/<preprocessVersion>/*` altında ayrı cache'lenir ve model mümkünse `handwriting_enhanced` crop görür.
- Desteklenen preprocess profilleri `original`, `clean_grayscale`, `handwriting_enhanced`, `high_contrast`, `high_contrast_bw` şeklindedir; model input varsayılanı `handwriting_enhanced` olup UI'da karşılaştırma için diğer varyantlar da saklanır.
- Identity OCR aynı preprocess altyapısını kullanır; preprocess başarısızsa `clean_grayscale`, o da başarısızsa original crop fallback kullanılır ve `preprocess_failed` ile `preprocess_fallback_used` uyarıları saklanır.
- Preprocess metadata'sı `preprocessMode`, `preprocessVersion`, `modelInputCropRef`, `originalCropRefs`, `preprocessedCropRefs`, `availablePreprocessVariants`, `preprocessDiagnostics`, `preprocessWarnings` alanlarını taşır; OCR prompt'u bu bilgileri model request metadata'ya ekler ama modelden görüntü kalitesi yorumu istenmez.
- OCR kaydında `uncertainSpans`, `suggestedCorrections`, `criticalTermWarnings`, `ocrSemanticWarnings` ve `criticalKeywordUncertain` saklanır; `answerText` otomatik değiştirilmez ve bu metadata scoring prompt'una bağlam olarak taşınır.
- `uncertainSpans`, `suggestedCorrections` ve `criticalTermWarnings` içinde opsiyonel `highlightRegion` taşınır; model bbox verirse crop üstünde overlay çizilir, yoksa metin chip/vurgu fallback'i kullanılır.
- OCR post-process aşamasında model metadata'sından bağımsız deterministic critical-term analyzer çalışır; rubric/beklenen cevap kaynaklı adayları tarar ve `critical_keyword_ocr_uncertain` uyarısını model sessiz kalsa da üretir.
- `StudentAnswerOcrIssueReviewPage`, partial-answer şüphesini genel OCR issue olarak genellemez; actionable issue listesi yalnızca somut crop/OCR sinyallerinden oluşur, bbox yoksa crop overlay zorlanmaz ve metin highlight fallback kullanılır.
- `rebuild_student_answer_ocr_issues` komutu model çağırmaz; mevcut OCR kayıtlarını deterministic analyzer'dan geçirir, approval durumundan bağımsız aday üretir ve `criticalKeywordUncertain=true` kayıtlar structured alanlar boş olsa bile somut issue kartına dönüşebilir.
- UI'da "İncelenecekler" filtresi yalnız açık issue'ları gösterir; "Tümü" ve "Çözülenler" filtreleri teacher_approved/onaylı adayları da listeler ve boş open-state resolved issue'ları saklamaz.

---

## 10. OCR Sorun İnceleme

### Amaç
`StudentAnswerOcrPage` üstündeki kayıtlar içinden riskli OCR örneklerini öğretmene toplu ve hızlı gösteren ayrı review ekranı.
Ekran kayıt/etiket listesi gibi davranmaz; somut ifade kartı üretir ve partial-answer genellemesi yapmaz.

### Giriş noktaları
- UI: `src/pages/StudentAnswerOcrIssueReviewPage.tsx`, `src/pages/StudentAnswerOcrPage.tsx`, `src/components/pdf/PdfPageViewer.tsx`
- TS API: `src/api/types.ts`, `src/utils/labels.ts`
- Navigation: `src/app/App.tsx`, `src/app/AppLayout.tsx`, `src/components/workflow/NextActions.tsx`
- Domain/Service: `src-tauri/src/domain/student.rs`, `src-tauri/src/domain/model.rs`, `src-tauri/src/services/llama_server_gateway.rs`, `src-tauri/src/services/student_answer_ocr_service.rs`

### Akış
Project snapshot → issue filter helper → student bazlı list → detail panel → crop overlay / metin vurgusu → save/approve → next issue.

### Kritik invariants
- `needsReview=true`, `criticalKeywordUncertain=true`, `uncertainSpans`, `suggestedCorrections`, `criticalTermWarnings`, preprocess/parse/printed-text uyarıları ve yalnızca gerçekten truncate edilmiş crop sinyalleri tek yerde toplanır; `partial answer` tek başına issue sayılmaz.
- `needsReview=true`, `criticalKeywordUncertain=true`, `uncertainSpans`, `suggestedCorrections`, `criticalTermWarnings` ve parse hataları actionable issue kartı üretebilir; preprocess/printed-text/partial-answer sinyalleri tek başına issue listesine girmez.
- Structured issue alanları boşsa deterministic candidate çıkarımı `answerText` + `expectedAnswer` + quoted semantic warnings üzerinden yapılır; bu yüzden eski kayıtlar yeniden taranınca somut kelime kartına dönüşebilir.
- Öğrenci adı ve numarası görünür; internal submission id kullanıcıya gösterilmez.
- Highlight region yoksa crop overlay zorlanmaz; metin highlight ve chip fallback çalışır.
- Sol listede kayıt değil issue ifadesi görünür; `Bu düzeltmeyi uygula` yalnız ilgili phrase'i değiştirir.

### Bu feature'a dokunurken önce oku
1. `src/pages/studentAnswerOcrUi.ts`
2. `src/pages/StudentAnswerOcrIssueReviewPage.tsx`

---

## 11. Student identity validation

### Amaç
Her öğrencinin sınav kağıdındaki öğrenci no / isim alanının tespiti, kırpılması, OCR yapılması ve onaylanması. (Öğrenci kimliği ayrı UI adımıdır; doğrulanmadan scoring hazır olmaz).

Kimlik alanı Crop Şablonu sayfasında ayrı `identity_header` crop template ile seçilir; kimlik OCR ayrı backend job olarak çalışır; OCR önerisi öğretmen doğrulaması olmadan scoring açmaz.

### Giriş noktaları
- UI: `src/pages/StudentIdentityPage.tsx`
- TS API: `src/api/commands.ts` (`update_student_identity`, `start_student_identity_ocr`, `save_student_identity_crop_template`)
- Tauri command: `student_scan_commands.rs`, `student_answer_ocr_commands.rs`
- Service: `student_scan_service.rs`, `student_answer_crop_service.rs`, `student_answer_ocr_service.rs`
- Domain: `student.rs`
- Job: `student_identity_ocr`
- Persistence: `project_store.rs`
- Diagnostics: Yok

### Akış
Crop UI → `save_student_identity_crop_template` → project_store.
Identity UI → `start_student_identity_ocr` job → identity crop render → ModelGateway → `Student.identityOcr` önerisi.
Teacher edit/verify UI → `update_student_identity` → canonical `Student` alanları → workflow.

### Kritik invariants
- Öğrenci numarası veya ismi olmadan kimlik doğrulanmış sayılmaz.
- OCR önerisi otomatik verified yapmaz; scoring için öğretmen doğrulaması gerekir.

### Hata kodları
- Yok (Genel `ProjectSaveFailed` olabilir)

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/services/student_scan_service.rs`

---

## 12. Model runtime / autostart

### Amaç
Arka planda çalışan llama.cpp model sunucusunun sağlık durumunu izlemek, eğer kapalıysa doğru port ve argümanlarla otomatik başlatmak ve kapanışta süreci (process) temizlemek.

### Giriş noktaları
- UI: `src/pages/ModelStatusPage.tsx`
- TS API: `src/api/commands.ts` (get_model_runtime_status, start_model_server)
- Tauri command: `model_commands.rs`
- Service: `model_runtime_service.rs`, `model_process_manager.rs`
- Domain: `model.rs` (ModelRuntimeStatus, ManagedModelProcess)
- Job: Yok
- Persistence: Process ID'nin memory'de (veya .lock dosyasında) tutulması
- Diagnostics: `ModelRuntimeStatus` dönen diagnostic snapshot

### Akış
UI → command → service (model_runtime_service) → service (model_process_manager) → OS (Process start) → domain (ModelRuntimeStatus).
Model isteyen diğer job'lar: Job → service (model_runtime_service.ensure_ready) → (Eğer kapalıysa) start.
*(Not: Model Durumu UI modernize edildi; autostart ve runtime kararları tamamen backend ModelRuntimeService kaynaklıdır, frontend sadece durumu görselleştirir.)*

### Kritik invariants
- İki farklı llama-server aynı anda aynı portta başlatılmamalıdır (Çakışma engellenmeli).
- Uygulama kapanırken (veya çakılırken) zombi process bırakılmamalıdır.

### Hata kodları
- `ModelServerNotRunning`, `ModelTimeout`

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/services/model_process_manager.rs`
2. `src-tauri/src/services/model_runtime_service.rs`

### Bozulabilecek şeyler
- Çapraz platform OS process kapatma (Windows vs Mac). Port'un başka bir uygulama tarafından işgal edilmesi.

---

## 13. Diagnostics CLI doctor / inspect jobs / inspect model

### Amaç
Sorun anında geliştiricilerin veya kullanıcıların projedeki hataları teşhis etmesi için (Model Raw çıktıları, Job hataları vs.) bir komut satırı aracı veya export aracı sunmak.

### Giriş noktaları
- UI: `src/pages/ModelStatusPage.tsx`
- TS API: Yok (CLI aracı ise terminalden girilir)
- Tauri command: Yok
- Service: Yok
- Domain: `model.rs` (ModelDiagnostics, StudentAnswerOcrParseDiagnostics)
- Job: Yok
- Persistence: JSON dosyalarından doğrudan okuma (`diagnostics.rs` veya `rubrika.rs` binary'si)
- Diagnostics: Kendisi

### Akış
CLI (`rubrika doctor <project-id>`) → JSON dosyalarını oku → stdout'a yaz. Veya UI üzerinden "Export Diagnostics" -> ZIP oluştur.

### Kritik invariants
- Diagnostics verisi öğrenci cevaplarının (PII) ve şifrelerin maskelenmiş halini (varsa) içermeli, ancak raw model JSON çıkışlarını (debug için) bozmadan vermelidir.
- Hata kodları asla lokalize (çevrilmiş) şekilde loglanmamalıdır (Loglarda İngilizce orijinal code olmalıdır).

### Hata kodları
- Yok

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/bin/rubrika.rs`
2. `src-tauri/src/diagnostics.rs`

---

## 14. Workflow readiness

*(Önceki FEATURE_FLOW_MAP dosyasında anlatılmıştır)*

---

## 15. Scoring gate

### Amaç
Tüm hazırlıklar ve OCR bittikten sonra, puanlamanın başlatılabilmesi için gerekli "katı kural kapısı". QEP'in "frozen" olmasını zorunlu kılar.

### Giriş noktaları
- UI: `src/pages/ScoringPage.tsx`, `src/components/workflow/WorkflowPanel.tsx` (Scoring başlama butonu)
- Tauri command: `scoring_commands.rs` (`start_scoring_job`, `update_scoring_record`)
- Service: `scoring_service.rs`, `workflow_engine.rs`, `model_runtime_service.rs`
- Domain: `scoring.rs`, `workflow.rs`, `project.rs`, `model.rs`
- Job: `job.rs` (`JobKind::Scoring`)
- Persistence: `project_store.rs`
- Diagnostics: `diagnostics.rs`, `bin/rubrika.rs`

### Akış
UI → command → scoring_service (gate + job başlatma) → model_runtime_service.ensure_ready → model_gateway → job_manager → project_store (kalıcı sonuç) → workflow_engine / diagnostics.

### Kritik invariants
- **QEP frozen gate must never be weakened** (Mühendislik kuralı 2.9). Kesinlikle UI veya arka planda bu kapı atlatılarak puanlama başlatılamaz.
- Scoring yalnızca `scoring_ready=true` backend gate ile başlatılabilir.
- Onaylı OCR, doğrulanmış öğrenci kimliği ve frozen paket olmadan scoring çalışmaz.
- `needsReview=true` OCR kayıtları scoring girdisine girmez.
- `criterionScores` varsa final `awardedScore` backend post-process ile kriter toplamından doğrulanır ve tutarsızlıkta düzeltilir.
- Rerun sonrası toplam puan yalnızca aktif/latest scoring run üzerinden hesaplanır; eski run sonuçları history/debug olarak kalır ama toplam puana katılmaz.
- Mevcut scoring sonuçları force rerun olmadan overwrite edilmez.
- ScoringPage ilk açılışta öğrenci bazlı özet kartları gösterir; soru detayları sadece ilgili öğrenci açıldığında görünür.

### Hata kodları
- `QepNotFrozen`, `WorkflowBlocked`, `ScoringNotReady`, `ScoringRerunRequired`

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/services/scoring_service.rs`
2. `src-tauri/src/services/workflow_engine.rs`
3. `src-tauri/src/domain/scoring.rs`

---

## 16. Ortak sınav analizi

### Amaç
Yazılı veya konuşma puanları tamamlandıktan sonra ölçüt/soru, öğrenci ve başarı bandı
özetlerini üretmek; aynı toplu veriden Gemma 4 12B ile öğretmen raporu yazdırmak.

### Uygulanan akış
- UI: `src/pages/AnalysisPage.tsx`
- Command: `finish_assessment`, `get_assessment_analysis`, `list_assessment_analyses`
- Service: `src-tauri/src/services/analysis_service.rs`
- Domain: `src-tauri/src/domain/analysis.rs`
- Job: `assessment_analysis`
- Storage: `outputs/analysis/<analysis-id>.json` (atomik)

Grafik ölçümleri deterministiktir ve model raporundan bağımsız kaydedilir. Gemma başarısız
olursa analiz `partial` olur; grafikler korunur. Model promptuna ham cevap, ses veya öğrenci
adı değil yalnızca anonim toplu ölçümler girer. Aynı servis `written` ve `speaking`
değerlendirme türlerini kabul eder.

## 17. OCR issue correction model check

### Amaç
Sorunlu OCR span'ı için öğretmene gösterilecek, otomatik uygulanmayan Gemma önerisi üretmek.

### Giriş noktaları
- UI: `src/pages/StudentAnswerOcrIssueReviewPage.tsx`
- TS API: `src/api/commands.ts` (`suggestOcrIssueCorrectionWithModel`)
- Tauri command: `student_answer_ocr_commands.rs`
- Service: `student_answer_ocr_service.rs`, `student_answer_crop_service.rs`
- Domain: `student.rs`, `model.rs`
- Model gateway: `llama_server_gateway.rs`
- Diagnostics: `logs/model_responses/student_answer_ocr_issue_correction/...`

### Akış
UI → command → service → issue crop seçimi → model runtime ensure_ready → ModelGateway strict JSON issue correction isteği → parsed suggestion DTO → UI sadece sonucu gösterir.

### Kritik invariants
- OCR metni otomatik değişmez.
- Tek kelimelik issue tek kelime öneri sınırını aşamaz.
- Model tüm cevabı yeniden yazamaz ve eksik cevabı tamamlayamaz.
- Gösterilen kararlar öğretmen onayı gerektirir.

### Hata kodları
- `ModelServerNotRunning`
- `ModelResponseInvalidJson`
- `ModelResponseInvalidSchema`
- `PdfRenderFailed`

### Bu feature’a dokunurken önce oku
1. `src-tauri/src/services/student_answer_ocr_service.rs`
2. `src-tauri/src/services/llama_server_gateway.rs`
3. `src/pages/StudentAnswerOcrIssueReviewPage.tsx`

### Konuşma sınavı kurulumu ve çoklu sınıf ataması

Konuşma sınavı çalışma alanı 2 kolonlu öğretmen odaklı kurulum sunar (Sol: Temel Bilgiler, Çoklu Sınıf Seçimi `assignedClassIds`, Görev Metni, Dk/Sn Süre Ayarları, Değerlendirme Özeti, Kompakt Sistem Durumu; Sağ: Canlı Sınav Özeti ve Sınavı Oluştur Aksiyonu). Bir sınav birden fazla sınıfa bağlanabilir (`assigned_class_ids: Vec<String>`). Yürütme ekranında üst sınıf Toolbar'ı sadece bu sınava atanmış sınıfları filtreler. Legacy `class_id` projeleri otomatik olarak `assignedClassIds = [classId]` şeklinde normalize edilir.

### Konuşma sınavı cleanup ve evidence değerlendirmesi

Whisper segmentleri → `SpeakingExamService` → tek Gemma 4 12B text-only runtime → deterministic cleanup gate → `transcript_for_scoring` segment JSON → v4 positive/counter evidence seçimi → evidence ID validation → v2 frozen ceiling/reconciliation → integer puan → öğretmen onayı. Cleanup başarısızlığı, eksik evidence, eksik alt gösterge ve tamamlanmamış reconciliation job’u tamamlanmış göstermez. Aynı evaluation hash + eksiksiz canonical sonuç model çağrısını production cache ile atlar.
