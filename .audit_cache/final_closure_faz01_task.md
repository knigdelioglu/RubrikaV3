# Final Technical Debt Closure — FAZ 0+1 (Başlangıç Doğrulaması + Performans Veri Güvenliği Kapanışı)

## Kapsam

Proje kökü: `/Users/kadir/Desktop/RubriKa/RubrikaV3`

Bu görev, "**Final Technical Debt Closure — Activity Scope, OCR Golden Pipeline, Model Efficiency, Scoring Calibration and Modular Boundaries**" kampanyasının ilk iki aşamasıdır:

- **FAZ 0**: Başlangıç doğrulaması + TD-01–TD-39 borç matrisinin güncel koda göre yeniden sınıflandırılması.
- **FAZ 1**: Performans değerlendirme veri güvenliği açıklarının (1.1–1.9) eksiksiz kapanışı.

Sonraki aşamalar (TD-15 semantik doğrulama, AssessmentActivity scope migration, workflow otoritesi, golden OCR pipeline, model verimliliği, scoring kalibrasyonu, DTO/modüler sınırlar, tam test kapıları, dokümantasyon) **ayrı görev dosyalarıyla** gelecektir — bu fazda onları uygulama.

**Yetki ve yasaklar:**
- Production kodunda değişiklik yapmaya ve versioned migration kodu geliştirmeye açık onay verilmiştir.
- Hiçbir gerçek kullanıcı projesinde migration, repair, cleanup veya write çalıştırma. Migration yalnız tempdir ve committed test fixture'larında doğrulanır.
- Git commit oluşturma. Kullanıcıya ait değişiklikleri silme, stash yapma veya geri alma.
- Çalışma ağacında **25 değiştirilmiş dosya** (önceki turların kullanıcı işi), `testdata/` (golden set) ve `.audit_cache/tur4b_task.md` (untracked) bulunmaktadır. Bunlara DOKUNMA, geri alma, üzerine yazma — bu ağacın ÜZERİNDE çalış.
- `git stash list`'te `stash@{0}: On main: tur0+tur1 WIP (performans regresyon testleri)` vardır. Stash'a dokunma, pop/drop/apply yapma; yalnızca ileride rapor için varlığını not et.

## Bağlam dosyaları

Aşağıdakileri oku ve görev boyunca referans olarak kullan (önemli olanlar `-f` ile bu çalışmaya eklenmiştir; dizinleri ve PDF'leri doğrudan diskten oku):

- `AGENTS.md` (mühendislik standartları — uy)
- `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` (TD-01–TD-39 kaynağı)
- `docs/FINAL_SECURITY_RELEASE_AUDIT.md`, `docs/FINAL_PRE_USE_DATA_LOSS_AUDIT.md`
- `docs/PROJECT_MAP.md`, `docs/FILE_OWNERSHIP_MAP.md`, `docs/API_CONTRACTS.md`, `docs/FEATURE_FLOW_MAP.md`, `docs/UYGULAMA_PLANI.md`, `docs/TYMM_PERFORMANCE_PLAN.md`
- `testdata/golden/tymm_tde_001/README.md`, `05_Rubrik_Golden.json`, `06_Golden_Set_Beklentileri.json` (bu fazda yalnız referans; golden pipeline Faz 5+)
- `src-tauri/src/domain`, `src-tauri/src/services`, `src-tauri/src/commands`, `src-tauri/src/jobs`, `src-tauri/src/platform`, `src-tauri/src/diagnostics.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/bin`, `src-tauri/tests`, `src-tauri/Cargo.toml`
- `src/api`, `src/app`, `src/components`, `src/pages`, `src/state`, `src/utils`, `src/` kök dosyaları, `package.json`, `tsconfig.app.json`

KURAL: Explore/keşif ajanları kullanma, dosyaları doğrudan oku. Plan sunma, onay isteme, soru sorma — doğrudan kod uygulamasına geç. Görev sonunda görev dosyasındaki STATUS formatında rapor ver.

---

## 0. Başlangıç doğrulaması ve borç matrisi

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

Çıktıları olduğu gibi raporuna yaz. `RubrikaV3_Oturum_Raporu.md` veya önceki ajan raporlarına körü körüne güvenme. `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` içindeki TD-01–TD-39 maddelerini **güncel production kodunda** yeniden sınıflandır:

- CONFIRMED
- ALREADY_FIXED
- PARTIAL
- NOT_FOUND
- NEEDS_RUNTIME_PROOF

Özellikle önceki kapanış raporunda eksik belgelenen **TD-05, TD-06, TD-07, TD-08, TD-09, TD-12 ve TD-17**'yi doğrudan kod ve testle doğrula (bunlar aynı zamanda Faz 1'in maddeleridir; kod+test kanıtı burada üretilir).

