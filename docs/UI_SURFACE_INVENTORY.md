# RubrikaV3 UI Surface Inventory

## 1. İnceleme yöntemi ve sınırlar

Bu rapor yalnızca mevcut production UI kodunun incelenmesiyle hazırlanmıştır. `src/design-reference/ai-studio/**` mock/tasarım dosyaları production sayımına dahil edilmemiştir. İncelenen ana kaynaklar:

- `src/app/App.tsx`, `src/app/AppLayout.tsx`
- `src/pages/**`
- `src/components/**`
- `src/api/commands.ts`, `src/api/types.ts`, `src/utils/labels.ts`
- `src-tauri/src/commands/**`, `src-tauri/src/services/**`, `src-tauri/src/domain/**`
- `src-tauri/src/lib.rs`, `src-tauri/src/bin/rubrika.rs`
- `package.json`, `vite.config.ts`, `src-tauri/tauri.conf.json`
- `docs/PROJECT_MAP.md`, `docs/FEATURE_FLOW_MAP.md`, `docs/FILE_OWNERSHIP_MAP.md`, `docs/SYMBOL_MAP.md`, `docs/API_CONTRACTS.md`

`npm run tauri:dev` çalıştırıldı. Vite `http://127.0.0.1:5173/` üzerinde açıldı ve Rust binary başlatıldı. İn-app browser ile `/` ve `/project-create` gerçek DOM üzerinden görüldü. Tarayıcıdaki saf Vite sekmesi Tauri `invoke` köprüsüne bağlı olmadığı için `/Users/kadir/Documents/RubrikaV3/Projects/11_46` projesi canlı olarak açılamadı; proje-bağlamı isteyen sayfaların boş bağlam ekranları gözlemlendi. Bu nedenle proje verisine bağlı loading/success/job sonuçları statik render akışı ve command sözleşmeleriyle doğrulandı; raporda bunlar “statik incelemede doğrulanamadı” olarak işaretlenmiştir.

## 2. Global uygulama kabuğu

### İlk açılış

`/` ve `/projects` aynı `HomePage` bileşenini gösterir. Başlık `Projeler`, alt metin `Sınav değerlendirme projelerinizi yönetin.` ve sağ üstte `➕ Yeni Proje` düğmesi vardır. Proje listesi TanStack Query ile `commands.listProjects` üzerinden alınır. Alt bölümde Tauri/Rust hazır olma durumu ve uygulama sürümü gösterilir.

Canlı saf-Vite gözleminde backend köprüsü bulunamadığı için iki `Bilinmeyen bir hata oluştu.` banner’ı ve `Sistem Durumu: Tauri Bekliyor, Rust Backend Bekliyor(Bilinmiyor)` metni göründü. Bu, production Tauri webview davranışı olarak değil, canlı inceleme ortamının invoke köprüsüz durumu olarak değerlendirilmelidir.

### Sol menü

`AppLayout` yalnızca `/`, `/projects` ve `/project-create` dışındaki route’larda görünür. Sabit genişliği `160px` olan sol aside içinde `RubrikaV3` logosu ve proje açılmadığında `Menüyü görmek için bir proje açın.` mesajı bulunur. `projectId` varsa aşağıdaki maddeler sıra ile görünür:

| Görünen metin | Route | Erişim koşulu | Davranış |
|---|---|---|---|
| İş Akışı | `/workflow` | `projectId` | `projectId` query parametresiyle navigasyon |
| Belgeler | `/documents` | `projectId` | Belge yönetimi |
| PDF Önizlemeler | `/pdf-preview` | `projectId` | Sınav/öğrenci PDF görüntüleme |
| Soru Metni Kontrolü | `/question-text` | `projectId` | `/question-text` aynı zamanda `QuestionTextReviewPage` alias’ıdır |
| Rubrik Hazırlama | `/rubric-preparation` | `projectId` | Rubrik hazırlama |
| Sınav Paketi | `/exam-package-review` | `projectId` | Paket doğrulama/dondurma |
| Öğrenci Gruplama | `/student-grouping` | `projectId` | Sayfa gruplama |
| Öğrenci Kimliği | `/student-identity` | `projectId` | Kimlik OCR/doğrulama |
| Crop Şablonu | `/crop-template` | `projectId` | Cevap/kimlik crop şablonu |
| Öğrenci Cevap OCR | `/student-answer-ocr` | `projectId` | OCR çalıştırma ve onay |
| OCR Sorun İnceleme | `/student-answer-ocr-issues` | `projectId` | Issue filtreleme/düzeltme |
| Notlandırma | `/scoring` | `projectId` | Scoring job ve sonuçlar |
| Kâğıt İnceleme | `/graded-exam-review` | `projectId` | Sayfa üstünde puanlı kâğıt inceleme |
| Model Durumu | `/model-status` | `projectId` gerekmez | Model runtime/diagnostics |

Aktif madde yalnızca `location.pathname === item.path` ile renklenir. Badge/sayaç, menü disabled durumu, responsive collapse, geri/ileri menüsü ve global toast sistemi yoktur. Global hata görünümü sayfa içindeki `ErrorBanner` bileşenleriyle sağlanır; global job/progress bar kabukta bulunmaz.

Kaynak: `src/app/App.tsx`, `src/app/AppLayout.tsx`, `src/components/common/ErrorBanner.tsx`.

## 3. Route ve sayfa envanteri

18 route tanımı vardır; `/` ve `/projects` aynı sayfayı, `/question-text` ise `QuestionTextReviewPage` alias’ını kullanır. Benzersiz production sayfa component’i 16’dır.

### Projeler — `HomePage`

- Route: `/`, `/projects`
- Teknik konum: `src/pages/HomePage.tsx`
- Backend: `get_app_status`, `list_projects`, proje kartına tıklanınca `open_project`; başarıda `setActiveProject` ve `/workflow?projectId=...` navigasyonu.
- Görünüm: başlık, açıklama, yeni proje düğmesi, proje kartları, belge/soru/rubrik kapsama özetleri, son açılan etiketi, atlanan projeler accordion’u, warning paneli, sistem durumu footer’ı.
- Loading: `Projeler yükleniyor...`. Empty: `Henüz bir proje oluşturulmamış...`. Hata: `ErrorBanner`; açma işlemi pending iken kart opaklığı azalır.
- Kart tıklanabilir ama semantik button/link değildir. Silme/arşivleme yoktur. `project.path`, tarih ve atlanan proje `technicalDetails` alanı teacher-facing ekranda görünür.
- Ulaşılamayan durum: canlı Vite incelemesinde backend olmadığı için gerçek project card, skipped project ve warning verisi görülemedi.

