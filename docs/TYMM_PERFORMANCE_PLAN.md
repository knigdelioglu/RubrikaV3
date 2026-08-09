> **DURUM: RETIRED / REMOVED (2026-08-08)**
>
> Bu plan uygulanmış ve TYMM Performans Değerlendirme modülü RubrikaV3'ten
> tamamen kaldırılmıştır. Bu dosya yalnız tarihsel arşivdir; aktif bir özellik
> planı değildir. Aktif workflow'da `performance` assessment türü yoktur;
> yalnız eski test projelerinin açılabilmesi için `AssessmentType::LegacyPerformance`
> tombstone variantı (serde `alias = "performance"`) tutulur.

# TYMM Performans Değerlendirme İş Akışı — Uygulama Planı

**Kaynak:** `docs/TYMM_PERFORMANCE_SCALE_REPORT.md` (3 Ağustos 2026 araştırma raporu)
**Hedef:** RubrikaV3'e TYMM uyumlu performans değerlendirme iş akışı eklemek; pilot kapsam 9. sınıf Türk Dili ve Edebiyatı.

## 1. Amaç, kapsam ve değişmezler

Eklenen özellik "performans ölçeği doldurma ekranı" değil, **TYMM uyumlu performans değerlendirme iş akışıdır**:

**Görev → Ölçek/Rubrik → Kanıt/Ürün → Öğrenci değerlendirmesi (öz/akran) → Öğretmen kararı (nihai)**

Performans görevi, yazılı sınav akışından (PDF/OCR/QEP) **bağımsız** ayrı bir akıştır; mevcut ders, dönem, sınıf ve öğrenci organizasyonu yeniden kullanılır.

### Değişmezler (rapor §8'den, kodda da kural olarak uygulanır)

1. Mevcut yazılı sınav `ScoringRecord` yapısı performans değerlendirmesi için **kullanılmaz**.
2. Performans görevi tek bir sayı giriş ekranına indirgenmez (görev + ölçek + kanıt + değerlendirme + onay bütünü korunur).
3. Her görev otomatik olarak ayrı dönem notuna dönüştürülmez.
4. Öğretmen puanı akran/öz değerlendirme ortalamasıyla otomatik değiştirilmez.
5. Grup çalışmasında herkese otomatik aynı puan verilmez.
6. Rubrik değiştikten sonra eski puanlar sessizce yeniden hesaplanmaz (sürümleme).
7. Eksik ürün otomatik olarak sıfır kabul edilmez (Missing/NotPerformed ≠ 0).
8. Yapay zekâya nihai puan verme yetkisi tanınmaz.

## 2. Mimari kararlar