Başlangıçta `docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` içinde **kabul matrisi** oluştur (tüm TD-01–TD-39 için sütunlar: ID, Öncelik, Başlık, Yeni Durum, Kanıt/Kaynak, Kapanış Notu); bu fazın sonunda matrisi güncelle. Bu dosya kampanya boyunca tek otorite olacaktır.

## 1. Performans değerlendirme veri güvenliği açıklarını eksiksiz kapat

Aşağıdakilerin her biri **production kodu + kırmızı regresyon testi + yeşil sonuç** ile kapanmalıdır. Her madde için önce testi yaz ve FAIL olduğunu gör (kırmızı), sonra production kodunu düzelt ve yeşile getir. Kırmızı-kanıtını (fail çıktısı) ve yeşil-kanıtını raporunda ayrı göster.

### 1.1 Onaylı karar değişmezliği

- `set_performance_assessment_status` assessment_id verilse de verilmese de Approved kaydı değiştiremez.
- Approved kaydın ratings, feedback, status, rubric id/version, approval metadata'sı hiçbir genel save/status komutuyla değişemez.
- Değişiklik gerekiyorsa ayrı typed "new revision/reopen" state-machine işlemi olmalı; sessiz reopen yok.

### 1.2 Kimlik ve scope çapraz doğrulaması

- assessment_id bu ClassApplication'a ait olmalı.
- assessment.student_id input student_id ile eşleşmeli.
- activity, class application, student, task ve rubric version birbirinin scope'u içinde olmalı.
- Yabancı ID yeni duplicate kayıt oluşturmamalı; typed error dönmeli.
- Aynı student + performance task + class application için en fazla bir aktif/final değerlendirme olmalı.

### 1.3 Delete dependency

- Performance assessment bulunan ClassApplication silinemez.
- Approved değerlendirme, kullanılan rubrik sürümü veya task dependency scan olmadan silinemez.
- Silme command katmanında değil, service/transaction sınırında yeniden doğrulanmalı.

### 1.4 Rubrik sürümü sabitleme

- Yeni kayıt en son yayımlanmış sürümü pinler.
- Var olan InProgress kayıt kendi rubric_id/version değerinde kalır.
- Yeni rubrik yayımlanması taslak puanları sessizce yeniden hesaplamaz.
- Rubrik değişimi yalnız açık "yeni sürüme geçir" işlemiyle yapılabilir ve eski state audit'te korunur.

### 1.5 Provisional/final rapor ayrımı

- InProgress toplam final toplam değildir.
- CSV/XLSX/PDF'de yalnız Approved satır final puan taşır.
- Provisional değer gerekiyorsa ayrı başlık ve açık etiketle gösterilir.
- Missing, NotPerformed ve gerçek Score(0) bütün katmanlarda ayrı kalır.

### 1.6 CSV/XLSX güvenliği

- `=`, `+`, `-`, `@`, tab ve CR/LF ile başlayan kullanıcı kontrollü hücreler formül olarak çalışamaz.
- XLSX hücreleri string tipinde tutulur.
- Türkçe karakterler ve delimiter davranışı test edilir.

### 1.7 Frontend mutation/draft güvenliği

- Save pending iken approve/status/revert/publish devre dışı.
- Refetch başarısız veya başarılı save sırasında daha yeni local draft'ı ezemez.
- Stale response yeni state'i overwrite edemez.
- Duplicate click tek mutation üretir.
- Backend commit gelmeden success gösterilmez.

### 1.8 Legacy scoring güvenli default

- `scoring_applied` eksikliği fail-closed davranmalı.
- Default false olmalı.
- Eski kayıt semantiği versioned normalization/migration ile açıkça sınıflandırılmalı.
- Eski alan yok diye kayıt accepted/final sayılmamalı.

### 1.9 Teacher-facing teknik sızıntı

- Raw UUID, enum adı, blocker code veya JSON teacher UI'da görünmez.
- Eksik label genel ama açıklayıcı Türkçe mesaja düşer.

**Önemli not:** 1.5 ve 1.8 yazılı sınav rapor/export katmanlarına da dokunabilir (rapor ayrımı, `scoring_applied` serde default). Bu kapsam dahilindedir; ancak yazılı scoring akışının QEP frozen gate'i ve teacher approval sözleşmesi **zayıflatılamaz**.

---

## ÇALIŞMA SÖZLEŞMESİ

