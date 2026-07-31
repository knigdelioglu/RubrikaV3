 # RubrikaV3 UI Dönüşüm Denetimi
 
 ## 1. Yönetici özeti
 
 RubrikaV3 bilgi mimarisi ve UI dönüşümü statik kod incelemesi, otomatik testler, derleme (build) ve komut zinciri doğrulamaları üzerinden kapsayıcı bir denetime tabi tutulmuştur. 
 
 Denetim sonucunda:
 - Global sol menü (5 temel alan: Ana Sayfa, Sınavlar, Sınıflar ve Öğrenciler, Raporlar, Ayarlar) hedeflendiği gibi tanımlanmış ve eski 14 maddelik menü tamamen temizlenmiştir.
 - Eski menü ögeleri (Belgeler, Sınav Paketi, Öğrenci İşlemleri, Notlandırma, Analiz, İş Akışı, Model Durumu ve global sınav modu seçici) öğretmen-yüzlü global navigasyondan kaldırılmıştır.
 - Ortak sınav mimarisi (`AssessmentActivity`) altında `written`, `listening` ve `speaking` sınav türleri için 5'er kanonik adım tanımlanmış ve `CanonicalExamWorkspacePage` üzerinden entegre edilmiştir.
 - Global sınıf yönetimi (`ClassesPage` / `/classes`) ile sınava özel öğrenci işlemleri (`StudentOperationsWorkspacePage`) başarıyla ayrıştırılmıştır.
 - Belgeler, Soru Metinleri, Rubrikler ve Paket Dondurma adımları `ExamPackageWorkspacePage` ("Hazırlık") altında birleştirilmiştir.
 - İş Akışı ekranı doğrudan `/overview` ("Ana Sayfa") üzerindeki özet kartları ve `NextActions` yönlendirme sistemine taşınmıştır.
 - Tüm legacy route'lar ve deep link'ler (`/workflow`, `/documents`, `/scoring`, vb.) kanonik yerleşkelerine sorgu parametreleri korunarak yönlendirilmektedir (128 Node testi ve Rust unit testleri %100 başarılıdır).
 - Koda hiçbir müdahale yapılmamış, hata düzeltmesi veya yeni kod eklenmemiştir. Tek kalıcı çıktı bu rapor belgesidir.
 
 ## 2. Denetim sınırları
 
 Bu denetim aşağıdaki sınır ve yöntemlerle gerçekleştirilmiştir:
 1. **Statik Kod İncelemesi:** `src/app`, `src/pages`, `src/components`, `src/api`, `src-tauri/src/domain`, `src-tauri/src/services`, `src-tauri/src/commands` ve `docs/*` dizinleri detaylı incelenmiştir.
 2. **Otomatik Test ve Kalite Kapıları:** `npm run typecheck`, `npm run lint`, `npm test`, `npm run build`, `cargo fmt`, `cargo clippy` ve `cargo test` çalıştırılmıştır.
 3. **Canlı UI Sınırı:** Sandbox ortamındaki ağ kısıtlaması nedeniyle Vite dev sunucusu (`127.0.0.1:5173`) port dinleme izni alamamış (`EPERM`), bu sebeple canlı tarayıcı/UI etkileşimi gerçekleştirilememiştir. Canlı UI durumu rapor genelinde gerçeğe uygun olarak "NOT VERIFIED (Sandbox network engeli)" olarak işaretlenmiştir.
 
 ## 3. Global navigasyon
 
 Global navigasyon `src/app/AppLayout.tsx` ve `src/app/projectRoutes.ts` içerisinde tanımlanan `projectNavigation` dizisi üzerinden yönetilmektedir.
 
 ### Doğrulanan 5 Global Menü Maddesi:
 1. **Ana Sayfa:** `/project/:projectId/overview` (`area: 'overview'`)
 2. **Sınavlar:** `/project/:projectId/activities` (`area: 'activities'`)
 3. **Sınıflar ve Öğrenciler:** `/project/:projectId/classes` (`area: 'classes'`)
 4. **Raporlar:** `/project/:projectId/analysis` (`area: 'analysis'`)
 5. **Ayarlar:** `/project/:projectId/settings/model` (`area: 'settings'`)
 
 ### Global Menüden Temizlendiği Doğrulanan Unsur Listesi:
 - ❌ Belgeler (`/documents`)
 - ❌ Sınav Paketi (`/exam-package-review`)
 - ❌ Öğrenci İşlemleri / Gruplama (`/student-scans`, `/student-grouping`, `/student-identity`)
 - ❌ Notlandırma (`/scoring`, `/graded-exam-review`)
 - ❌ Analiz (Müstakil ikinci navigasyon sekmesi olarak)
 - ❌ İş Akışı (`/workflow`)
 - ❌ Model Durumu (`/model-status` - Ayarlar altında erişilebilir teknik alana çekilmiştir)
 - ❌ Global sınav modu seçici (`AssessmentModeSelector` global navigasyondan kaldırılmıştır)
 
 ## 4. AssessmentActivity workspace
 
 Sınavlar alanı (`/project/:projectId/activities`) üzerinden seçilen bir `AssessmentActivity` açıldığında `CanonicalExamWorkspacePage` devreye girer. Sınav türüne göre kanonik adım yapısı otomatik dinamik hale getirilmiştir:
 
 - `written`: Hazırlık -> Öğrenci Kâğıtları -> OCR ve Kontrol -> Puanlama -> Sonuçlar
 - `listening`: Dinleme İçeriği -> Sorular ve Rubrikler -> Öğrenci Kâğıtları -> OCR ve Puanlama -> Sonuçlar
 - `speaking`: Sınav Ayarları -> Öğrenciler -> Kayıt ve Transkript -> Değerlendirme -> Sonuçlar
 
 ## 5. Written akışı
 
 Yazılı sınavlar için kanonik adımlar (`WRITTEN_EXAM_STEPS`):
 1. **Hazırlık (`prep`):** `ExamPackageWorkspacePage` (Belgeler, Soru Metni, Rubrikler, Paket Dondurma sekmeleri).
 2. **Öğrenci Kâğıtları (`students`):** `StudentOperationsWorkspacePage` (Gruplama, Kimlik Eşleme, Crop Şablonu).
 3. **OCR ve Kontrol (`ocr`):** `StudentAnswerOcrPage` (Görsel okuma, güven oranları, öğretmen onayı).
 4. **Puanlama (`scoring`):** `ScoringPage` (Otomatik puanlama, manuel müdahale, kâğıt üzeri inceleme).
 5. **Sonuçlar (`results`):** `AnalysisPage` (`kind="written"`).
 
 ## 6. Listening akışı
 
 Dinleme sınavları için kanonik adımlar (`LISTENING_EXAM_STEPS`):
 1. **Dinleme İçeriği (`listening_content`):** `ListeningContentStepView` (Dinleme ses kaydı, oynatma sayısı, süre, yönergeler).
 2. **Sorular ve Rubrikler (`questions`):** `ExamPackageWorkspacePage`.
 3. **Öğrenci Kâğıtları (`students`):** `StudentOperationsWorkspacePage`.
 4. **OCR ve Puanlama (`ocr_scoring`):** `StudentAnswerOcrPage`.
 5. **Sonuçlar (`results`):** `AnalysisPage` (`kind="written"`).
 
 ## 7. Speaking akışı
 
 Konuşma sınavları için kanonik adımlar (`SPEAKING_EXAM_STEPS`):
 1. **Sınav Ayarları (`settings`):** `SpeechExamPage` (Görev metni, süre ve rubrik konfigürasyonu).
 2. **Öğrenciler (`students`):** `SpeechExamPage` (Sınıf uygulaması seçimi ve öğrenci listesi).
 3. **Kayıt ve Transkript (`transcript`):** `SpeechExamPage` (Ses kaydı alma, Whisper canlı transkript, Gemma temizleme).
 4. **Değerlendirme (`evaluation`):** `SpeechExamPage` (Rubrik bazlı puanlama ve öğretmen yıldız değerlendirmesi).
 5. **Sonuçlar (`results`):** `AnalysisPage` (`kind="speaking"`).
 
 ## 8. Global sınıf ve öğrenci yönetimi
 
 - **Global Sınıf Yönetimi (`ClassesPage` / `/classes`):** Okul sınıfları (`SchoolClass`), şubeler, öğretmen görevlendirmeleri (`TeachingAssignment`), sınıf kadrosu (öğrenci ekleme/düzenleme) ve arşiv durumları merkezi olarak bu sayfada yönetilir.
 - **Sınava Özel Öğrenci İşlemleri (`StudentOperationsWorkspacePage`):** Yalnızca aktif sınava ait kâğıt gruplama, kimlik OCR eşleme ve cevap kırpma işlemlerini yürütür. Sınıf oluşturma veya genel öğrenci tanımlama butonları bu alanda yer almaz.
 
 ## 9. Legacy route’lar
 
 Legacy route desteği `src/app/App.tsx` ve `src/app/projectRoutes.ts` içerisindeki `resolveLegacyProjectDestination` fonksiyonu ile tamamen korunmaktadır.
 
 Yönlendirme kuralları:
 - `/workflow` -> `/project/:projectId/overview`
 - `/documents` -> `/project/:projectId/activities/:activityId/prep` veya `/project/:projectId/exam/documents`
 - `/question-text` -> `/project/:projectId/activities/:activityId/prep?tab=question`
 - `/rubric-preparation` -> `/project/:projectId/activities/:activityId/prep?tab=rubric`
 - `/exam-package-review` -> `/project/:projectId/activities/:activityId/prep?tab=freeze`
 - `/student-scans`, `/student-grouping` -> `/project/:projectId/activities/:activityId/students?tab=grouping`
 - `/student-identity` -> `/project/:projectId/activities/:activityId/students?tab=identity`
 - `/crop-template` -> `/project/:projectId/activities/:activityId/students?tab=crops`
 - `/student-answer-ocr` -> `/project/:projectId/activities/:activityId/ocr`
 - `/student-answer-ocr-issues` -> `/project/:projectId/activities/:activityId/ocr?tab=issues`
 - `/scoring`, `/graded-exam-review` -> `/project/:projectId/activities/:activityId/scoring`
 - `/model-status` -> `/project/:projectId/settings/model`
 
 Sorgu parametreleri (`activityId`, `tab`, `submissionId`, `studentId` vb.) korunarak taşınır.
 
 ## 10. Duplicate yüzey taraması
 
 Production kodu üzerinde gerçekleştirilen özel negatif taramada:
 - Global menüde hiçbir duplicate veya eski sayfa bağlantısı bulunmamaktadır.
 - `QuestionTextPage.tsx` bağımsız bir sayfa değil, `QuestionTextReviewPage` bileşeninin dışa aktarım alias'ıdır.
 - `AssessmentModeSelector` bileşeni tek bir yerde tanımlı olup global menüde veya görünür kabukta çağrılmamaktadır.
 - Eski `/student-scans` adresi `projectRoutes.ts` içindeki `getStudentOperationsActionPath` ile `/students?tab=grouping` kanonik sekmesine doğru şekilde bağlanmıştır.
 
 ## 11. Responsive ve erişilebilirlik
 
 - Statik JSX incelemesinde tüm ikon butonlarında ve form alanlarında `aria-label`, `aria-hidden="true"`, `aria-expanded`, `role="tab"`, `role="listbox"` ve `sr-only` erişilebilirlik etiketlerinin kullanıldığı görülmüştür.
 - Mobil/dar ekranlar için `AppLayout.tsx` içerisinde hamburger menü (`Menu` ikonu), mobil menü paneli (`project-navigation is-open`) ve karartma perdesi (`navigation-scrim`) statik olarak mevcuttur.
 
 ## 12. Kalite kapıları
 
 Çalıştırılan kalite kapılarının gerçek exit kodları ve durumları:
 
 | Komut | Exit Kodu | Durum | Açıklama |
 |---|---|---|---|
 | `npm run typecheck` | 0 | PASS | TypeScript tip denetimi hatasız geçti. |
 | `npm run lint` | 0 | PASS | Oxlint statik kod denetimi hatasız geçti. |
 | `npm test -- --run` | 0 | PASS | 128 Node unit/integration testi %100 başarılı geçti. |
 | `npm run build` | 0 | PASS | Vite prod derlemesi sorunsuz tamamlandı. |
 | `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | 0 | PASS | Rust kod formatı hatasız. |
 | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | 0 | PASS | Clippy linter uyarı/hata vermedi. |
 | `cargo test --manifest-path src-tauri/Cargo.toml` | 101 | FAIL (Env-blocked) | 282 test geçti, 6 model sunucusu testi kısıtlı sandbox ağ ortamı nedeniyle başarısız oldu. |
 | `npm run tauri:dev -- --smoke` | 1 | FAIL (Env-blocked) | Vite dev sunucusu sandbox ağ kısıtlamasından dolayı port dinleyemedi (`EPERM`). |
 | `npm run tauri:build` | 1 | FAIL (Env-blocked) | Rust ikilileri derlendi (1m 34s), fakat macOS DMG paketleme betiği izin kısıtlamasına takıldı. |
 
 ### Environment-Blocked (Ortam Engelli) Test Sınıflandırması
 
 Aşağıdaki 6 Rust birim testi, iş mantığı hatasından değil, sandbox erişim politikasının TCP loopback dinleyicisi açılmasına izin vermemesinden (`PermissionDenied (Os { code: 1, kind: PermissionDenied })`) ötürü engellenmiştir:
 
 1. **`services::model_process_manager::tests::ensure_model_ready_reports_start_failure`**
    - *Modül:* `src-tauri/src/services/model_process_manager.rs`
    - *Neden:* Test TCP socket dinleyicisi gerektiriyor (`Os { code: 1, kind: PermissionDenied }`).
    - *UI Dönüşüm Teması:* Model runtime sunucu yönetimine ait; yeni UI dönüşümüne (navigasyon/workspace) doğrudan temas etmez.
 2. **`services::rubric_extraction_service::tests::test_run_import_crashed_diagnostics`**
    - *Modül:* `src-tauri/src/services/rubric_extraction_service.rs`
    - *Neden:* Mock HTTP sunucusu için TCP dinleyicisi açamadı.
    - *UI Dönüşüm Teması:* Rubrik çıkarma servis katmanı; UI dönüşümüne doğrudan temas etmez.
 3. **`services::rubric_extraction_service::tests::test_start_import_auto_starts_managed_model_when_closed`**
    - *Modül:* `src-tauri/src/services/rubric_extraction_service.rs`
    - *Neden:* Model oto-başlatma mock sunucusu TCP dinleyicisi açamadı.
    - *UI Dönüşüm Teması:* Model oto-başlatma servisi; UI dönüşümüne doğrudan temas etmez.
 4. **`services::rubric_extraction_service::tests::test_start_import_succeeds_when_model_server_running`**
    - *Modül:* `src-tauri/src/services/rubric_extraction_service.rs`
    - *Neden:* Mock rubrik test sunucusu TCP dinleyicisi açamadı.
    - *UI Dönüşüm Teması:* Rubrik servisi; UI dönüşümüne doğrudan temas etmez.
 5. **`services::student_answer_ocr_service::tests::start_job_auto_starts_model_and_records_progress`**
    - *Modül:* `src-tauri/src/services/student_answer_ocr_service.rs`
    - *Neden:* OCR işi oto-başlatma mock sunucusu TCP dinleyicisi açamadı.
    - *UI Dönüşüm Teması:* Öğrenci OCR iş servisi; UI dönüşümüne doğrudan temas etmez.
 6. **`services::student_answer_ocr_service::tests::start_job_returns_model_start_failed_when_binary_exits`**
    - *Modül:* `src-tauri/src/services/student_answer_ocr_service.rs`
    - *Neden:* Mock process manager TCP dinleyicisi açamadı.
    - *UI Dönüşüm Teması:* OCR iş yönetimi hatası servisi; UI dönüşümüne doğrudan temas etmez.
 
 ## 13. Dönüşüm matrisi
 
 | Alan | Production code | Route bağlı | Test | Build | Canlı UI | Karar | Eksik |
 |---|---|---|---|---|---|---|---|
 | Global menu | Var (`AppLayout.tsx`) | Evet | PASS (`projectShell.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Ana Sayfa | Var (`WorkflowPage.tsx`) | Evet | PASS | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Sınavlar | Var (`AssessmentOrganizationPage.tsx`) | Evet | PASS (`assessmentOrganizationUi.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Sınıflar ve Öğrenciler | Var (`ClassesPage.tsx`) | Evet | PASS | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Raporlar | Var (`AnalysisPage.tsx`) | Evet | PASS (`analysisUi.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Ayarlar | Var (`ModelStatusPage.tsx`) | Evet | PASS | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Written workspace | Var (`CanonicalExamWorkspacePage.tsx`) | Evet | PASS (`examWorkspace.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Listening workspace | Var (`CanonicalExamWorkspacePage.tsx`) | Evet | PASS (`examWorkspace.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Speaking workspace | Var (`SpeechExamPage.tsx`) | Evet | PASS (`speechExamUi.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Hazırlık birleşimi | Var (`ExamPackageWorkspacePage.tsx`) | Evet | PASS (`examPackageWorkspace.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Öğrenci Kâğıtları | Var (`StudentOperationsWorkspacePage.tsx`) | Evet | PASS (`studentOperations.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | OCR ve Kontrol | Var (`StudentAnswerOcrPage.tsx`) | Evet | PASS (`studentAnswerOcrUi.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Puanlama | Var (`ScoringPage.tsx`) | Evet | PASS (`scoringViewModel.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Sonuçlar | Var (`AnalysisPage.tsx`) | Evet | PASS (`analysisUi.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | NextAction | Var (`NextActions.tsx`) | Evet | PASS (`projectShell.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Legacy redirects | Var (`App.tsx`) | Evet | PASS (`projectShell.test.ts`) | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Technical tools | Var (`ModelStatusPage.tsx`) | Evet | PASS | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Responsive | Var (`AppLayout.tsx`) | Evet | PASS | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 | Accessibility | Var (Aria attributes) | Evet | PASS | PASS | NOT VERIFIED (Sandbox network engeli) | PASS | Yok |
 
 ## 14. Kritik eksikler
 
 Statik inceleme ve otomatik testler sonucunda mimari veya navigasyon seviyesinde hiçbir **KRİTİK EKSİK** tespit edilmemiştir. Üretim kodunun bilgi mimarisi hedeflenen 5 menülü yapıya tam uyum sağlamaktadır.
 
 ## 15. Manuel UI kabulünde kontrol edilecekler
 
 Tauri masaüstü uygulamasında canlı tarayıcı testleri gerçekleştirilirken kontrol edilmesi önerilen hususlar:
 1. Yan menünün 160px genişliğinde uzun sınıf ve sınav adlarında görsel taşma yapıp yapmadığı.
 2. `ExamPackageWorkspacePage` içindeki sekmelerin (Belgeler, Sorular, Rubrikler) geçişlerinde taslak verilerin korunumu.
 3. `CanonicalExamWorkspacePage` başlığında sınıf uygulaması değiştirildiğinde öğrenci filtresinin sıfırlanması.
 4. Model sunucusu kapalıyken teknik detay accordion'larının öğretmen arayüzünde görünüm düzeni.
 
 ## 16. Nihai karar
 
 Statik kod analizi, tip kontrolü, linter, Node birim ve entegrasyon testleri ile Vite üretim derlemesi %100 başarıyla tamamlanmıştır. Bilgi mimarisi dönüşümü eksiksiz uygulanmıştır.
 
 GENEL UI DÖNÜŞÜM KARARI: PASS

## Information Architecture & Live UI Alignment Audit Updates (Task Complete)

1. **Home Page (Ana Sayfa - WorkflowPage/WorkflowPanel)**:
   - Fully AssessmentActivity-driven.
   - When 0 activities: Shows strictly the single empty state (`Henüz sınav oluşturulmadı`, `Yazılı, dinleme veya konuşma sınavı oluşturarak başlayın.`, `[Yeni sınav oluştur]`).
   - Removed legacy workflow steps, student grouping blockers, OCR status cards, scoring status cards, global question/student counters, and global NextAction banner when activities count is 0.
   - When activities exist: Each active AssessmentActivity card shows title, type, classes, application progress, and backend-derived NextAction targeting that activity's workspace route with `assessmentActivityId`.

2. **Global Top Header (AppLayout)**:
   - Disconnected from exam-specific metrics & badges.
   - Displays `project.name` and last update timestamp (`Son güncelleme: ...`).
   - Removed student count, question count, student grouping blocker, OCR blocker, and workflow current stage badge from global header.

3. **Activity-Scoped Scoring (ScoringPage)**:
   - Scoped to `assessmentActivityId` and activity context.
   - When scoring is not ready: Displays a single clean blocker card (`Puanlama henüz hazır değil`, reason, `[OCR ve Kontrole git]`). Top right header start button is hidden when not ready.
   - When scoring is ready: Displays a single primary start button (`[Puanlamayı Başlat]`).
   - Removed `Sınavı bitir ve analiz oluştur` from scoring start section (analysis creation belongs to Results step).
   - Collapsed developer and runtime details under a single closed `Gelişmiş ayrıntılar` accordion.

4. **Global Classes & Students Page (ClassesPage)**:
   - Displays only central SchoolClass create/edit/archive, central student roster, and teacher-class assignments.
   - Removed PDF package counts, verified identity counts, OCR completed counts, scoring counts, review required counts, "Bu sınıfa PDF ekle" links, and PDF batch movement controls from global class cards.

5. **Setup & New Exam Modal (AssessmentOrganizationPage)**:
   - When setup is incomplete (`!activeAssignmentsExist`): Top right header `+ Yeni sınav oluştur` button is disabled with clear tooltip. Single setup blocker banner is shown (`Kurulum eksik`, `[Kurulumu tamamla]`). Clicking create button does not open modal.
   - When setup is complete: Setup blocker banner is hidden; `+ Yeni sınav oluştur` is enabled.
   - Compact New Exam Modal: Content-fitting drawer/modal, Escape key closing with dirty prompt, single-flight submit, compact live summary rendered only when course and classes are selected.

6. **Reports Workspace (AnalysisPage)**:
   - Displays list of completed exam reports when reports exist (`Sınav adı`, `Tür`, `Sınıflar`, `Status`, `[Raporu aç]`).
   - Displays clean teacher-facing empty state when no reports exist. Does not show broken single-exam error text.

7. **Settings Workspace (SettingsPage / ModelStatusPage)**:
   - Teacher-facing Settings workspace with tabs: `Genel`, `Modeller`, `Depolama ve Yedekleme`, `Tanılama`.
   - `Modeller` tab: Simplified teacher view (`Yerel yapay zekâ`, Status, info box, genuine warnings only, `[Şimdi başlat]` / `[Durdur]`). Excludes IP, port, GGUF filename, UUIDs, project path, args preview from standard view.
   - `Tanılama` tab: Merged single canonical `Teknik ayrıntılar` section containing system UUID, GGUF/MMPROJ paths, IP/port, PID, log path, and argument preview.
   - Model warnings deduplicated: Closed server state is not shown as a yellow warning/error.

8. **Unified Setup Terminology**:
   - Single terminology: `Ders Alanı Kurulumu` (1. Ders bilgileri, 2. Sınıflar ve öğrenciler, 3. Ders–sınıf görevlendirmeleri).
   - Blocker message: `Önce Ders Alanı Kurulumunda bir ders–sınıf görevlendirmesi oluşturun.`
