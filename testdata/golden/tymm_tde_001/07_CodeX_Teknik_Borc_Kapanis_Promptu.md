VS Code context’e ekle:

- @AGENTS.md
- @docs/CURRENT_TECHNICAL_DEBT_AUDIT.md
- @docs/FINAL_SECURITY_RELEASE_AUDIT.md
- @docs/FINAL_PRE_USE_DATA_LOSS_AUDIT.md
- @docs/PROJECT_MAP.md
- @docs/FILE_OWNERSHIP_MAP.md
- @docs/API_CONTRACTS.md
- @docs/FEATURE_FLOW_MAP.md
- @docs/UYGULAMA_PLANI.md
- @docs/TYMM_PERFORMANCE_PLAN.md

- @testdata/golden/tymm_tde_001/README.md
- @testdata/golden/tymm_tde_001/05_Rubrik_Golden.json
- @testdata/golden/tymm_tde_001/06_Golden_Set_Beklentileri.json
- @testdata/golden/tymm_tde_001/01_Bos_Sinav_Kagidi.pdf
- @testdata/golden/tymm_tde_001/02_Doldurulmus_Ornek_Kagit.pdf
- @testdata/golden/tymm_tde_001/03_Doldurulmus_Tarama_Varyanti.pdf
- @testdata/golden/tymm_tde_001/04_Cevap_Anahtari_ve_Rubrik.pdf

- @src-tauri/src/domain
- @src-tauri/src/services
- @src-tauri/src/commands
- @src-tauri/src/jobs
- @src-tauri/src/platform
- @src-tauri/src/diagnostics.rs
- @src-tauri/src/lib.rs
- @src-tauri/src/bin
- @src-tauri/tests
- @src-tauri/Cargo.toml

- @src/api
- @src/app
- @src/components
- @src/pages
- @src/state
- @src/utils
- @src
- @package.json
- @tsconfig.app.json

Proje kökü:

/Users/kadir/Desktop/RubriKa/RubrikaV3

Golden set:

/Users/kadir/Desktop/RubriKa/RubrikaV3/testdata/golden/tymm_tde_001

# Görev

RubrikaV3’te açık kalan bütün doğrulanmış teknik borçları, yeni TYMM performans özelliğini ve sentetik golden sınav paketini birlikte ele alarak aşamalı biçimde kapat.

Görev adı:

**Final Technical Debt Closure — Activity Scope, OCR Golden Pipeline, Model Efficiency, Scoring Calibration and Modular Boundaries**

Bu görev büyük fakat tek bir kontrolsüz refactor değildir. Aşamalar sırayla yürütülecek, her aşama bağımsız doğrulanacak ve bir aşama yeşil olmadan sonraki aşamaya geçilmeyecektir.

Kod değişikliği yapmaya ve versioned migration kodunu geliştirmeye açık onay verilmiştir. Ancak hiçbir gerçek kullanıcı projesinde migration, repair, cleanup veya write çalıştırma. Migration yalnız tempdir ve committed test fixture’larında doğrulanmalıdır.

Git commit oluşturma. Kullanıcıya ait değişiklikleri silme, stash yapma veya geri alma.

# 0. Başlangıç doğrulaması ve borç matrisi

Önce mevcut çalışma ağacını doğrula:

```bash
git status --short
git branch --show-current
git rev-parse HEAD
git log --oneline -8
git diff --stat
git diff --check
git stash list
git ls-files .audit_cache
```

`RubrikaV3_Oturum_Raporu.md` veya önceki ajan raporlarına körü körüne güvenme. `CURRENT_TECHNICAL_DEBT_AUDIT.md` içindeki TD-01–TD-39 maddelerini güncel production kodunda yeniden sınıflandır:

- CONFIRMED
- ALREADY_FIXED
- PARTIAL
- NOT_FOUND
- NEEDS_RUNTIME_PROOF

Özellikle önceki kapanış raporunda eksik belgelenen TD-05, TD-06, TD-07, TD-08, TD-09, TD-12 ve TD-17’yi doğrudan kod ve testle doğrula.

Başlangıçta `docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` içinde kabul matrisi oluştur; finalde güncelle.

# 1. Önce performans değerlendirme veri güvenliği açıklarını eksiksiz kapat

Aşağıdakilerin her biri production kodu + kırmızı regresyon testi + yeşil sonuçla kapanmalıdır:

## 1.1 Onaylı karar değişmezliği