- Önce mevcut projeyi ve ilgili dosyaları incele.
- Görev kapsamı dışındaki dosyaları değiştirme.
- Mevcut kullanıcı değişikliklerini silme veya geri alma.
- `git reset`, `git clean`, `git checkout --`, `git restore`, force push, rebase veya geçmiş değiştiren Git komutlarını kullanma.
- Hiçbir koşulda Git commit, branch, tag veya pull request oluşturma — değişiklik ne kadar büyük olursa olsun, kullanıcı onayı olsa bile.
- Kullanıcı açıkça istemedikçe bağımlılık sürümlerini topluca yükseltme.
- Kullanıcı açıkça istemedikçe dosya silme.
- Gizli anahtarları, tokenleri, kullanıcı verilerini veya proje içeriğini dış servislere gönderme.
- Gereksiz biçimlendirme ve kapsam dışı refactor yapma.
- Uygulamadan önce ilgili mimariyi ve mevcut davranışı doğrula.
- Değişiklikleri küçük ve denetlenebilir tut.
- Çalıştırılan testler başarısız olursa saklama; hata mesajlarını kısa ve doğru biçimde raporla.
- Çalışma sonunda yalnızca aşağıdaki formatta sonuç ver:

```text
STATUS: COMPLETED | BLOCKED | APPROVAL_REQUIRED | FAILED
SUMMARY: En fazla 10 satırlık sonuç özeti
CHANGED_FILES: Değiştirilen dosya yolları
VALIDATION: Çalıştırılan testler ve sonuçları (exit code + passed/failed + süre)
RISKS: Kalan riskler veya "none"
NEXT_ACTION: Gerekli sonraki işlem veya "none"
```

Onay gerektiren, geri döndürülemez, kapsamı genişleten ya da güvenlik açısından riskli bir işlemle karşılaşırsan işlemi gerçekleştirme. Şu formatta çıkış yap:

```text
STATUS: APPROVAL_REQUIRED
APPROVAL_REQUEST: Yapılmak istenen işlem
REASON: Neden gerekli olduğu
IMPACT: Hangi dosya, veri veya sistemi etkileyeceği
ALTERNATIVES: Varsa daha güvenli seçenekler
```

## Doğrulama (bu fazın kapsamı — AGENTS.md seviye D)

Kampanyanın tam kapı listesi (build, check:all, smoke, full cargo test, tauri:build, .app/DMG) **Faz 11'de** çalıştırılacaktır; bu fazda çalıştırma. Bu fazda yalnız şunlar:

- `cargo fmt --manifest-path src-tauri/Cargo.toml --check` (bozuksa yalnız değiştirdiğin dosyaları formatla)
- Hedefli Rust testleri: `cargo test --manifest-path src-tauri/Cargo.toml performance` ve `cargo test --manifest-path src-tauri/Cargo.toml assessment_organization` (yeni yazdığın regresyon testleri dahil; kırmızı→yeşil kanıtlarıyla)
- `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` — değiştirdiğin dosyalarla ilgili hataları düzelt; mevcut bilinen 5 test-kodu hatası (`deterministic_scoring_service.rs:888`, `scoring_anchor_service.rs:626/636/647`, `scoring_cache_service.rs:392`) kapsam dışıdır, raporla ama düzeltme.
- `npm run typecheck`
- `npm run lint` (yalnız değişen dosyalara ait hatalar)
- `npm test -- --run` — yalnız etkilenen frontend testleri (performanceScoringUi / performanceReportUi / examWorkspace ile ilgili olanlar; 1.7 için yazacağın yeni component testleri dahil)
- `git diff --check`

Çalıştırdığın **her komutun süresini** raporla (komut, exit code, passed/failed/ignored, elapsed). `npm run check:all` ve tam `cargo test` ÇALIŞTIRMA — Faz 11'e ait.

## Kabul kriterleri (bu faz için)

- TD-01–TD-39 güncel durum matrisi `docs/FINAL_TECHNICAL_DEBT_CLOSURE.md`'de var.
- Onaylı performance karar hiçbir generic command ile değiştirilemiyor (1.1).
- Yabancı assessment ID duplicate oluşturmuyor; tek aktif/final kayıt garantisi var (1.2).
- ClassApplication performance verisiyle silinemiyor (1.3).
- Rubrik re-pin sessiz puan değişimi yapmıyor (1.4).
- Provisional puan final export'a girmiyor (1.5).
- CSV/XLSX formül enjeksiyonu kapalı (1.6).
- Frontend mutation yarışı/draft kaybı testi geçiyor (1.7).
- Legacy scoring fail-closed; `scoring_applied` default false (1.8).
- Teacher UI'da raw UUID/enum/blocker code yok (1.9).
- Her kapanışın kırmızı regresyon kanıtı ve yeşil sonucu raporda var.
- Yeni git commit yok; kullanıcı değişikliklerine dokunulmamış.