### Yeni Proje Oluştur — `ProjectCreatePage`

- Route: `/project-create`; kaynak `src/pages/ProjectCreatePage.tsx`.
- Alanlar: `Proje Adı` text input; eğitim yılı, ders kodu ve ders adı text input’ları; `Klasör Yolu` text input; `📁` klasör seçici. Eğitim yılı 1 Temmuz kuralıyla otomatik gelir; ders kodu `tde`, ders adı `Türk Dili ve Edebiyatı` olarak başlar ve üçü de düzenlenebilir.
- `Proje adı zorunludur.` uyarısı gösterilir; ad boşsa `Proje Oluştur` disabled olur. Klasör yolu `getDefaultProjectPath(name, academicYearId)` ile alınır ve varsayılan klasör adına eğitim yılı eklenir.
- `Geri Dön` `/projects`’e gider. `Proje Oluştur` → `create_project`; başarıda active project set edilir ve workflow’a gider. Pending state `LoadingButton` ile gösterilir, hata `ErrorBanner`’dir.
- Confirmation yoktur. Backend persistence doğrudan project store’dur; job yoktur.

### Belgeler — `DocumentsPage`

- Route: `/documents`; kaynak `src/pages/DocumentsPage.tsx`.
- Üç belge kartı: sınav PDF’i, cevap anahtarı/rubrik PDF’i, öğrenci cevap PDF’i. Her kartta belge adı, sayfa sayısı, preview durumu, `Sil`, `Önizlemeyi Oluştur`/önizleme linki ve `PDF Yükle` alanı bulunur.
- Dosya seçimi Tauri dialog ile yapılır. Import role göre `importExamSourcePdf`, `importAnswerKeyPdf`, `importStudentScanPdf` çağırır. Preview render role göre `startPdfPreviewRender` veya `startStudentScanPreviewRender` başlatır; sonuç query invalidate eder.
- Empty upload kartında `PDF Yükle` ve `Sürükle bırak veya seç` metni vardır. Pending `Yükleniyor...`; bilgi paneli `Belge Yükleme Tamamlandı` metnini gösterir.
- `Sil` mutation pending iken disabled’dır. Confirmation yoktur; bu, geri dönüşsüz UI etkisi için yüksek risklidir.
- Kaynak: `DocumentsPage.tsx`, `document_commands.rs`, `document_service.rs`, `project_store.rs`.

### İş Akışı — `WorkflowPage` / `WorkflowPanel`

- Route: `/workflow`; kaynak `src/pages/WorkflowPage.tsx`, `src/components/workflow/WorkflowPanel.tsx`, `NextActions.tsx`, `BlockingReasons.tsx`.
- Üstte aktif proje adı ve path, `İş Akışı Durumu`, stage etiketi, adım kartları, durum ikonları, `Puanlama Hazırlığı` checklist’i, `Genel Durum`, `Sistem İpucu`, `Engeller` ve `Geliştirici Detayları` accordion’u bulunur.
- Adım kartları backend `WorkflowSnapshot` içindeki step code/label/message/state’ten türetilir. `NextActions` action code’a göre gerçek command veya route çalıştırır; success/error mesajlarını panel içinde gösterir.
- Loading: `Yükleniyor...`; partial/running/success/failed ikonları vardır. Blocking reason label’ları `labels.ts` mapper’ından geçer.
- Teknik path ve JSON workflow snapshot developer accordion’unda görünür.
- Risk: `WorkflowPanel` bazı checklist özetlerini UI tarafında project alanlarından hesaplar; backend snapshot ile aynı sonucu verip vermediği statik incelemede her veri varyantı için doğrulanamadı.

### PDF Önizlemeler — `PdfPreviewPage`

- Route: `/pdf-preview`; kaynak `src/pages/PdfPreviewPage.tsx`, `PdfPageViewer.tsx`, `PageNavigation.tsx`, `ZoomControls.tsx`.
- Hub görünümünde sınav ve öğrenci PDF kartları, sayfa sayısı, hazır preview sayısı, durum, `Önizlemeyi Oluştur`, `Önizlemeyi Aç`, `İş Akışı`, `Belgeler` bağlantıları bulunur.
- Viewer’da sayfa geri/ileri, zoom in/out, fit/100% görünüm, sayfa görüntüsü ve loading/empty/error overlay’leri vardır. Crop/highlight seçimi production viewer’da yok; crop şablonu ayrı sayfadadır.
- Durumlar `Yüklenmedi`, `Kuyrukta`, `Oluşturuluyor`, `Hazır`, `Başarısız`; Poppler yoksa `PDF önizleme aracı bulunamadı.` uyarısı gösterilir.
- Backend: render başlatma job, status/list/get preview read command’ları. İş sonucu query refetch ile gösterilir.

### Soru Metni Kontrolü — `QuestionTextReviewPage`

- Route: `/question-text`; kaynak `src/pages/QuestionTextReviewPage.tsx`; `QuestionTextPage.tsx` yalnızca export alias’ıdır.
- Başlık, açıklama, stage/status summary, toplam/onaylı/öneri/eksik sayaçları, preview oluşturma, metin çıkarma, vision fallback, `Önerileri Onayla` kontrolleri ve soru tablosu bulunur.
- Tablo kolonları: `No`, `Soru Metni`, `Durum`, `İşlem`. Satırda text input/textarea benzeri düzenleme, `Kaydet`, `Onayla`; eksik metinde `Bu soru için metin eksik. Kontrol edilmeli.`.
- `Soru Metnini Çıkar` ve fallback job başlatır; `confirmQuestionText`, `confirmAllQuestionTexts`, `editQuestionText` persistence mutation’ıdır. Model önerisi suggested kalır, onayla confirmed olur.
- Loading/running: spinner ve `Soru metni çıkarılıyor`; empty: `Henüz Soru Bulunmuyor` / `PDF'i tarayarak soruları çıkarın.`; job warning ve extraction error banner’ı vardır.

### Rubrik Hazırlama — `RubricPreparationPage`

