# Final Technical Debt Closure — FAZ 6 (Golden Sınav Paketi → Committed Test Corpus)

## Kapsam

Proje kökü: `/Users/kadir/Desktop/RubriKa/RubrikaV3`

"**Final Technical Debt Closure**" kampanyasının altıncı uygulama aşaması. FAZ 0+1, 2, 3, 4, 5 tamamlandı (`docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` — tek otorite matris; FAZ 6 sonucuyla güncelle). Bu faz yalnız iş emrinin 5. bölümünü kapsar: **TD-32** (golden set / benchmark altyapısı yok) ve golden paketin committed test corpus'a dönüştürülmesi. Kaynak: `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` §P2-9, `testdata/golden/tymm_tde_001/README.md` ve `06_Golden_Set_Beklentileri.json`.

**Yetki ve yasaklar:**
- Production kodunda değişiklik serbest. Git commit oluşturma. Kullanıcıya ait değişiklikleri silme, stash yapma, geri alma (WIP korunur). `stash@{0}` korunur.
- Migration/repair/cleanup çalıştırma. Gerçek kullanıcı projesine dokunma.
- Golden dosyalarını DEĞİŞTİRME (üretim testi sırasında değiştirilmemeli). Test output'ları tempdir'e yazılır.
- Model binary'si bu ortamda yok: model benchmark'ı çalıştırma; `NEEDS_MODEL_RUNTIME` olarak ayrı raporla. PASS uydurma.
- Plan sunma, onay isteme yok. Doğrudan uygula, sonunda STATUS raporu ver.

## Mevcut durum (doğrula; tekrar keşfetme)

