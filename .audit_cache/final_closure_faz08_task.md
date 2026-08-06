# Final Technical Debt Closure — FAZ 8 (Scoring Calibration & Fingerprint)

## Kapsam

Proje kökü: `/Users/kadir/Desktop/RubriKa/RubrikaV3`

"Final Technical Debt Closure" kampanyasının **Faz 8'i: Scoring Calibration**. Aşağıdaki teknik borçları kapat: **orijinal denetim TD-28** (scoring fingerprint calibration/anchor `"none"` placeholder), **TD-37** (deterministik scoring kapsamı PARTIAL), **TD-35** (model/runtime benchmark statüsü — Faz 7+ sonrası), **TD-34** (preprocess eager maliyeti — Faz 7'de kaldırıldı, doğrula).

**Yetki ve yasaklar:**
- Production kodunda değişiklik serbest. Git commit oluşturma. Kullanıcıya ait değişiklikleri silme/stash/geri alma (WIP korunur). `stash@{0}` korunur.
- Migration/repair/cleanup çalıştırma. Gerçek kullanıcı projesine dokunma. `testdata/golden/tymm_tde_001/` dosyalarını DEĞİŞTİRME.
- Llama-server 127.0.0.1:8080 çalışıyor (Faz 7+ benchmark'ı için kullanıldı); yalnız `start_job_returns_model_mmproj_missing` ortam testi bu yüzden başarısız olabilir — o testi `--skip` ile koş, NOT düş (kod kaynaklı değil).
- Plan sunma, onay isteme yok. Doğrudan uygula, sonunda STATUS raporu.

## Mevcut durum (doğrula; tekrar keşfetme)

- `docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` matrisi güncel (TD-01..39; FAZ 1–7+ kapanışları). Orijinal TD-28 notu matris TD-28 satırında: "scoring fingerprint calibration/anchor `none` placeholder, `scoring_service.rs`; ayrı izlenir".
- `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` P2-5 (satır ~627): "ScoringFingerprint kalibrasyon ve anchor sürümlerini sabit `none` olarak yazıyor → cache geçersiz kullanılabilir".
- `src-tauri/src/services/scoring_cache_service.rs`: fingerprint cache — kalibrasyon/anchor alanlarını incele (muhtemelen sabit `"none"`).
- `src-tauri/src/services/deterministic_scoring_service.rs`: 8 tür deterministik scoring (TD-37 PARTIAL — kapsamı 06_Golden_Set_Beklentileri.json'daki answer-type'larla karşılaştır).
- Faz 7+ benchmark'ı bitti: `docs/GOLDEN_OCR_SCORING_BENCHMARK.md` bölüm 5'te gerçek sonuçlar (modelRuntime=available; Q1–Q6 CER/WER tablosu). TD-35 `NOT_FOUND` statüsü artık eski.
- Faz 7 (TD-22) eager 5x preprocess üretimini kaldırdı — TD-34 "Preprocess eager maliyeti" muhtemelen çözüldü; kodda doğrula.

## Görevler (sırayla)

1. **Fingerprint kalibrasyon (orijinal TD-28):** `ScoringFingerprint`'te kalibrasyon + anchor'ın sabit `"none"` placeholder'ı yerine gerçek, istikrarlı politikayı temsil eden değerler yaz. Politika: kalibrasyon sürümü (ör. model/prompt/parametre kümesi değişince artan sabit) + anchor sürümü (rubrik/şablon kümesi sürümü). Bu alanların cache anahtarına girdiğini ve `prompt_contract.rs` sürümlerine bağlandığını doğrula (bağlantı yoksa ekle). Yeni fingerprint'in eski cache'leri geçersiz kılması gerekiyorsa bunu nasıl sağlayacağına karar ver (sürüm sabitini artırmak yeterli mi?) — karar + test.
2. **TD-37 deterministik scoring kapsamı:** Mevcut 8 türü `06_Golden_Set_Beklentileri.json` answer-type eşleşmeleriyle karşılaştır; eksik kapsanabilir tür varsa ekle (küçük, güvenli ekleme; mevcut davranışı bozma). Kapsam değerlendirmesini matrise yaz.
3. **TD-35:** Matris satırını güncelle: benchmark Faz 7+ ile çalıştı (`modelRuntime=available`), kalan tuning kararları için not. `NOT_FOUND` → `ALREADY_FIXED` (benchmark altyapısı + ilk ölçüm) veya PARTIAL (tuning yapılmadı) — dürüst seçim yap, gerekçelendir.
4. **TD-34:** Kodda eager preprocess kaldırıldığını doğrula (Faz 7: `preprocess_model_inputs` yalnız seçilen varyantı üretir); doğruysa matriste `ALREADY_FIXED` yap (FAZ 7 referansıyla).
5. **Docs:** `FINAL_TECHNICAL_DEBT_CLOSURE.md`'ye FAZ 8 kapanış bölümü (kısa: ne kapatıldı, testler, kararlar). `CURRENT_TECHNICAL_DEBT_AUDIT.md`'ye dokunma (kaynak belge).
6. **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --skip start_job_returns_model_mmproj_missing` (tam), hedefli `--test golden_tymm_tde_001`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `git diff --check`. HEAD `fdb8e6e` değişmemeli, stash korunmalı, golden SHA-256 değişmemeli.

## Kabul kriterleri

- Fingerprint kalibrasyon/anchor artık gerçek politikayı temsil ediyor; cache anahtarına girdiği testle kanıtlanmış; placeholder `"none"` kalmadı (kanıt: grep).
- TD-37 kapsam kararı (genişletildiyse testlerle, değilse gerekçeyle) matriste.
- TD-35/TD-34 matris statüleri dürüstçe güncellenmiş.
- Testler/clippy/fmt/diff yeşil; commit yok; golden değişmedi; stash korundu.

## Çıktı

`STATUS: COMPLETED` + SUMMARY (ne kapatıldı, kararlar, CHANGED_FILES, VALIDATION sayıları).