- Route: `/rubric-preparation`; kaynak `RubricPreparationPage.tsx`, `RubricEditor.tsx`, `RubricQuestionCard.tsx`, `RubricImportSummary.tsx`.
- `Cevap anahtarı PDF'inden çıkarılan bilgileri kontrol edin veya manuel düzenleyin.` açıklaması, toplam `Toplam Puan: x / 100`, durum sayaçları, JSON import, PDF extraction, validation, tüm rubrikleri onaylama, soru kartları ve import summary görünür.
- Soru kartı alanları: maksimum puan, beklenen cevap, kriterler, kısmi puan notları, sıfır puan koşulları, yaygın yanlışlar. Kriter ekleme/silme ve puan alanları component içinde bulunur.
- Backend: `importRubricJson`, `startRubricPdfImport` job, `updateQuestionRubric`, `validateRubrics`, `confirmQuestionRubric`, `confirmAllRubrics`; model kapalıysa `Gemma model sunucusu çalışmıyor.` paneli ve model durumuna geçiş gösterilir.
- Empty/missing/error: `Rubrik eksik`, `Otomatik Doldurulan`, validation blocker listesi. Placeholder ve toplam puan uyuşmazlığı teacher-facing warning’e map edilir.

### Sınav Paketi İnceleme — `ExamPackageReviewPage`

- Route: `/exam-package-review`; kaynak `src/pages/ExamPackageReviewPage.tsx`.
- Soru/rubrik eşleşme kartları, `Soru Metni Onaylı`, `Rubrik Onaylı`, `Toplam: x Puan` rozetleri, warning/blocker listesi ve paket dondurma düğmesi vardır.
- Düğme durumları: `Paketi Dondur (Freeze)`, pending `Donduruluyor...`, frozen `Paket Donduruldu`; `canConfirmAll` ve frozen state ile disabled olur, title içinde blocker açıklaması gösterilir.
- `validateRubrics`, `confirmAllRubrics` ve job list query kullanılır. Kaynakta paket build/freeze command wrapper’ları olsa da bu sayfa freeze için `confirmAllRubrics` mutation’ını kullanır; gerçek freeze davranışının backend tarafındaki service karşılığı ile UI adı arasında ayrıca doğrulama gerekir.
- QEP/frozen gate scoring backend’de korunur; UI shortcut tespit edilmedi.

### Öğrenci Gruplama — `StudentGroupingPage`

- Route: `/student-grouping`; kaynak `StudentGroupingPage.tsx`, `StudentSubmissionList.tsx`, `PageGroupEditor.tsx`.
- `Bir öğrencinin sınavı kaç sayfa?` number input (`min=1`, `max=20`), `Grupla`, `Grupları Onayla`, submissions listesi, öğrenci sayısı, page range, PDF sayfalarını inceleme linki ve kimlik/OCR durumları görünür.
- `Grupla` → `createStudentPageGroups`; `Grupları Onayla` → `markStudentGroupingComplete`; sayfa düzenleme child component’inde `updateSubmissionPages`; `Sayfaları İncele` PDF preview’e gider.
- Blocker: PDF yok, preview tamamlanmamış veya grouping hazır değil. Empty: `Henüz öğrenci grubu oluşturulmadı...`. Pending/status message’lar panelde görünür.
- Teacher-facing sızıntı: her submission kartında `• ID: {sub.id.split('-')[0]}` görünür. Bu UUID/internal ID’dir ve kaldırılmalı ya da diagnostik alana taşınmalıdır.
- Kodda `StudentSubmissionList` ve `PageGroupEditor` reusable parçaları vardır; ana sayfada doğrudan kullanılan görünür akışın bir kısmı inline’dır.

### Öğrenci Kimlik Doğrulama — `StudentIdentityPage`

- Route: `/student-identity`; kaynak `StudentIdentityPage.tsx`, `StudentIdentityEditor.tsx`.
- `Kimlik OCR’ını Başlat` düğmesi, `Ad Soyad`, `Okul No`, `Sınıf` kolonları, durum (`Doğrulandı`, `Eksik / kontrol gerekli`), `OCR detayı` accordion’u ve satır bazlı `Doğrula`/`Güncelle` görünür.
- Kimlik crop template yoksa OCR düğmesi disabled ve `Önce Crop Şablonu sayfasında kimlik alanını seçin.` görünür. En az ad veya numara girilmeden doğrulama disabled’dır.
- Backend: `startStudentIdentityOcr` job; `updateStudentIdentity` persistence. OCR raw object `JSON.stringify(student.identityOcr, null, 2)` ile öğretmen ekranında açılabilir.
- Empty/all verified durumları: `Eksik kimlikler var`, `Tüm kimlikler doğrulandı!`, `Notlandırmadan önce öğrenci kimlikleri doğrulanmalı.`.

### Crop Şablonu — `CropTemplatePage`

- Route: `/crop-template`; kaynak `CropTemplatePage.tsx`, `PdfPageViewer.tsx`, `PageNavigation.tsx`.
- `Cevap crop’ları` ve `Kimlik crop’u` özetleri, `Cevap alanları`/`Kimlik alanı` sekmeleri, soru seçim düğmeleri, preprocess seçenekleri (`Orijinal`, `Temiz gri ton`, `El yazısı güçlendirildi`, `Yüksek kontrast`, `Siyah-beyaz alternatif`), sayfa navigation, crop overlay ve kaydet/sıfırla kontrolleri bulunur.
- `Cevap Crop’larını Kaydet` → `saveStudentAnswerCropTemplate`; `Kimlik Crop’unu Kaydet` → `saveStudentIdentityCropTemplate`; preprocess preview → `preprocessOcrImage`.
- Empty: `Öğrenci 1 sayfa önizlemesi hazır değil.`; kimlik draft yoksa kaydet disabled ve durum `eksik`.
- Koordinatlar local draft’ta tutulur, kaydetme ile project store’a yazılır. Preview’nin gerçek crop görseliyle doğrulanması aktif öğrenci projesi olmadan statik incelemede doğrulanamadı.

### Öğrenci Cevap OCR — `StudentAnswerOcrPage`

- Route: `/student-answer-ocr`; kaynak `StudentAnswerOcrPage.tsx`.
- Başlık/açıklama, `OCR Başlat`, `Yeniden Çalıştır`, Kimlik doğrulama linki, workflow durumu, crop template özeti, toplam/onaylı/bekleyen sayaçları, öğrenci kartları, kayıt detayları ve progress paneli bulunur.
- `OCR Başlat`/yeniden çalıştır → `startStudentAnswerOcr`; kayıt bazında metin düzenleme → `updateStudentAnswerOcrText`; `Onayla` → `markStudentAnswerOcrReviewed`; toplu onay → `markAllStudentAnswerOcrReviewed`.
- Running: `Yeni OCR Çalışıyor`, `Lütfen bekleyin, model yanıtları üretiyor...`, `İlerleme: current / total`. Record status’ları `Bekliyor`, `Çalışıyor`, `Üretildi`, `Kısmi`, `Başarısız`, `Kontrol gerekli`, `Düzeltilmiş`, `Onaylandı` gibi map edilir.
- Her OCR kartında original/preprocessed crop preview, OCR metni, suggested correction, warning’ler ve `Teknik Detaylar & Ham Model Çıktısı (Debug)` accordion’u vardır. Raw model output, crop ref sayıları, preprocess version/mode ve render diagnostics görünür.
- Scoring’e geçiş için teacher-approved OCR gate backend’e bağlıdır; model hatası zero score olarak gösterilmez.