- **K1 — Organizasyon yeniden kullanımı:** `AssessmentType::Performance` varyantı ve `WorkflowFamily::Performance` eklenir. `AssessmentActivity` ortak görev kaydı olarak yeniden kullanılır (ders, sınıf, dönem, tema, teklik anahtarı, sıra numarası, `ClassApplication`'lar). Performans "alanı" ayrı UI akışı ve ayrı değerlendirme kayıtlarıyla sağlanır; yazılı sınav workflow'una karışmaz.
- **K2 — Yeni domain tipleri:** `src-tauri/src/domain/performance.rs` (yeni): `PerformanceDetails`, `PerformanceRubric`, `PerformanceCriterion`, `PerformanceLevel`, `PerformanceAssessment`, `CriterionRating`, `PerformanceAssessmentStatus`.
- **K3 — Değerlendirme saklama:** Kayıtlar `ClassApplication.performance_assessments` altında canonical olarak saklanır (`SpeakingAttempt` deseni). `ScoringRecord` ve yazılı sınav cevap yapılarına dokunulmaz.
- **K4 — Servis sahipliği:** Yeni `PerformanceService` (tek sahip): görev CRUD, rubrik sürümleme, değerlendirme kaydı, geçici toplam hesaplama, onay kuralları, eksik durum yönetimi. `ProjectStore` tek-yazar desenine uyar.
- **K5 — Komut katmanı:** Yeni `commands/performance_commands.rs`; `AppState`'e `PerformanceService` Arc olarak eklenir; `invoke_handler`'a kaydedilir.
- **K6 — Frontend:** Ayrı `PerformanceOrganizationPage` (görev + rubrik oluşturma, şablonlardan başlatma) ve `PerformanceScoringPage` (sınıf listesi → öğrenci → ölçüt işaretleme → geçici toplam + eksik uyarısı → geri bildirim → onay). Mevcut sınav organizasyon sayfasındaki tür listesinde performans ayrı görünür.
- **K7 — TDE şablonları:** Yerleşik 3 şablon (rapor §7): yazılı ürün (5 ölçüt), sözlü performans (5 ölçüt), grup çalışması/drama (5 ölçüt). Her biri 3-6 ölçüt, 3 veya 5 düzey, gözlenebilir tanımlar; zümre tarafından düzenlenebilir.
- **K8 — Rubrik sürümleme:** Her yayın yeni sürüm üretir. Değerlendirme kaydı, kullanılan rubrik sürümünü sabitler. Onaylı değerlendirmesi olan rubrik değiştirilemez; değişiklik yeni sürüm ister. Eski puanlar yeniden hesaplanmaz.
- **K9 — Eksik durumlar:** `Missing` (teslim edilmedi) ve `NotPerformed` (performans gösterilmedi) ayrı durumlardır; sıfır puanla karışmaz, raporda ayrı gösterilir. Onay, tüm zorunlu ölçütler değerlendirilmeden verilemez.

## 3. Mevcut mimariyle eşleştirme (dosya referansları)

| Katman | Dosya | Kullanım |
|---|---|---|
| Domain | `src-tauri/src/domain/assessment.rs` | `AssessmentType`, `WorkflowFamily`, `AssessmentActivity`, `ClassApplication` (Performance varyantı + `performance_details` + `performance_assessments` buraya eklenir) |
| Domain | `src-tauri/src/domain/speaking.rs` | `SpeakingAttempt` deseni — class application altında attempt saklama referansı |
| Servis | `src-tauri/src/services/assessment_organization_service.rs` | Organizasyon kuralları: teklik anahtarı, class application doğrulama, sıra numarası |
| Servis | `src-tauri/src/services/project_store.rs` | Canonical JSON persistence + idempotent migration + reload testleri |
| Servis | `src-tauri/src/services/speaking_exam_service.rs` | Attempt lifecycle örneği (create → validate → persist) |
| App | `src-tauri/src/lib.rs` | `AppState` servis kayıtları + `invoke_handler` |
| Komut | `src-tauri/src/commands/speaking_exam_commands.rs` | Komut katmanı deseni (typed input/output, hata eşleme) |
| Frontend | `src/pages/assessmentOrganizationUi.ts`, `AssessmentOrganizationPage.tsx` | Mevcut organizasyon UI; performans giriş noktası |
| Frontend | `src/pages/speechExamUi.ts`, `SpeechExamPage.tsx` | Attempt akış UI deseni (performans değerlendirme ekranı için) |
| Frontend | `src/api/commands.ts`, `src/api/types.ts` | Tauri client + tipler (Performance tipleri eklenir) |
| Frontend | `src/app/` | Routing + navigasyon |
| Doküman | `docs/API_CONTRACTS.md`, `FILE_OWNERSHIP_MAP.md`, `SYMBOL_MAP.md`, `FEATURE_FLOW_MAP.md`, `ASSESSMENT_ORGANIZATION.md` | Faz C'de güncellenir |

## 4. Veri modeli tasarımı (Rust)

Yeni dosya `src-tauri/src/domain/performance.rs`:

```rust
// serde: rename_all = "snake_case" (enum'lar) / "camelCase" (struct'lar)

pub enum PerformanceSkillArea { Reading, ListeningWatching, Speaking, Writing }
pub enum PerformanceWorkMode { Individual, Group }

pub struct PerformanceDetails {
    pub theme: String,                    // tema
    pub learning_outcomes: Vec<String>,   // öğrenme çıktıları
    pub skill_area: PerformanceSkillArea, // TDE beceri alanı
    pub task_instruction: String,         // görev yönergesi
    pub work_mode: PerformanceWorkMode,   // bireysel / grup
    pub due_date: Option<String>,
    pub evidence_types: Vec<String>,      // yazılı ürün, ses/video, sunum, drama...
}

pub struct PerformanceLevel {
    pub id: String, pub name: String,     // örn. "Başlangıç".."Çok iyi" (3 veya 5 düzey)
    pub points: u32, pub description: String,
}

pub struct PerformanceCriterion {
    pub id: String, pub name: String, pub description: String,
    pub level_descriptions: Vec<LevelDescription>, // her düzey için gözlenebilir tanım
}
pub struct LevelDescription { pub level_id: String, pub description: String }

pub struct PerformanceRubric {
    pub id: String, pub name: String, pub version: u32,
    pub criteria: Vec<PerformanceCriterion>,   // 3-6 ölçüt
    pub levels: Vec<PerformanceLevel>,         // 3 veya 5 düzey
    pub created_at: String,
}

pub enum PerformanceAssessmentStatus { InProgress, Approved, NotPerformed, Missing }

pub struct CriterionRating {
    pub criterion_id: String, pub level_id: String, pub note: Option<String>,
}

pub struct PerformanceAssessment {
    pub id: String,
    pub student_id: String,
    pub rubric_id: String, pub rubric_version: u32, // sürüm sabitleme (K8)
    pub ratings: Vec<CriterionRating>,
    pub provisional_total: u32,   // geçici toplam; servis hesaplar
    pub feedback: Option<String>,
    pub status: PerformanceAssessmentStatus,
    pub assessed_at: Option<String>,
    pub approved_at: Option<String>,
    pub created_at: String, pub updated_at: String,
}
```

### Şema etkisi ve migration

- `AssessmentActivity.assessment_type` → `performance` varyantı; yeni `performance_details: Option<PerformanceDetails>` (serde `default` + `skip_serializing_if`).
- `ClassApplication.performance_assessments: Vec<PerformanceAssessment>` (serde `default`).
- `WorkflowFamily::Performance` eklenir; mevcut `workflow_family()` eşlemesi güncellenir.
- Migration: eksik alanlar serde default ile açılır; idempotent, atomik JSON yazımı, timestamp'li backup (mevcut ProjectStore deseni). Eski projeler bozulmadan açılır; eski sınavların `assessmentType` değeri değişmez.
- Yeni varyant, Rust'ta `WorkflowFamily`/`AssessmentType` üzerindeki tüm `match`'leri derleme hatasıyla ortaya çıkarır (exhaustiveness) — Faz A'da tüm noktalar güncellenir.

## 5. Fazlı uygulama

Her faz tek başına derlenebilir/test edilebilir durumda biter. Fazlar sırayla, tek opencode çağrısıyla uygulanır. **Hiçbir fazda git commit/push yapılmaz.**

### Faz A — Rust backend (domain + servis + komutlar)

Kapsam:
1. `src-tauri/src/domain/performance.rs` (yeni): §4 tipleri.
2. `src-tauri/src/domain/assessment.rs`: `AssessmentType::Performance`, `WorkflowFamily::Performance`, `AssessmentActivity.performance_details: Option<PerformanceDetails>`, `ClassApplication.performance_assessments: Vec<PerformanceAssessment>`; mevcut match'lerin tamamı güncellenir (workflow_family, sequence, doğrulama).
3. `src-tauri/src/services/performance_service.rs` (yeni) — `PerformanceService`:
   - Görev CRUD: oluşturma/güncelleme (teklik anahtarı `academicYearId + courseId + gradeLevel + term + assessmentType + sequenceNumber` korunur; görevlendirme/sınıf/öğrenci doğrulama `SchoolClassService` + `AssessmentOrganizationService` üzerinden).
   - Rubrik sürümleme: yayın = yeni sürüm; onaylı değerlendirmesi olan rubrik değiştirilemez; rubrik doğrulama: 3-6 ölçüt, 3 veya 5 düzey, her düzeyde tanım, puanlar tutarlı.
   - Değerlendirme: ölçüt bazında düzey seçimi kaydı, geçici toplam hesabı (servis hesaplar, istemci güvenilmez), geri bildirim.
   - Onay: tüm ölçütler değerlendirilmeden onay reddi; onay tarihi + rubrik sürümü kayda yazılır; onay sonrası düzenleme reddi (yeni değerlendirme açılabilir).
   - Eksik durumlar: `Missing` / `NotPerformed` işaretleme; bu kayıtlar toplamda yok sayılır, sıfır yazılmaz.
4. `src-tauri/src/services/project_store.rs`: şema uzantısı + idempotent migration + reload/migration testleri.
5. `src-tauri/src/commands/performance_commands.rs` (yeni):
   - `create_performance_task`, `update_performance_task`, `list_performance_tasks`, `get_performance_task`
   - `publish_performance_rubric`, `get_performance_rubric_history`
   - `save_performance_assessment`, `approve_performance_assessment`, `set_performance_assessment_status` (Missing/NotPerformed), `list_performance_assessments`
6. `src-tauri/src/lib.rs`: `AppState` + `invoke_handler` kaydı.
7. `src/api/types.ts`: `AssessmentType`/`WorkflowFamily`'e `performance`; Performance tipleri (Faz A sonunda frontend `npm run build`'i kırmadan).
8. Testler:
   - Domain/servis birim: rubrik doğrulama (ölçüt/düzey sayısı, tanım zorunluluğu), sürümleme (eski değerlendirme etkilenmez), onay kuralları (eksik ölçüt reddi), eksik≠sıfır, geçici toplam.
   - ProjectStore: eski JSON açılışı, migration idempotence, reload.
   - Komut kontratları: her komutun typed input/output + hata senaryoları.