- `set_performance_assessment_status` assessment_id verilse de verilmese de Approved kaydı değiştiremez.
- Approved kaydın ratings, feedback, status, rubric id/version, approval metadata’sı hiçbir genel save/status komutuyla değişemez.
- Değişiklik gerekiyorsa ayrı typed “new revision/reopen” state-machine işlemi olmalı; sessiz reopen yok.

## 1.2 Kimlik ve scope çapraz doğrulaması

- assessment_id bu ClassApplication’a ait olmalı.
- assessment.student_id input student_id ile eşleşmeli.
- activity, class application, student, task ve rubric version birbirinin scope’u içinde olmalı.
- Yabancı ID yeni duplicate kayıt oluşturmamalı; typed error dönmeli.
- Aynı student + performance task + class application için en fazla bir aktif/final değerlendirme olmalı.

## 1.3 Delete dependency

- Performance assessment bulunan ClassApplication silinemez.
- Approved değerlendirme, kullanılan rubrik sürümü veya task dependency scan olmadan silinemez.
- Silme command katmanında değil, service/transaction sınırında yeniden doğrulanmalı.

## 1.4 Rubrik sürümü sabitleme

- Yeni kayıt en son yayımlanmış sürümü pinler.
- Var olan InProgress kayıt kendi rubric_id/version değerinde kalır.
- Yeni rubrik yayımlanması taslak puanları sessizce yeniden hesaplamaz.
- Rubrik değişimi yalnız açık “yeni sürüme geçir” işlemiyle yapılabilir ve eski state audit’te korunur.

## 1.5 Provisional/final rapor ayrımı

- InProgress toplam final toplam değildir.
- CSV/XLSX/PDF’de yalnız Approved satır final puan taşır.
- Provisional değer gerekiyorsa ayrı başlık ve açık etiketle gösterilir.
- Missing, NotPerformed ve gerçek Score(0) bütün katmanlarda ayrı kalır.

## 1.6 CSV/XLSX güvenliği

- `=`, `+`, `-`, `@`, tab ve CR/LF ile başlayan kullanıcı kontrollü hücreler formül olarak çalışamaz.
- XLSX hücreleri string tipinde tutulur.
- Türkçe karakterler ve delimiter davranışı test edilir.

## 1.7 Frontend mutation/draft güvenliği

- Save pending iken approve/status/revert/publish devre dışı.
- Refetch başarısız veya başarılı save sırasında daha yeni local draft’ı ezemez.
- Stale response yeni state’i overwrite edemez.
- Duplicate click tek mutation üretir.
- Backend commit gelmeden success gösterilmez.

## 1.8 Legacy scoring güvenli default

- `scoring_applied` eksikliği fail-closed davranmalı.
- Default false olmalı.
- Eski kayıt semantiği versioned normalization/migration ile açıkça sınıflandırılmalı.
- Eski alan yok diye kayıt accepted/final sayılmamalı.

## 1.9 Teacher-facing teknik sızıntı

- Raw UUID, enum adı, blocker code veya JSON teacher UI’da görünmez.
- Eksik label genel ama açıklayıcı Türkçe mesaja düşer.

# 2. Son oturumdaki TD-15 kapanışını gerçek semantik olarak doğrula

`commit_snapshot_cas` veya kritik ProjectStore commit’i başarısız olduğunda yalnız audit/log yazmak yeterli değildir.

Zorunlu davranış:

```text
commit fail
→ typed error
→ command success dönmez
→ UI “kaydedildi” göstermez
→ memory state canonical sayılmaz
→ retry mümkündür
```

Speaking, performance, OCR ve scoring kritik mutation yollarında regresyon testleri ekle.

# 3. TD-01 — Yazılı sınav verilerini gerçek AssessmentActivity kapsamına taşı

Bu görevde yalnız geçici “tek yazılı sınav guard” ile yetinme. Versioned, backup-gated activity-scope migration kodunu tamamla.

## 3.1 Canonical scope

Aşağıdaki entity’ler açık `assessment_activity_id` taşımaya veya activity-owned koleksiyon altında saklanmaya başlamalı:

- Question
- StudentSubmission
- QuestionText generation
- Rubric/rubric version/frozen QEP
- StudentAnswerOcrGeneration ve active projection
- ScoringRun/ScoringRecord
- ExamPackageFreeze
- İlgili analysis/export referansları

Her kayıt ayrıca gerekli yerde `class_application_id` ve öğrenci/submission scope’unu korumalı.

## 3.2 Migration