### OCR Sorun İnceleme — `StudentAnswerOcrIssueReviewPage`

- Route: `/student-answer-ocr-issues`; kaynak `StudentAnswerOcrIssueReviewPage.tsx`.
- Sol liste filtreleri: `Açık`, `Çözülen`, `Tümü`; issue cards; seçili öğrenci/numara/sınıf/soru başlığı; crop görüntüsü, bbox/text highlight, OCR text, önerili düzeltme ve tanılama panelleri bulunur.
- Aksiyonlar: `Gemma ile öneriyi kontrol et` → `suggestOcrIssueCorrectionWithModel`; `Düzeltmeyi Uygula` → `updateStudentAnswerOcrText`; `Bu OCR doğru` → `markStudentAnswerOcrReviewed`; `Sonraki issue`; `Sorunları Yeniden Tara` → `rebuildStudentAnswerOcrIssues`.
- Empty states: `Detay görmek için soldan bir kayıt seçin.`, OCR metni yok, issue listesi boş/filtreye göre boş durumlar. BBox yoksa `Görsel konumu bulunamadı; metin vurgusu kullanılıyor.`.
- Teacher-facing açıklamalar çoğunlukla mapper’dan gelir; ancak açılabilir teknik panelde `Görsel`, `Prompt`, `Ham çıktı`, scope, confidence ve model raw output görünür.
- Kaynakta `internal id gösterilmez` hedeflenmiş olsa da bu davranış yalnızca statik kod niyetiyle doğrulandı.

### Model Durumu — `ModelStatusPage`

- Route: `/model-status`; kaynak `ModelStatusPage.tsx`.
- `Sunucu Durumu`, adres/port, health (`OK`/`Yanıt Yok`), yönetim modu, aktif profil, binary/model/mmproj yolları, başlat/durdur/mod değiştir, argüman önizleme, profil sıfırlama ve log/diagnostics görünür.
- Aksiyonlar `probeModelServer`, `startModelServer`, `stopModelServer`, `setModelMode`, `previewModelServerArgs`, `resetModelProfile`; pending spinner’ları vardır.
- Model kapalı: `Model sunucusu şu an kapalı...`; hazır: `Model hazır. OCR için kullanılabilir.`. Hata panelinde önerilen action ve teknik detaylar görünür.
- Debug panelinde PID, completion probe, log path, technicalDetails ve raw status bilgisi teacher-facing sayfada açılabilir. Bu ayrım kullanıcıya açık olduğu için teknik veri sızıntısı riski yüksektir.

### Notlandırma — `ScoringPage`

- Route: `/scoring`; kaynak `ScoringPage.tsx`, `scoringViewModel.ts`.
- Scoring gate/preparation kartı, sonuç özeti, model durumu, blocker paneli, running progress, öğrenci bazlı accordion, toplam/maksimum veya `Geçici toplam / Maksimum`, soru kayıtları, kriter puanları, gerekçeler, review reasons, uyarılar ve manual score inputları bulunur.
- `Notlandırmayı Başlat` → `startScoringJob`; `Yeniden çalıştır` force rerun; manuel puan → `updateScoringRecord`; `Kâğıt İnceleme` linki graded review sayfasına gider.
- Running: `Notlandırma işlemi çalışıyor`; blocker: `Notlandırma engelleri`; model sonucu invalid ise `Bu kayıt öğrenci toplamına sıfır olarak eklenmedi... Manuel puan girerek kaydı tamamlayın.`.
- `Teknik tanı ayrıntıları` ve `Geliştirici özeti` accordion’larında raw model diagnostics, run IDs/history ve reconciliation ayrıntıları görülebilir. Active/latest run view-model ile duplicate history ayrıştırılır.
- Review/approval persistence ve stale scoring backend contract’a bağlıdır; canlı proje olmadan `active/latest run` kombinasyonları doğrulanamadı.

### Kâğıt Üzerinde İnceleme — `GradedExamReviewPage`

- Route: `/graded-exam-review`; kaynak `GradedExamReviewPage.tsx`, `ScoredExamReviewPanel.tsx`.
- Üst toolbar’da `Önceki öğrenci`, `Sonraki öğrenci`, sayfa navigation, zoom ve puan anotasyonları; yan panelde soru bazlı puanlar, max puan ve review uyarıları vardır.
- Loading: `Puanlı sınav kâğıdı hazırlanıyor…`; empty: `İncelenecek kâğıt henüz yok`. Model hatası sıfır olarak yerleştirilmez; review uyarıları gri yan alanda gösterilir.
- Persistence yok; `getGradedExamReview` yalnızca read model döndürür. Sayfa ve submission query parametreleriyle deep-link edilir.

## 4. Bütün buton ve aksiyonların toplu envanteri

Aşağıdaki tablo logical action’ları tekilleştirir. Kaynakta 78 `<button>`, 39 `<Link>`, 24 form kontrolü ve 18 `<details>/<summary>` declaration’ı vardır; tekrar eden öğrenci/soru satırları runtime’da daha fazla kontrol üretebilir.