9. Doğrulama: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `npm run build` (frontend tip uyumu), `npm run typecheck`.

### Faz B — Frontend iş akışı (organizasyon + değerlendirme ekranları)

Kapsam:
1. `src/pages/performanceOrganizationUi.ts` + `PerformanceOrganizationPage.tsx` (yeni):
   - Görev listesi (ders/sınıf/dönem filtresiyle; tür: performans).
   - Görev oluşturma/düzenleme formu: tema, öğrenme çıktıları, beceri alanı, yönerge, bireysel/grup, teslim tarihi, kanıt türleri.
   - Rubrik düzenleyici: ölçüt ekle/sil (3-6 sınırı), düzey sayısı (3/5), her düzey için ad + puan + gözlenebilir tanım; yayınlama = yeni sürüm; onaylı rubrik kilitli.
   - Şablonlardan başlatma: yazılı ürün / sözlü performans / grup çalışması (rapor §7 ölçütleri).
2. `src/pages/performanceScoringUi.ts` + `PerformanceScoringPage.tsx` (yeni):
   - Sınıf uygulaması seçimi → öğrenci listesi → öğrenci seçimi.
   - Ölçüt bazında düzey işaretleme (rubrik tanımları görünür), gerekçe notu, geri bildirim.
   - Geçici toplam + eksik ölçüt/eksik öğrenci uyarıları; `Missing`/`NotPerformed` işaretleme (sıfırdan ayrı, görsel olarak da ayrı).
   - Onay akışı: zorunlu ölçütler tamamlanmadan onay butonu kapalı.