- Yeni schema sürümü ekle.
- Migration başlamadan verified backup sözleşmesi korunmalı.
- Eski flat proje tek bir written activity içeriyorsa deterministic olarak ona bağlanmalı.
- Hiç written activity yok ama legacy written data varsa sentetik/göç activity’si yalnız açık, belgeli policy ile oluşturulabilir.
- Birden çok written activity varken flat legacy verinin hangi activity’ye ait olduğu belirsizse otomatik tahmin yapma; typed `MigrationAmbiguousAssessmentScope` blocker üret.
- İkinci migration no-op olmalı.
- Unknown alanlar kaybolmamalı.
- Gerçek kullanıcı projesinde migration çalıştırma.

## 3.3 Bütün servisleri activity-scoped yap

Question/rubric/import/OCR/scoring/freeze/workflow/export command’leri assessment_activity_id olmadan çalışamamalı veya mevcut route/context’ten backend-authoritative biçimde çözmelidir.

Frontend ham proje-level listelerden assessment verisi türetmemeli.

## 3.4 Zorunlu fixtures

- Eski tek written sınav projesi → migration → semantik equality.
- Aynı projede iki yeni written activity → soru/OCR/scoring izolasyonu.
- Aynı öğrenci numarası farklı sınıflarda → karışma yok.
- Bir activity silme isteği dependency nedeniyle bloklanır.
- Activity A’nın frozen QEP’i Activity B scoring’inde kullanılamaz.

# 4. Workflow ve readiness tek otoritesini finalleştir

Tur 2 değişikliklerini yeniden doğrula ve eksik kalanları kapat:

- Persisted `project.workflow` authoritative olmamalı; yalnız cache/diagnostic ise açıkça işaretlenmeli.
- Her workflow sonucu canlı canonical state’ten hesaplanmalı.
- Performance, Written, Speaking ve Listening aynı backend snapshot sözleşmesini kullanmalı.
- Frontend `derive*Statuses` domain kararı üretmemeli.
- Scoring readiness gerçek `(submission_id, question_id)` kümesi üzerinden duplicate/missing kontrolü yapmalı.
- Boş öğrenci/soru listesi vacuous `.all()` ile hazır sayılamaz.

# 5. Golden sınav paketini committed test corpus’a dönüştür

`testdata/golden/tymm_tde_001` içeriğini test kaynağı olarak kullan.

## 5.1 Dosya bütünlüğü

- Golden dosyalarının SHA-256 manifestini testte doğrula.
- PDF’ler production test sırasında değiştirilmemeli.
- Test output’ları tempdir’e yazılmalı.

## 5.2 Sınav yapısı

- Blank exam: soru text extraction ve rubrik extraction kaynağı.
- Filled vector exam: temiz OCR baseline.
- Scanned variant: skew/contrast/raster OCR testi.
- Q1 iki sayfalı ve iki bölgeli cevap olarak işlenmeli.
- Q2 table, Q3 matching, Q4 correction table, Q5 grammar analysis, Q6 open-ended schema ile çözülmeli.

## 5.3 OCR kalite metrikleri

`06_Golden_Set_Beklentileri.json` içindeki gerçek ground truth’a göre ölç:

- CER
- WER
- critical-token error
- printed-question leakage
- structured-field exact match
- p50/p95 süre
- image token sayısı
- model call sayısı
- retry sayısı
- peak memory mümkünse

Model binary veya model dosyası test ortamında yoksa:

- preprocess/crop/registration ve parser testlerini çalıştır,
- model benchmark’ını `NEEDS_MODEL_RUNTIME` olarak ayrı raporla,
- PASS uydurma.

# 6. OCR görüntü hattını tamamla

Golden set kullanılarak önce baseline kaydet, sonra aşağıdaki değişiklikleri ölçüm kapısıyla uygula:

## 6.1 OCR özel render

- UI preview ile OCR source render ayrılmalı.
- OCR için 300 DPI başlangıç profili ekle; 400 DPI yalnız küçük stroke/karakter ölçümünde seçici olsun.
- Render DPI provenance’ı kaydedilsin.

## 6.2 Registration / deskew / perspective

- Page boundary ve skew tespiti.
- En az deskew + translation/scale registration.
- Perspective correction yalnız güvenilir anchor bulunduğunda.
- Transform provenance ve confidence kaydedilsin.
- Registration başarısızsa sessiz yanlış crop yerine review/fallback.

## 6.3 Multi-region / multi-page crop

- `regions[]` page/order/bbox/role taşımalı.
- Q1 continuation doğru sırayla birleştirilmeli.
- `page_count > 1` otomatik partial anlamına gelmemeli.

## 6.4 Adaptive preprocess