| Görünen metin / kontrol | Sayfa | Görünme/disabled | Backend / job | Sonuç |
|---|---|---|---|---|
| `➕ Yeni Proje` | Projeler | Her zaman | Yok | `/project-create` |
| Proje kartı | Projeler | Proje listesinde; pending kart kilitlenir | `open_project` | Active project + workflow |
| `Atlanan projeler` | Projeler | skippedProjects varsa | Yok | Accordion |
| `Geri Dön` | Yeni proje | Her zaman | Yok | `/projects` |
| `📁` | Yeni proje | Her zaman | Dialog | Klasör yolu doldurur |
| `Proje Oluştur` | Yeni proje | Ad boşsa disabled | `create_project` | Project store + workflow |
| `PDF Yükle` | Belgeler | Belge rol kartında | `import_*_pdf` | Document + persistence |
| `Sil` | Belgeler | Mutation pending disabled; confirmation yok | `remove_document` | Belge silinir |
| `Önizlemeyi Oluştur` | Belgeler/PDF | Preview eksik veya yeniden başlatma | `start_*_preview_render` job | Progress/status |
| `Önizlemeyi Aç` | Belgeler/PDF | Preview hazır olduğunda | Yok | PDF route |
| `İş Akışı`, `Belgeler`, `PDF Önizlemeler` | Çeşitli | Link | Yok | Route navigation |
| `Soru Metnini Çıkar` | Soru metni/Workflow | PDF/model koşullarına bağlı | `start_question_text_extraction` job | Suggested text |
| Vision fallback / `Yeniden Dene` | Soru metni | extraction failure/available | `start_question_text_vision_fallback` job | Suggested text retry |
| `Kaydet` | Soru metni | Text valid/pending değil | `edit_question_text` | Edited text |
| `Onayla` / `Önerileri Onayla` | Soru metni | Suggested/missing olmayan kayıt | `confirm_question_text`, `confirm_all_question_texts` | Confirmed |
| `JSON Yükle` | Rubrik | File selected | `import_rubric_json` | Imported rubric |
| `Cevap Anahtarını Çıkar` | Rubrik | PDF/question count/model koşulları | `start_rubric_pdf_import` job | Suggested rubric |
| `Rubrikleri Doğrula` | Rubrik | Rubric items varsa | `validate_rubrics` | Validation report |
| `Rubriği Kaydet` | Rubrik card | Form validation | `update_question_rubric` | Edited rubric |
| `Rubriği Onayla` / `Tümünü Onayla` | Rubrik | Valid | `confirm_question_rubric`, `confirm_all_rubrics` | Confirmed rubric |
| `Paketi Dondur (Freeze)` | Sınav paketi | blocker/canConfirmAll ile disabled | Statik UI’da `confirmAllRubrics`; package build wrapper ayrı | Frozen label veya error |
| `Grupla` | Öğrenci gruplama | PDF yok/geçersiz sayfa disabled | `create_student_page_groups` | Submission list |
| `Grupları Onayla` | Öğrenci gruplama | Submission yoksa disabled | `mark_student_grouping_complete` | Grouping complete |
| `Sayfaları İncele` | Öğrenci gruplama | Submission satırında | Yok | Student PDF preview |
| `Kimlik OCR’ını Başlat` | Öğrenci kimliği | Crop template yoksa disabled | `start_student_identity_ocr` job | OCR suggestion |
| `Doğrula` / `Güncelle` | Öğrenci kimliği | Ad veya numara yoksa disabled | `update_student_identity` | Identity saved |
| `OCR detayı` | Öğrenci kimliği | identityOcr varsa | Yok | Raw JSON accordion |
| `Cevap alanları` / `Kimlik alanı` | Crop | Her zaman | Yok | Local mode switch |
| Preprocess chip’leri | Crop/OCR | Her zaman | Crop sayfasında `preprocess_ocr_image` | Preview mode |
| `Cevap Crop’larını Kaydet` | Crop | Draft varsa | `save_student_answer_crop_template` | Template persisted |
| `Kimlik Crop’unu Kaydet` | Crop | identityDraft yoksa disabled | `save_student_identity_crop_template` | Template persisted |
| `OCR Başlat` / `Yeniden Çalıştır` | Öğrenci OCR | readiness/model/crop koşulları | `start_student_answer_ocr` job | OCR records |
| `Kaydet` | Öğrenci OCR | Text edit | `update_student_answer_ocr_text` | Edited OCR |
| `Onayla` / `Tüm OCR Kayıtlarını Onayla` | Öğrenci OCR | Record valid / pending | `mark_student_answer_ocr_reviewed`, `mark_all...` | Teacher approved |
| Açık/Çözülen/Tümü filtreleri | OCR issue | Her zaman | Yok | Local filtered list |
| `Gemma ile öneriyi kontrol et` | OCR issue | Selected issue/model ready | `suggest_ocr_issue_correction_with_model` | Non-persistent suggestion |
| `Düzeltmeyi Uygula` | OCR issue | Suggestion available | `update_student_answer_ocr_text` | OCR text updated |
| `Bu OCR doğru` | OCR issue | Selected issue | `mark_student_answer_ocr_reviewed` | Issue resolved |
| `Sorunları Yeniden Tara` | OCR issue | Records exist | `rebuild_student_answer_ocr_issues` | Deterministic issue refresh |
| `Notlandırmayı Başlat` | Scoring | QEP frozen + OCR approved | `start_scoring_job` job | Scores |
| `Yeniden çalıştır` | Scoring | Existing run/stale | `start_scoring_job(forceRerun)` job | New active run |
| Manual score save | Scoring | Score input valid | `update_scoring_record` | Manual score/review |
| `Kâğıt İnceleme` | Scoring | Review data available | `get_graded_exam_review` read | Annotated review |
| `Önceki/Sonraki öğrenci` | Kâğıt inceleme | Queue bounds | Yok | Selected submission |
| Zoom/page icon buttons | PDF/Kâğıt | Page/zoom bounds | Yok | Local viewer state |
| `Başlat`, `Durdur`, `Health Check` | Model | Runtime state | `start_model_server`, `stop_model_server`, `probe_model_server` | Model status |
| Managed/External mode | Model | Profile/mutation pending | `set_model_mode` | Runtime mode |
| Argüman Önizleme | Model | Always | `preview_model_server_args` | Technical args |
| Profil Sıfırla | Model | Always | `reset_model_profile` | Profile reset |
| `Geliştirici Detayları`, `Teknik tanı ayrıntıları`, `Tanılama` | Çeşitli | `<details>` | Yok | Raw technical data |

## 5. Form alanları ve girdiler