3. Routing + navigasyon: `src/app/` (performans organizasyon ve değerlendirme rotaları; mevcut organizasyon sayfasından tür ayrımıyla giriş).
4. Testler: **yok — kullanıcı politikası gereği test yalnızca büyük işlerde (mimari değişiklik/büyük refactor) yazılır/çalıştırılır.** Faz B orta ölçekli bir iştir; opencode görev dosyasına test dayatması yazılmaz. (Kullanıcı açıkça isterse ayrı bir görevle eklenir.)
5. Doğrulama (dar): yalnız `npm run build` (TS derlemesi + vite; frontend kırılmadan derlenmeli). `typecheck` build'e dahilse ayrıca koşulmaz. `npm run lint`, `npm test` **koşulmaz** (kural).

### Faz C — TDE şablonları, raporlama ve kapanış

Kapsam:
1. Yerleşik 9. sınıf TDE görev şablonları (servis sabitleri veya başlangıç verisi): yazılı ürün / sözlü performans / grup çalışması — rapor §7 ölçüt setleriyle, 5 ölçüt + 4 düzey (veya rapordaki yapıya uygun 3-6 ölçüt / 3-5 düzey). Şablonlar öğretmen tarafından düzenlenip onaylanabilir (zümre kararı).
2. Performans sonuç raporu: öğrenci bazlı ölçüt düzeyleri, geçici/onaylı toplam, geri bildirim; `Missing`/`NotPerformed` sıfırdan ayrı gösterilir. PDF çıktısı mevcut `pdf_service` altyapısıyla; Excel çıktısı mevcut raporlama deseniyle (projede hangisi varsa o desen).
3. Rubrik sürüm geçmişi görünümü (zümre onayı için: sürüm listesi + kilit durumu).
4. Doküman güncellemeleri: `docs/API_CONTRACTS.md` (yeni komut sözleşmeleri), `docs/FILE_OWNERSHIP_MAP.md`, `docs/SYMBOL_MAP.md`, `docs/FEATURE_FLOW_MAP.md`, `docs/ASSESSMENT_ORGANIZATION.md` (performans bölümü).
5. Doğrulama (dar — kural): Faz C küçük/orta ölçekli iştir; tam kalite gate (`npm run quality` vb.) **dayatılmaz**. Yalnız değişiklik alanına uygun doğrulama: `cargo fmt --check` + `cargo clippy` (şablonlar Rust tarafındaysa) ve/veya `npm run build` (rapor UI değişiyorsa). Kullanıcı isterse kapanışta ayrı bir tam kalite gate görevi çalıştırılır.
6. Git commit/push YOK — sonuç raporu kullanıcıya sunulur.