- Beş varyantı eager üretme.
- Quality profile’a göre tek başlangıç varyantı.
- Düşük confidence, ink/empty mismatch, clipping, schema failure veya length finish’te ikinci varyantı lazy üret.
- İkinci sonuç ilk teacher-approved sonucu otomatik ezmemeli.

## 6.5 Content-addressed image cache

Cache key en az:

```text
source hash
+ render DPI
+ registration transform/version
+ region list/order
+ preprocess policy/version
+ resize/image-token policy
+ encoder quality/version
```

Cache hit gerçek byte reuse sağlamalı; mevcut dosyayı silip yeniden üretme.

## 6.6 Typed structuredAnswer

Her answer type için Rust enum/struct ve schema doğrulaması:

- OpenText
- Table
- Matching
- CorrectionTable
- GrammarAnalysis
- Numeric/ShortText mevcutsa

Geçersiz schema `needsReview=true`, `scoringApplied=false` üretmeli.

# 7. Soru/rubrik model çağrısı verimliliğini tamamla

- Her soru için tüm sayfaları tekrar gönderme.
- Question-to-page/region map oluştur.
- İlk çağrı hedef sayfa/region; düşük confidence/not_visible durumunda ±1 pencere; son çare geniş fallback.
- Rubrik parse retry’de görselleri yeniden göndermeden deterministic salvage veya text-only JSON repair kullan.
- Full multimodal retry yalnız açık retry reason ile.
- Model çağrısı, image token ve prefill metriklerini diagnostics’e yaz.

# 8. Scoring fingerprint, calibration ve anchor borcunu tamamla

Mevcut deterministic scoring/cache’i koru ve eksik provenance alanlarını gerçek değerlerle doldur:

- model checksum
- llama/runtime version
- prompt version
- schema version
- policy version
- sampling
- OCR generation hash
- frozen rubric/QEP hash
- calibration version
- anchor set hash

`"none"` sabit placeholder kullanma; politika yoksa typed `unconfigured:v1` gibi versioned değer kullan.

Exact duplicate cache yalnız aynı tam fingerprint’te çalışmalı.

Teacher-approved anchor altyapısı eklenirse:

- aynı assessment + question + criterion scope’u dışına taşamaz,
- yakın cevap puanı kör kopyalanamaz,
- negation/sayı/birim/kritik kavram farkı consistency review üretir,
- anchor değişikliği cache invalidation yapar.

Golden doldurulmuş kâğıdın beklenen puanı 80/100’dür. Deterministik bölümler exact olmalı; semantik kriterlerde öğretmen-approved golden karar ile karşılaştır.

# 9. Command/error/job ve provenance kapanışını doğrula

Tur 4’ün uygulamasını production call graph üzerinde denetle:

- `isAppError` runtime validation bütün command/event sınırlarında.
- Job rehydrate tek startup noktası, typed error ve görünür warning.
- Production unwrap/expect/panic yok veya açık safety proof’u var.
- OCR readers yalnız canonical active resolver’dan.
- Tek merkezi job store/query.
- Command → job → model → mutation → audit tek correlation ID.
- PromptContract None production’da fail-closed.

Eksik call-site varsa tamamla ve negative repository scan testi ekle.

# 10. DTO/read-model ve servis sınırları

Büyük-bang rewrite yapmadan sürdürülebilir kapanış uygula:

## 10.1 API DTO ayrımı

- `src/api/types.ts` domainlere göre bölünsün: project, assessment, performance, OCR, scoring, speaking, jobs, errors.
- Barrel export ile çağrıcı uyumluluğu korunabilir.
- Rust–TS drift için en az committed schema snapshot/contract test ekle.

## 10.2 Project read model

- `get_project_snapshot` ham persisted Project yerine frontend’in ihtiyaç duyduğu versioned read DTO döndürsün veya yeni additive command ekle.
- Frontend persistence internallerine bağımlı olmamalı.
- Eski command hemen kaldırılamıyorsa deprecated ve yalnız migration/diagnostic amaçlı tut.

## 10.3 Servis modülerleşmesi

Yalnız davranış-koruyan extraction yap:

- performance_service → rubric_versioning / assessment_evaluation / reporting facade
- llama gateway → health/runtime request builder/response parser modülleri
- diagnostics → preflight/audit/model/job alt modülleri
- project_store → migration/atomic persistence/normalization alt modülleri

Her extraction sonrası test çalıştır; aynı anda davranış değiştirme.

# 11. Test ve doğrulama kapıları

Her fazda hedefli test. Finalde tamamı:

