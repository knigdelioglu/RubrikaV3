# GÖREV: TYMM Performans Değerlendirme — FAZ A (Rust Backend)

RubrikaV3 projesine TYMM uyumlu performans değerlendirme iş akışının **Faz A — Rust backend** katmanını uygula. Bu fazda **yalnızca backend + frontend tip uyumu** yapılır; UI sayfaları Faz B'de yapılacak (bu fazda UI sayfası YAZMA).

## Bağlam (önce oku)

1. `docs/TYMM_PERFORMANCE_SCALE_REPORT.md` — araştırma raporu (tasarım gerekçeleri, yasaklar §8, şablon ölçütleri §7).
2. `docs/TYMM_PERFORMANCE_PLAN.md` — uygulama planı (bu görevin kapsamı §5 "Faz A" + §4 veri modeli + §1 değişmezler + §6 test stratejisi).
3. `AGENTS.md` — mühendislik kuralları (küçük doğru değişiklik, tek yazar ProjectStore, typed komutlar, üretim yolunda unwrap/panic yok).
4. `docs/ASSESSMENT_ORGANIZATION.md` — mevcut organizasyon modeli.
5. İlgili kod: `src-tauri/src/domain/assessment.rs`, `src-tauri/src/domain/speaking.rs` (desen referansı), `src-tauri/src/services/assessment_organization_service.rs`, `src-tauri/src/services/project_store.rs`, `src-tauri/src/services/speaking_exam_service.rs` (desen referansı), `src-tauri/src/commands/speaking_exam_commands.rs` (komut deseni), `src-tauri/src/lib.rs` (AppState + invoke_handler), `src/api/types.ts` ve `src/api/commands.ts` (frontend client).

## Kapsam (tamamı bu fazda)

### 1. Domain — `src-tauri/src/domain/performance.rs` (yeni)
Plan §4'teki tipler: `PerformanceSkillArea` (Reading/ListeningWatching/Speaking/Writing), `PerformanceWorkMode` (Individual/Group), `PerformanceDetails`, `PerformanceLevel`, `PerformanceCriterion`, `LevelDescription`, `PerformanceRubric`, `CriterionRating`, `PerformanceAssessment`, `PerformanceAssessmentStatus` (InProgress/Approved/NotPerformed/Missing). Serde isimlendirme mevcut kodla tutarlı (enum snake_case, struct camelCase). Mevcut domain dosyalarındaki desenlere uy (skip_serializing_if vb.).

### 2. Domain — `src-tauri/src/domain/assessment.rs` (güncelle)
- `AssessmentType::Performance` varyantı ekle; `workflow_family()` → `WorkflowFamily::Performance`.
- `WorkflowFamily::Performance` ekle (Default DEĞİŞMEZ — Written kalır).
- `AssessmentActivity`'ye `#[serde(default, skip_serializing_if = "Option::is_none")] pub performance_details: Option<PerformanceDetails>`.
- `ClassApplication`'a `#[serde(default)] pub performance_assessments: Vec<PerformanceAssessment>`.
- Aynı dosyada/ilgili servislerde `AssessmentType`/`WorkflowFamily` üzerindeki TÜM match'leri derleyici hatalarından giderek güncelle (exhaustiveness; yeni varyantı ihmal etme, `_ =>` arm'ına gizleme — her yer anlamlı davranışa sahip olmalı).

### 3. Servis — `src-tauri/src/services/performance_service.rs` (yeni) + `mod.rs` kaydı
`PerformanceService` (diğer servislerin desenine uy; gereken bağımlılıklar Arc olarak alınır):
- `create_performance_task` / `update_performance_task`: `AssessmentActivity` (assessment_type=performance) + `PerformanceDetails` + rubrik oluşturur. Teklik anahtarı korunur: `academicYearId + courseId + gradeLevel + term + assessmentType + sequenceNumber` (AssessmentOrganizationService kurallarıyla tutarlı). Sınıf uygulamaları ClassApplication olarak eklenir; öğrenci kapsamı merkezi SchoolClassService roster'ından doğrulanır.
- `publish_performance_rubric`: rubrik doğrulaması — 3-6 ölçüt; 3 veya 5 düzey; her ölçütün adı + açıklaması boş değil; her düzey için gözlenebilir tanım zorunlu; düzey puanları tutarlı (azalan sıra, eşit puan yok). Yayın = yeni sürüm (version+1). **Onaylı değerlendirmesi olan rubrik değiştirilemez** (yeni sürüm bile açılamaz; hata döner).
- `save_performance_assessment`: ölçüt bazında düzey seçimi (`CriterionRating`); geçici toplamı **servis** hesaplar (istemci girdisine güvenilmez); ölçüt id'leri ve düzey id'leri rubriğe ait olmalı; kayıt `rubric_id` + `rubric_version` sabitler.
- `approve_performance_assessment`: **tüm ölçütler değerlendirilmemişse reddet**; onay tarihi + sürüm yazılır; onay sonrası `save` reddedilir (yeni değerlendirme açılabilir).
- `set_performance_assessment_status`: `Missing` / `NotPerformed` işaretleme — bu kayıtlara sıfır PUAN YAZILMAZ (toplam hesabına girmez, raporda ayrı gösterilir).
- Görev/sınıf/öğrenci sahiplik doğrulamaları mevcut servislerin deseninde (activity → class application → student üyelik).
- Sıra numarası (sequence_number) performans türü kendi kapsamında ilerler; yazılı sınav sıralarıyla karışmaz.