- `testdata/golden/tymm_tde_001/`: 01_Bos_Sinav_Kagidi.pdf, 02_Doldurulmus_Ornek_Kagit.pdf, 03_Doldurulmus_Tarama_Varyanti.pdf, 04_Cevap_Anahtari_ve_Rubrik.pdf, 05_Rubrik_Golden.json, 06_Golden_Set_Beklentileri.json, README.md, manifest.sha256 (README'de geçiyor; dosya gerçekte var mı doğrula).
- Beklenen toplam puan 80/100; kalite kapıları 06 içinde (CER/WER eşikleri, leakage yasakları, structured field exact match 1.0, expected_total 80).
- Q1 iki sayfa/iki bölge; Q2 table; Q3 matching; Q4 correction table; Q5 grammar analysis; Q6 open-ended.
- Test corpus: mevcut integration testleri `src-tauri/tests/` altında (final_data_loss_proofs.rs vb. deseni).

## 5. Golden sınav paketini committed test corpus'a dönüştür

## 5.1 Dosya bütünlüğü

- Golden dosyalarının SHA-256 manifestini testte doğrula (mevcut manifest.sha256 yoksa veya eksikse — README'de belirtilen 07_CodeX_Teknik_Borc_Kapanis_Promptu.md de listede — dosyalardan üret ve committed manifest olarak ekle; golden PDF'lerin byte'larını test sabiti olarak kullanma, dosya okuma yap).
- PDF'ler production test sırasında değiştirilmemeli: testler salt okur (read-only) erişimle çalışır; değiştirme denemesi yok.
- Test output'ları tempdir'e yazılır (`std::env::temp_dir()` + unique suffix; test bitiminde temizlik best-effort).

## 5.2 Sınav yapısı

- Blank exam (01): soru text extraction ve rubrik extraction kaynağı olarak kullanılabilirliği testi — pdf parse/sayfa sayısı/boyut doğrulaması.
- Filled vector exam (02): temiz OCR baseline kaynağı — render/crop pipeline'ının region'ları doğru kesmesi (06'daki normalized bbox'larla karşılaştırma, tempdir'de render).
- Scanned variant (03): skew/contrast/raster — dosya var ve geçerli PDF; render edilebiliyor; (deskew/registration Faz 7'de; bu fazda yalnız pipeline'ın girdiyi kabul ettiği ve bbox'ların sınırlar içinde kaldığı testi).
- Q1 iki sayfalı ve iki bölgeli cevap olarak işlenmeli: 06'daki `regions.q1` iki bölge (page 1 primary, page 2 continuation) — region modeline uygunluk testi (mevcut `QuestionAnswerTemplate.regions[]` deseni).
- Q2 table, Q3 matching, Q4 correction table, Q5 grammar analysis, Q6 open-ended schema ile çözülmeli: 05'teki answer_type'lar ile `StructuredAnswer` varyantlarının eşleşmesi testi (her golden question için beklenen typed variant).

## 5.3 OCR kalite metrikleri (NEEDS_MODEL_RUNTIME ayrımıyla)

`06_Golden_Set_Beklentileri.json` içindeki gerçek ground truth'a göre ölçüm ALTYAPISI kur:

- CER / WER hesaplayan saf fonksiyon (Levenshtein tabanlı; Türkçe karakterlerle; normalize edilmiş karşılaştırma) + birim testleri.
- critical-token error, printed-question leakage (OCR metninde basılı soru metninden sızıntı), structured-field exact match (Q2 tablo alanları, Q3 eşleştirme anahtarı) hesaplayan saf fonksiyonlar.
- p50/p95 süre, image token sayısı, model call sayısı, retry sayısı, peak memory için **ölçüm DTO'su ve raporlama yapısı** (benchmark runner'ı olmadan): `GoldenOcrBenchmarkReport` tipi + serde + doküman.
- Model binary/model dosyası test ortamında YOKSA:
  - preprocess/crop/registration ve parser testlerini çalıştır (PDF render → region crop → (mevcut preprocess) → boyut/format doğrulama; Q1 iki bölge birleştirme sırası testi; 06'daki bbox'ların geçerliliği).
  - Model benchmark'ını `NEEDS_MODEL_RUNTIME` olarak ayrı raporla (`docs/GOLDEN_OCR_SCORING_BENCHMARK.md` içinde dürüst bölüm).
  - PASS uydurma.

## 5.4 Teslim

- `docs/GOLDEN_OCR_SCORING_BENCHMARK.md`: corpus tanımı, SHA-256 manifest tablosu, ölçüm metodolojisi (CER/WER tanımları), mevcut durumda çalıştırılan testlerin sonuçları, `NEEDS_MODEL_RUNTIME` bölümü, Faz 7+ için eşikler.
- `docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` matrisinde TD-32 güncelle.

## ÇALIŞMA SÖZLEŞMESİ

- Kapsam dışı dosyaları değiştirme; gereksiz refactor/biçimlendirme yapma.
- `git reset/clean/checkout --/restore`, force push, rebase, geçmiş değiştiren komutlar yasak. Hiçbir koşulda git commit/branch/tag/PR oluşturma.
- Testler mevcut desenlere uygun: saf fonksiyonlar `#[cfg(test)]` modüllerinde, corpus entegrasyonu `src-tauri/tests/` altında ayrı hedef dosyasında (örn. `golden_tymm_tde_001.rs`), model gerektirmeyen.
- Golden dosyaları üzerinde hiçbir yazma işlemi yok; manifest dosyası dışında corpus dizinine yazma.
- Çalışma sonunda:

```text
STATUS: COMPLETED | BLOCKED | APPROVAL_REQUIRED | FAILED
SUMMARY: En fazla 10 satır
CHANGED_FILES: ...
VALIDATION: komut, exit code, passed/failed, süre
RISKS: ...
NEXT_ACTION: ...
```

## Doğrulama (bu faz)

- `cargo test --manifest-path src-tauri/Cargo.toml --test golden_tymm_tde_001` (yeni hedef; exit 0)
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` (TAM lib; regresyon yok)
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check` (bozuksa yalnız değiştirdiğin dosyaları formatla)
- Frontend değişikliği varsa: `npm run typecheck`, `npm run lint`, `npm test -- --run`
- `git diff --check`

Tam suite (check:all, smoke, build, integration) FAZ 11'e aittir.

## Kabul kriterleri (bu faz)

- SHA-256 manifest testi golden dosyalarını doğruluyor (manifest eksikse committed olarak eklendi).
- Golden dosyalarına test sırasında yazma yok; output'lar tempdir.
- CER/WER/critical-token/leakage/structured-field exact match saf fonksiyonları testli.
- Benchmark rapor DTO'su ve `docs/GOLDEN_OCR_SCORING_BENCHMARK.md` mevcut; model benchmark'ı dürüstçe NEEDS_MODEL_RUNTIME.
- Q1 iki bölge, Q2-Q6 answer type eşleşmeleri testli.
- TD-32 matriste güncellendi. Yeni commit yok; kullanıcı değişiklikleri korunmuş.