| Alan | Sayfa | Tip | Zorunlu/validation | Backend alanı |
|---|---|---|---|---|
| Proje Adı | Yeni proje | text | Zorunlu; trim boş olamaz | `CreateProjectInput.name` |
| Klasör Yolu | Yeni proje | text + folder dialog | Optional/default path | `CreateProjectInput.rootPath` |
| Öğrenci başına sayfa | Gruplama | number | `1..20`, integer | `pages_per_student` |
| Ad Soyad | Kimlik | text | Ad veya numaradan biri doğrulama için yeterli | `display_name` |
| Okul No/Numara | Kimlik | text | Ad veya numaradan biri yeterli | `number` |
| Sınıf | Kimlik | text | Görsel olarak optional; format validation yok | `class_name` |
| Soru metni | Soru metni | text/textarea | Boş text blocker | `questionText.value` |
| Soru metni edit | Soru satırı | text/textarea | Backend confirm/edit | `edit_question_text.text` |
| Rubrik max puan | Rubrik | number | Pozitif; toplam/criterion uyumu | `max_score` |
| Beklenen cevap | Rubrik | textarea | Question-type rubric gate | `expected_answer` |
| Kriter adı/açıklaması | Rubrik | text/textarea | Kriter listesi/domain validation | `criteria[]` |
| Kriter puanı | Rubrik | number | Range ve toplam max | `criteria[].max_score` |
| Kısmi puan notları | Rubrik | textarea/list | Optional, placeholder invalid | `partial_credit_hints` |
| Sıfır puan koşulları | Rubrik | textarea/list | Optional | `zero_score_conditions` |
| Yaygın yanlışlar | Rubrik | textarea/list | Optional | `common_mistakes` |
| Cevap crop bbox | Crop | drag/coordinate draft | Page bounds; save öncesi local | `StudentAnswerCropTemplateItem` |
| Kimlik crop bbox | Crop | drag/coordinate draft | Page bounds; save öncesi local | `StudentIdentityCropTemplate` |
| OCR metni | Öğrenci OCR/issue | textarea | Empty warning; save command | `update_student_answer_ocr_text.text` |
| Manuel puan | Scoring | number | Max score/range; exact UI message statik incelemede doğrulanamadı | `update_scoring_record` |
| Preprocess mode | Crop/OCR | button chips/select-like | Enum mode | `preprocess_ocr_image.mode` |
| Issue filtresi | OCR issue | tabs/chips | Local `open/resolved/all` | Yok |
| Model mode/profile | Model | buttons | Enum external/managed | `set_model_mode` |

Native `select`, checkbox ve radio kontrolü production page’lerde tespit edilmedi; filtre ve preprocess seçimleri button/chip ile yapılır.

## 6. Durum etiketleri, uyarılar ve teknik veri

`src/utils/labels.ts` teacher-facing mapper’ları içerir. Önemli label grupları:

- Workflow: `Belgeler eksik`, `Soru metni onay bekliyor`, `Cevap anahtarı/rubrik eksik`, `Kırpma alanı eksik`, `OCR çalışıyor`, `Öğretmen kontrolü gerekli`, `Notlandırma hazır`, `Notlandırma tamamlandı`.
- Preview/job: `Eksik`, `Kuyrukta`, `Oluşturuluyor`, `Hazır`, `Başarısız`, `Çalışıyor`, `Başarılı`, `Kısmi`, `İptal edildi`.
- OCR: `Bekliyor`, `Üretildi`, `Kısmi`, `Kontrol gerekli`, `JSON çözülemedi`, `Kırpma eksik`, `Model hatası`, `Düzeltilmiş`, `Onaylandı`.
- Scoring: `Onay bekliyor`, `Onaylandı`, `Düzenlendi`, `Geçersiz`, `Puan uygulanmadı`, `Geçici toplam / Maksimum`.
- Warning: kritik terim belirsizliği, OCR parse failure, preprocess fallback, basılı metin karışması, kırpım eksik/truncated, düşük güven, scoring JSON doğrulanamaması.

Teacher-facing riskler:

1. `StudentGroupingPage` submission kısa ID’si açıkça `ID: ...` olarak görünür.
2. `StudentIdentityPage` `OCR detayı` raw JSON olarak açılabilir.
3. `StudentAnswerOcrPage` raw model output, preprocess version/ref sayıları ve render diagnostics açılabilir.
4. OCR issue debug panelinde `Prompt`, `Ham çıktı`, `Görsel`, confidence ve scope görünür.
5. `ModelStatusPage` PID, log path, completion probe ve `technicalDetails` görünür.
6. `ScoringPage` raw diagnostics, run/history detayları ve technical accordion’u görünür.
7. Home’de skipped project `technicalDetails` doğrudan gösterilir.

Bu paneller “teacher-facing UI’dan gizli developer paneli” olarak niyetlenmiş olsa da erişilebilirlikleri yalnızca `<details>` ile sınırlandırılmıştır; teknik veri teacher-facing build’e sızmaktadır.

## 7. Modal, confirmation ve diyaloglar

Kodda genel-purpose modal/confirmation sistemi görünmüyor. `QuestionCountDialog` gerçek modal benzeri tek ortak diyalogdur:

- `start_exam_package_build` veya `start_rubric_pdf_import` öncesi beklenen soru sayısını ister.
- Başlık: `Soru sayısını girin`.
- Input number ve cancel/kapat kontrolü vardır; submit sonrası ilgili job başlar.
- Cancel local dialog state’i kapatır; backend etkisi yoktur.

Dosya seçimi Tauri native dialog’dur. Belge silme, OCR rerun, scoring rerun, freeze, overwrite, sınıf taşıma/arşivleme ve model durdurma için confirmation modalı statik kodda bulunamadı. Bu alanlar özellikle `removeDocument`, force rerun, freeze ve stop işlemleri için UX riski taşır.

## 8. Backend command/service bağlantı matrisi

| UI | TS wrapper | Tauri command | Service/domain/job |
|---|---|---|---|
| Proje listesi/açma | `listProjects`, `openProject` | `project_commands` | `ProjectStore`, `Project`, workflow evaluation |
| Proje oluşturma | `createProject` | `project_commands` | `ProjectStore` |
| Workflow | `getWorkflowSnapshot`, `getProjectSnapshot`, `listJobs` | `workflow_commands`, `project_commands`, `job_commands` | `WorkflowEngine`, `ProjectStore`, `JobManager` |
| PDF import | `importExamSourcePdf`, `importAnswerKeyPdf`, `importStudentScanPdf` | document/student_scan commands | document/student scan service, `ProjectStore` |
| PDF preview | `start*PreviewRender`, status/list/get | `pdf_commands`, student scan commands | `PdfPreviewService`, `PdfService`, `JobManager` |
| Soru metni | `startQuestionTextExtraction`, fallback, confirm/edit | `question_text_commands` | `QuestionTextService`, `ModelRuntimeService`, `ModelGateway`, `JobManager` |
| Rubrik | import/list/update/confirm/validate/start PDF import | `rubric_commands` | `RubricService`, `RubricExtractionService`, `ProjectStore`, `JobManager` |
| Paket | `startExamPackageBuild`, `confirmAllRubrics`, validation | `exam_package_commands`, `rubric_commands` | `ExamPackageBuildService`, rubric/project store |
| Gruplama | create/list/update/delete/complete | `student_scan_commands` | `StudentScanService`, `ProjectStore` |
| Kimlik | start OCR/update identity | `student_answer_ocr_commands`, student scan command | `StudentAnswerOcrService`, `StudentScanService`, `JobManager` |
| Crop | save crop/preprocess/preview | student OCR commands, PDF commands | `StudentAnswerCropService`, `OcrImagePreprocessService` |
| OCR | start/update/review/rebuild/model suggestion | `student_answer_ocr_commands` | `StudentAnswerOcrService`, `ModelGateway`, `JobManager`, `ProjectStore` |
| Scoring | start/update | `scoring_commands` | `ScoringService`, `ModelRuntimeService`, `ModelGateway`, `JobManager` |
| Kâğıt inceleme | `getGradedExamReview` | `graded_exam_review_commands` | `GradedExamReviewService`, PDF preview read model |
| Model | status/probe/start/stop/mode/profile/args | `model_commands` | `ModelRuntimeService`, `ModelProcessManager`, `LlamaServerGateway` |