### 4. Kalıcılık — `src-tauri/src/services/project_store.rs` (güncelle)
- Yeni alanlar serde default ile açılır; migration **idempotent** ve mevcut verileri bozmaz (mevcut migration desenini izle; timestamp'li backup, atomik yazım).
- Eski şema JSON'ının açılışı ve reload bütünlüğü için test ekle.

### 5. Komutlar — `src-tauri/src/commands/performance_commands.rs` (yeni) + `mod.rs` + `lib.rs`
Typed input/output (mevcut komut deseni), hatalar yapısal `AppError`'a eşlenir:
- `create_performance_task`, `update_performance_task`, `list_performance_tasks` (ders/sınıf/dönem filtresi), `get_performance_task`
- `publish_performance_rubric`, `get_performance_rubric_history`
- `save_performance_assessment`, `approve_performance_assessment`, `set_performance_assessment_status`, `list_performance_assessments`
- `AppState`'e `performance_service: Arc<PerformanceService>`; `invoke_handler`'a kayıt.

### 6. Frontend tip uyumu (yalnız tipler — UI yok)
- `src/api/types.ts`: `AssessmentType`/`WorkflowFamily` birliklerine `'performance'`; Performance DTO tipleri (backend serde çıktısıyla birebir).
- `src/api/commands.ts`: yeni komutların client fonksiyonları (invoke sarmalayıcıları).
- Mevcut frontend kodunda `assessmentType`/`workflowFamily` switch'lerinde kırılma varsa **minimal** uyum düzeltmesi yap (UI davranışı ekleme — Faz B'nin işi; örn. performans türü için mevcut sayfalarda "desteklenmiyor/açılamaz" pasif durum yeterli olabilir; ancak kapsam dışı UI değişikliği yapma — önce mevcut testlerin ne beklediğine bak).

### 7. Testler (bu fazda)
- Servis birim: rubrik doğrulama (3-6 ölçüt, 3/5 düzey, boş tanım reddi, puan tutarlılığı), sürümleme (eski değerlendirme yayın sonrası değişmez; onaylı rubrik kilidi), onay kuralları (eksik ölçüt reddi; onay sonrası kayıt reddi), eksik≠sıfır, geçici toplam hesabı.
- ProjectStore: eski JSON açılışı + migration idempotence + reload.
- Komut kontratları: typed giriş/çıkış, geçersiz girdi hataları, sahiplik reddi (yanlış activity/class/student).
- Mevcut testler kırılmamalı (özellikle assessmentOrganization/speaking ile ilgili).

## Kısıtlar (değişmez)

- `ScoringRecord` ve yazılı sınav puanlama yapılarına DOKUNMA.
- PDF/OCR/QEP akışına dokunma.
- UI sayfası yazma (Faz B kapsamı).
- Git add/commit/push YAPMA; dosya değişikliklerini olduğu gibi bırak.
- Kapsamı genişletme (AGENTS.md: küçük doğru değişiklik; görülen iyileştirmeleri rapora yaz, uygulama).
- Üretim yollarında unwrap/expect/panic yok; typed hatalar.
- Türkçe yorum/string'ler uygun yerlerde (mevcut kodun diline uy).

## Doğrulama (kullanıcı politikası: test yalnızca büyük işlerde; bu faz mimari değişiklik içerdiğinden dar tutulmuş doğrulama yapılır — tam suite DEĞİL)

1. `cargo fmt --manifest-path src-tauri/Cargo.toml --check` (bozuksa `cargo fmt --manifest-path src-tauri/Cargo.toml` ile formatla)
2. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` (yeni enum varyantları = mimari değişiklik; derleyici doğrulaması zorunlu)
3. `cargo test --manifest-path src-tauri/Cargo.toml` (yalnız bu fazda eklenen/yeni servis testleri ve migration testleri; tüm suite'i ayrıntılı raporlama)
4. `npm run build` + `npm run typecheck` (frontend tip uyumu — build kırılmasın; zorunlu)
5. `npm test` YAPMA (tam frontend suite'i uzun sürer — kural gereği koşulmaz; yalnızca mevcut `assessmentOrganizationUi.test.ts` gibi doğrudan etkilenen bir test varsa ve kısa sürüyorsa çalıştırıp sonucunu bildir)

Çalıştırdığın her komutun süresini raporla.

## Rapor (yanıtının sonunda)

- Değiştirilen/oluşturulan dosya listesi (kısa özet + her dosyada ne yapıldığı)
- Eklenen testlerin listesi ve çalışan komut çıktılarının özeti (PASS/FAIL sayıları)
- Kapsam dışı bırakılanlar / açık sorular / sonraki faz için notlar
- Çalışan komutlardan herhangi biri başarısızsa: hata + nasıl çözüldüğü (çözülemediyse net biçimde belirt, gizleme)