## 6. Test stratejisi

**Kullanıcı politikası (her faza uygulanır):** testler yalnızca **büyük işlerde** (mimari değişiklik, büyük refactor) yazılır ve çalıştırılır; küçük/orta işlerde test, typecheck, lint, clippy, fmt dayatması yoktur; büyük işlerde bile test **dar tutulur** ve süre maliyeti kullanıcıya bildirilir. Faz A (mimari değişiklik) bu kapsamda test içerir; Faz B/C'de test dayatması yoktur.

- **Rust birim (servis):** rubrik doğrulama (3-6 ölçüt, 3/5 düzey, boş tanım reddi, puan tutarlılığı), sürümleme (yayın sonrası eski değerlendirme değişmez, onaylı rubrik kilidi), onay kuralları (eksik ölçüt reddi, onay sonrası düzenleme reddi), eksik durum ≠ sıfır, geçici toplam hesabı.
- **Rust komut kontratları:** her komutun typed input/output doğrulaması, geçersiz girdi hata senaryoları, yetkisiz erişim (yanlış activity/class/student) reddi.
- **ProjectStore:** eski şema JSON'ının açılışı, migration idempotence, reload sonrası veri bütünlüğü.
- **Frontend (node --test + `--experimental-strip-types` deseni):** form doğrulama, toplam hesabı, eksik uyarıları, onay akışı, şablon yükleme.
- **Gate'ler:** `npm run quality` (AGENTS.md §5-7 seviyelerine uygun); her faz kendi kapsamındaki gate'leri geçmeden bitmez.

## 7. Riskler ve açık sorular

1. **Exhaustiveness:** Yeni enum varyantları Rust'ta tüm match'leri derleme hatasıyla bulur; frontend'de `workflowFamily`/`assessmentType` switch'leri elle güncellenmeli — Faz A'da `npm run build` + mevcut frontend testleri (`assessmentOrganizationUi.test.ts` vb.) regresyonu yakalar.
2. **Migration:** Mevcut JSON projeler bozulmadan açılmalı; serde `default` + idempotent migration + reload testi zorunlu. Bilinen gerçek proje verisi varsa açılış testiyle doğrulanır.
3. **Sequence/teklik anahtarı:** Performans türü kendi sıra numarası alanında ilerler (dönem/tür/sınıf düzeyi kapsamı); yazılı sınav sıra numaralarıyla karışmaz.
4. **Onaylı rubrik kilidi:** Zümre "onay" süreci uygulamada basit tutulur (sürüm geçmişi + kilit); gerçek zümre onay iş akışı (çok kullanıcı) kapsam dışıdır.
5. **e-Okul dönem performans puanı:** Okul/zümre uygulamasına bağlı; uygulama kapsamı dışında, rapora ayrı yansıtılır.
6. **Kanıt/ürün dosyaları (ses/video, taslaklar, öz/akran formları):** Rapor §9 ikinci aşama; bu planda `evidence_types` alanıyla hazırlanır, dosya yükleme Faz B/C'ye **eklenmez** (kapsam disiplini — ilk pilot öğretmen kararı + rubrik + rapordur).
7. **AI yetkisi:** AI hiçbir noktada nihai puan üretmez; ileride yalnız taslak/gerekçe önerisi düşünülebilir (raporda "yapılmaması gerekenler" maddesi).

## Kapanış notu

Bu plan `docs/TYMM_PERFORMANCE_SCALE_REPORT.md` ile birlikte okunur. Rapor tasarım kararlarının gerekçesini, bu plan uygulamanın kapsamını tanımlar. Faz sonlarında `task.md`'ye çalışma durumu eklenir (mevcut gelenek).