Frontend-only actions: navigation, accordions, filters, tabs, zoom, page selection, local crop draft and form drafts. Command çağırmadan “başarı” gösteren production domain action tespit edilmedi. Ancak `NextActions.tsx` içinde `/student-scans` route’una navigation vardır; `App.tsx` bu route’u register etmez. Bu action erişilemez/dead route adayıdır.

`src-tauri/src/lib.rs` command registration’ı çoğu wrapper ile uyumludur. UI’nin kanonik proje dosyasını topluca ezmesine izin veren `save_project` komutu kaldırılmıştır; proje mutasyonları yalnızca adlandırılmış domain komutları üzerinden yapılır. `get_model_runtime_status` ve `get_model_log_tail` gibi tanılama komutlarının wrapper/kullanım eşleşmesi ayrıca test edilmelidir.

## 9. Uçtan uca öğretmen kullanıcı yolculuğu

1. `/project-create`: Proje adı ve klasör yolu girilir; `Proje Oluştur` → `create_project`; `/workflow`.
2. `/documents`: Sınav PDF’i, cevap anahtarı/rubrik PDF’i ve öğrenci scan PDF’i yüklenir; import sonrası workflow güncellenir.
3. `/pdf-preview`: PDF preview job’ları başlatılır; sayfalar hazır olduğunda incelenir.
4. `/question-text`: Soru metni çıkarma job’ı başlatılır; suggested text düzenlenir/onaylanır.
5. `/rubric-preparation`: JSON/PDF import veya manuel rubrik düzenleme; validation ve öğretmen onayı.
6. `/exam-package-review`: soru/rubrik eşleşmesi incelenir; paket onay/freeze akışı.
7. `/student-grouping`: öğrenci PDF’i için sayfa sayısı girilir; `Grupla`, sonra `Grupları Onayla`.
8. `/student-identity`: Crop template oluşturulur, kimlik OCR başlatılır; ad/no/sınıf düzenlenir ve doğrulanır.
9. `/crop-template`: soru cevap crop’ları ve kimlik crop’u seçilir/kaydedilir; preprocess preview ile kontrol edilir.
10. `/student-answer-ocr`: OCR job’ı başlatılır; progress ve kayıtlar beklenir; metin düzenlenir/onaylanır.
11. `/student-answer-ocr-issues`: açık issue’lar filtrelenir; crop/text highlight incelenir; deterministic veya Gemma önerisi kontrol edilir, düzeltme uygulanır veya OCR doğru kabul edilir.
12. `/scoring`: QEP frozen ve OCR approved gate’leri geçilirse scoring job başlatılır; öğrenci accordion’larında sonuçlar incelenir.
13. `/graded-exam-review`: puanlı kâğıtlar sayfa üstü anotasyonlarla incelenir; önceki/sonraki öğrenci ile ilerlenir.

Production’da sınıf listesi/oluşturma/arkivleme veya PDF batch’i sınıfa bağlayan ayrı bir class module route’u bulunmuyor. `className` yalnızca öğrenci kimliği alanında tutuluyor ve `StudentIdentityEditor`/identity table ile güncelleniyor.

## 10. Ulaşılamayan, eski veya duplicate parçalar

- `src/pages/QuestionTextPage.tsx` bağımsız UI değil, `QuestionTextReviewPage` alias’ıdır.
- `src/design-reference/ai-studio/**` route’a bağlı değildir; mock/tasarım kapsamındadır.
- `NextActions.tsx` içindeki `/student-scans` linki `App.tsx` route ağacında yoktur; kullanıcı action’ı bu branch’e gelirse erişilemeyen route üretir.
- `StudentSubmissionList.tsx` ve `PageGroupEditor.tsx` reusable parçaları vardır; bazı submission/grouping UI’ları `StudentGroupingPage` içinde inline tutulur. Bu duplicate/dağınık yüzey bakım riskidir.
- `src-tauri/src/bin/rubrika.rs` CLI doctor/inspect/replay/repair araçlarıdır; production UI yüzeyi değildir.
- `src/design-reference` dışındaki bazı `fix_*.py` ve `refactor*.py` dosyaları production route/component ağacına bağlı değildir; UI envanterine dahil edilmemiştir.
- `/student-identity` canlı saf-Vite route taramasında yalnızca main container görünmüş, veri bağlamı gözlemi tamamlanamamıştır; neden statik incelemede doğrulanamadı.

## 11. UX ve tutarlılık sorunları

