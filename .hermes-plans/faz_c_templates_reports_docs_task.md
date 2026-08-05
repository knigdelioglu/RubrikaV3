# Faz C — TYMM Performans Değerlendirme: TDE Şablonları + Rapor + Dokümanlar

## Amaç
Faz A (backend) ve Faz B (frontend) üzerine: yerleşik TDE rubrik şablonları, PDF/Excel performans raporu ve dokümantasyon güncellemeleri. UI smoke kullanıcı tarafından yapılacaktır.

## Kapsam (yalnız bunlar)

### 1. Yerleşik TDE rubrik şablonları
- `PerformanceOrganizationPage`'teki `RubricDraftEditor` + `freshRubric()` noktasına **initialRubric olarak beslenen** şablon kataloğu ekle (Faz B raporundaki entegrasyon noktası).
- Kapsam: 9. sınıf pilot; iki öğrenme alanı için ölçüt setleri — **Metin Tahlili** (dinleme/izleme-okuma) ve **Edebiyat Atölyesi** (konuşma-yazma).
- Ölçüt adlarını ve pedagojik çerçeveyi `docs/TYMM_PERFORMANCE_SCALE_REPORT.md` ve `docs/UYGULAMA_PLANI.md`'den al (konuşma 8 ölçüt, yazma 8 ölçüt); metin tahlili için okuma/dinleme odaklı ölçütler (metin türünü tanıma, ana düşünce, yapı, dil-anlatım, eleştirel bakış vb.).
- Her şablon: 4 ölçüt (3-6 aralığında), 4 düzey (3-5 aralığında) veya 3/5 — backend doğrulamasına uygun (3-6 ölçüt, 3/5 düzey), her düzey için gözlenebilir tanım metni.
- Kullanıcı seçtiğinde şablon, rubrik taslağına (sürüm 0) yüklenir ve öğretmen düzenleyebilir; şablonlar salt-okunur katalogdur.

### 2. PDF rapor
- Mevcut PDF üretim desenini (`pdf_service` / raporlama altyapısı) izleyerek performans sonuç raporu: öğrenci adı, sınıf, görev başlığı, ölçüt bazında puanlar, toplam, **Missing/NotPerformed ayrı gösterimi** (boş hücre/etiket; sıfırla karıştırma), öğretmen adı, tarih.
- `PerformanceResultsView`'dan (Faz B) rapor komutuna/butonuna bağla; mevcut yazılı sınav rapor akışını bozma.

### 3. Excel rapor
- Mevcut Excel/CSV çıktı desenine uygun sınıf düzeyi sonuç tablosu: öğrenci satırları × ölçüt puanları, toplam, durum (Missing/NotPerformed/puanlandı).
- `PerformanceResultsView`'dan bağla.

### 4. Doküman güncellemeleri (Faz B raporundaki liste)
- `docs/API_CONTRACTS.md`, `docs/FILE_OWNERSHIP_MAP.md`, `docs/SYMBOL_MAP.md`, `docs/FEATURE_FLOW_MAP.md`, `docs/ASSESSMENT_ORGANIZATION.md` — performans değerlendirme akışını (tip, komutlar, ekranlar, raporlar) işleyecek şekilde güncelle.

## Kısıtlar (değişmez kurallar — ihlal etme)
1. `ScoringRecord` ve yazılı sınav puanlama/rapor akışına DOKUNMA.
2. `Missing` ≠ `NotPerformed` ≠ sıfır; raporda ayrı gösterim korunur.
3. AI/otomasyon puan vermez.
4. Türkçe arayüz ve doküman metinleri; mevcut kod stil desenine uy.
5. Şablonlar öğretmen kararını kısıtlamaz: düzenlenebilir başlangıç noktasıdır.

## Test — YOK (kullanıcı kuralı)
Kullanıcı politikası: testler yalnızca büyük işlerde (mimari değişiklik/büyük refactor). Faz C küçük/orta ölçekli iştir → **yeni test yazma, mevcut testleri çalıştırma**. `npm test`, `cargo test`, lint tam suite ÇALIŞTIRMA.

## Doğrulama (dar — kural)
1. `npm run build` — PASS olmalı.
2. `npm run typecheck` — PASS olmalı.
3. Rust tarafında değişiklik yaptıysan: `cargo fmt --manifest-path src-tauri/Cargo.toml --check` + `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` — PASS olmalı. Yapmadıysan çalıştırma.
4. Her komutun süresini raporla.

## Rapor (yanıtının sonunda)
- Yapılanlar (dosya yollarıyla): şablon kataloğu, PDF/Excel rapor, dokümanlar.
- Doğrulama tablosu (komut, sonuç, süre).
- Kullanıcı için UI smoke kontrol listesi (elle test adımları).
- Kalan/deferred işler (varsa).

## Git
git add/commit/push YAPMA. Çalışma ağacını olduğu gibi bırak.