```bash
npm run typecheck
npm run lint
npm test -- --run
npm run build

cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo clippy --manifest-path src-tauri/Cargo.toml \
  --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored --nocapture

npm run check:all
npm run tauri:dev -- --smoke
npm run tauri:build
git diff --check
```

Port doluysa kullanıcı süreçlerini izinsiz öldürme. Kullanılabilir farklı port/isolated smoke yöntemi varsa repo sözleşmesine uygun biçimde kullan; yoksa BLOCKED raporla, PASS sayma.

Her komut için command, exit code, passed, failed, ignored, filtered, elapsed ve hata sınıfını raporla.

# 12. Repo hijyeni

- `fdb8e6e` commit’ini ve `.audit_cache` tracked durumunu incele.
- Log/cache dosyalarını production source commit’inde tutma; ancak kullanıcı onayı olmadan geçmiş commit rewrite etme.
- WIP stash’i benzersiz değişiklik açısından incele; kullanıcı onayı olmadan silme.
- Yeni git commit oluşturma.

# 13. Dokümantasyon

Güncelle:

- docs/CURRENT_TECHNICAL_DEBT_AUDIT.md
- docs/PROJECT_MAP.md
- docs/FILE_OWNERSHIP_MAP.md
- docs/API_CONTRACTS.md
- docs/FEATURE_FLOW_MAP.md
- docs/UYGULAMA_PLANI.md

Yeni rapor:

- docs/FINAL_TECHNICAL_DEBT_CLOSURE.md
- docs/GOLDEN_OCR_SCORING_BENCHMARK.md
- docs/ASSESSMENT_ACTIVITY_SCOPE_MIGRATION.md

# 14. Kabul kriterleri

Görev ancak şu şartlarla tamamlanır:

- TD-01–TD-39 güncel durum matrisi var.
- Bütün CONFIRMED P0/P1’ler kapalı veya açıkça benchmark-gated kalite işi olarak yeniden sınıflandırılmış.
- Onaylı performance karar hiçbir generic command ile değiştirilemiyor.
- ClassApplication performance verisiyle silinemiyor.
- Yabancı assessment ID duplicate oluşturmuyor.
- Rubrik re-pin sessiz puan değişimi yapmıyor.
- Provisional puan final export’a girmiyor.
- CSV/XLSX formül enjeksiyonu kapalı.
- Frontend mutation yarışı/draft kaybı testi geçiyor.
- Legacy scoring fail-closed.
- Written/OCR/scoring/QEP gerçek activity scope’lu.
- Legacy migration idempotent ve ambiguity’de fail-closed.
- Golden sınav multi-page/multi-region crop testi geçiyor.
- Typed structuredAnswer bütün örnek türlerinde doğrulanıyor.
- Golden OCR metrikleri raporlanıyor; model yoksa dürüstçe BLOCKED.
- Soru/rubrik çağrılarında hedef sayfa penceresi var.
- Rubrik parse retry gereksiz tam multimodal tekrar yapmıyor.
- Scoring fingerprint gerçek calibration/anchor sürümünü içeriyor.
- Command/job/model/mutation/audit correlation zinciri tam.
- Full Cargo suite 0 fail.
- All-target/all-feature Clippy 0 warning.
- Frontend suite 0 fail.
- check:all exit 0.
- Smoke exit 0 veya dürüst environment blocker.
- `.app` ve DMG üretiliyor.

# Teslim raporu

Son cevapta:

1. Başlangıç snapshot
2. TD-01–TD-39 kapanış matrisi
3. Performance veri güvenliği kapanışı
4. AssessmentActivity migration
5. Workflow/readiness
6. Golden sınav corpus’u
7. OCR baseline ve yeni sonuçlar
8. Multi-page/multi-region sonucu
9. Soru/rubrik model maliyeti
10. Scoring fingerprint/cache/calibration
11. Job/error/correlation
12. DTO/read-model/modülerleşme
13. Değiştirilen dosyalar
14. Migration testleri
15. Frontend testleri
16. Rust testleri
17. Ignored testler
18. check:all
19. Smoke
20. Release build
21. Kalan benchmark-gated işler
22. Repo hijyeni
23. Nihai karar

başlıklarını kullan.

Son satır yalnız şunlardan biri:

```text
Teknik borç kapanış kararı: COMPLETE
```

```text
Teknik borç kapanış kararı: COMPLETE_WITH_BENCHMARK_BLOCKERS
```

```text
Teknik borç kapanış kararı: INCOMPLETE
```