| Önem | Sorun / kullanıcı etkisi | Kaynak | Önerilen çözüm (uygulanmadı) |
|---|---|---|---|
| yüksek | `StudentGroupingPage` kısa submission ID’sini teacher-facing gösteriyor | `src/pages/StudentGroupingPage.tsx` | ID’yi kaldır veya diagnostics paneline taşı |
| yüksek | Model PID/log path/raw technicalDetails ve raw model output teacher build’de açılabilir | `ModelStatusPage.tsx`, `StudentAnswerOcrPage.tsx`, `StudentAnswerOcrIssueReviewPage.tsx`, `ScoringPage.tsx` | Developer mode/diagnostics erişim sınırı ekle |
| yüksek | `NextActions` `/student-scans` route’una navigasyon yapıyor; route register edilmemiş | `src/components/workflow/NextActions.tsx`, `src/app/App.tsx` | Mevcut `/student-grouping` route’una eşleştir veya route’u bilinçli tanımla |
| yüksek | Belge `Sil` işleminde confirmation yok | `src/pages/DocumentsPage.tsx` | Silme öncesi confirmation ve sonuç mesajı |
| orta | Freeze UI metni/akışı `confirmAllRubrics` ile bağlı; `startExamPackageBuild` ve frozen command sözleşmesiyle isimsel ayrışma var | `ExamPackageReviewPage.tsx`, `commands.ts`, `exam_package_commands.rs` | Freeze command/state contract’ını tekleştir |
| orta | UI bazı readiness özetlerini local hesaplıyor; backend snapshot ile drift riski var | `WorkflowPanel.tsx`, `NextActions.tsx` | Sadece backend `nextActions/blockingReasons` render edilmeli |
| orta | Global kabukta job/progress göstergesi yok; uzun işler yalnız sayfa içinde görülüyor | `AppLayout.tsx` | Global active job indicator eklenebilir |
| orta | 160px fixed sidebar uzun Türkçe label’ları ellipsis ile kesiyor; responsive collapse yok | `AppLayout.tsx` | Genişlik/breakpoint ve keyboard navigation tasarla |
| orta | Native semantic button yerine project card `div onClick` kullanıyor | `HomePage.tsx` | Keyboard/focus erişimi olan button/link kullan |
| orta | `aria-label` özellikle icon-only zoom/page/model kontrollerinde tutarlı değil | `ZoomControls.tsx`, `PageNavigation.tsx`, page icon buttons | Bütün icon-only kontrollere anlamlı aria-label ekle |
| orta | Tablo minimum genişlikleri dar ekranlarda yatay taşma üretiyor | `StudentIdentityPage.tsx`, `RubricQuestionCard.tsx` | Responsive table/card layout |
| düşük | Metinler tutarsız: `Projects sayfasına git`, `İş Akışı`, `İş akışına dön`; `Puanlama`/`Notlandırma` | çeşitli pages/components | Terminoloji sözlüğü ve ortak link component’i |
| düşük | Technical details `<details>` ile varsayılan erişilebilir ama teacher-facing görünür; raw JSON okunabilirliği düşük | çeşitli pages | Ayrı diagnostics ekranı/export |
| düşük | Job cancellation action görünür bir UI kontrolü olarak yok | job commands/pages | Job cancel contract ve kullanıcı kontrolü |
| düşük | Sınıf modülü yok; sınıf alanı yalnız identity text input | `StudentIdentityPage.tsx`, `StudentIdentityEditor.tsx` | Ayrı class domain/UI gerekiyorsa tasarla |

## 12. Teacher-facing teknik veri sızıntıları

Doğrudan tespit edilenler: submission kısa UUID, project path, skipped project technical details, identity OCR raw JSON, OCR crop/source refs ve raw model output, issue prompt/raw output/used image ref, model PID/log path/technical details, scoring run/diagnostic accordions. `submissionId`, `runId` ve hash alanlarının tüm varyantları aktif fixture olmadan ekranda doğrulanamadı; ancak `ScoringPage` ve API/domain tiplerinde run/history alanları mevcut olduğundan “statik incelemede doğrulanamadı” kabul edilmelidir.

## 13. Test edilmemiş veya statik incelemede doğrulanamayan noktalar

- Aktif `/Users/kadir/Documents/RubrikaV3/Projects/11_46` projesiyle gerçek veri üzerinden route gezinimi.
- PDF dosya seçimi, preview rendering, crop drag koordinatlarının persistence sonucu.
- Model start/stop/health, llama-server response ve raw diagnostics içerikleri.
- Her job’ın gerçek progress event sırası ve stale refresh davranışı.
- Scoring active/latest run duplicate history ve manual score persistence.
- Freeze command’ın gerçek backend state transition’ı.
- `/student-identity` canlı route içeriğinin saf Vite gözleminde boş kalma nedeni.
- Dar viewport, klavye navigation, focus ring, contrast ve screen-reader deneyimi.

## 14. Özet sayılar

- Production route tanımı: **18**
- Benzersiz production page component’i: **16**
- Production navigasyon maddesi: **14**
- Statik JSX button declaration: **78**
- Statik JSX Link declaration: **39**
- Statik form control declaration (`input`, `textarea`, `select`): **24**
- Statik `details/summary` declaration: **18**
- Logical action grubu: **yaklaşık 50**
- Backend command’a bağlı ana UI aksiyonu: **yaklaşık 38**
- Yalnız frontend/local davranan aksiyon grubu: **yaklaşık 12** (navigation, filters, tabs, accordions, zoom, local draft)
- Ayrı modal/confirmation: **1 ortak QuestionCountDialog + native file dialog**
- Class module route: **0**
- Belirgin erişilemeyen/dead route adayı: **1** (`/student-scans`)
- Belirgin teknik veri sızıntısı grubu: **7**
- Raporlanan UX/tutarlılık sorunu: **14**

## 15. Kaynak referans özeti

- Route/shell: `src/app/App.tsx`, `src/app/AppLayout.tsx`
- Proje yönetimi: `src/pages/HomePage.tsx`, `src/pages/ProjectCreatePage.tsx`, `src/state/projectSession.ts`
- Workflow: `src/pages/WorkflowPage.tsx`, `src/components/workflow/WorkflowPanel.tsx`, `src/components/workflow/NextActions.tsx`
- UI actions/API: `src/api/commands.ts`, `src/api/types.ts`, `src/api/errors.ts`
- Labels: `src/utils/labels.ts`
- PDF: `src/pages/PdfPreviewPage.tsx`, `src/components/pdf/PdfPageViewer.tsx`, `PageNavigation.tsx`, `ZoomControls.tsx`
- OCR: `src/pages/StudentAnswerOcrPage.tsx`, `src/pages/StudentAnswerOcrIssueReviewPage.tsx`, `src/pages/CropTemplatePage.tsx`
- Scoring/review: `src/pages/ScoringPage.tsx`, `src/pages/GradedExamReviewPage.tsx`, `src/components/scoring/ScoredExamReviewPanel.tsx`
- Backend registration: `src-tauri/src/lib.rs`, `src-tauri/src/commands/mod.rs`
- Backend services/domain: `src-tauri/src/services/**`, `src-tauri/src/domain/**`, `src-tauri/src/jobs/**`
- Contract/maps: `docs/API_CONTRACTS.md`, `docs/PROJECT_MAP.md`, `docs/FEATURE_FLOW_MAP.md`, `docs/FILE_OWNERSHIP_MAP.md`, `docs/SYMBOL_MAP.md`
