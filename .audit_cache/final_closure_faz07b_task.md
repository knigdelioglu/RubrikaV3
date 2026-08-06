# Final Technical Debt Closure — FAZ 7+ (Golden OCR Benchmark — gerçek model)

## Kapsam

Proje kökü: `/Users/kadir/Desktop/RubriKa/RubrikaV3`

"**Final Technical Debt Closure**" kampanyasının **Faz 7+ benchmark runner'ı**. Kod işi (TD-21/22/28) tamamlandı; bu görev yalnızca **gerçek model üzerinde golden OCR benchmark'ı** çalıştırıp kararları raporlar.

**Yetki ve yasaklar:**
- Git commit oluşturma. Kullanıcıya ait değişiklikleri silme/stash/geri alma. Golden dosyalarını DEĞİŞTİRME.
- Model server'ı zaten çalışıyor: **127.0.0.1:8080** (llama-server, Gemma 4 12B + mmproj). `/health` ile doğrula. SEN başlatma/kapatma.
- API: OpenAI uyumlu `POST /v1/chat/completions`; görsel `image_url: "data:image/png;base64,..."`; deterministik çıktı için `temperature: 0` (server zaten `--temp 0 --top-k 1 --reasoning off`).
- Her istek timeout 180s. Toplam 40 dk sınırı; aşarsa PARTIAL raporla, takılma.
- Plan sunma, onay isteme yok. Sonunda STATUS raporu.

## Mevcut durum (tekrar keşfetme; doğrula yeter)

- `testdata/golden/tymm_tde_001/`: 01_Bos, 02_Doldurulmus, **03_Doldurulmus_Tarama_Varyanti.pdf** (benchmark hedefi), 04_Cevap_Anahtari, 05_Rubrik_Golden.json (ground truth + crop rect'ler), 06_Golden_Set_Beklentileri.json (answer-type eşleşmeleri), manifest.sha256.
- `src-tauri/src/services/golden_ocr_metrics.rs`: `cer()`, `wer()`, `exact_match()`, `leakage` ölçümleri + `GoldenOcrBenchmarkReport` DTO + 16 birim testi. Bunları KULLAN (yeniden yazma).
- `src-tauri/src/services/ocr_image_geometry_service.rs`: deskew/registration/DPI fonksiyonları (Faz 7).
- `src-tauri/src/services/student_answer_ocr_service.rs`: gerçek üretim hattı; görsel istekleri nasıl kuruyor (prompt şablonu, crop) → buradaki prompt şablonunu kullan (yalnız server adresi 127.0.0.1:8080).
- PDF render: poppler `pdftoppm` veya JXA mevcut (Faz 6/7'de kullanıldı; golden entegrasyon testi render konvansiyonunu `page-{i}.png` olarak biliyor).
- `docs/GOLDEN_OCR_SCORING_BENCHMARK.md`: bölüm 5 `NEEDS_MODEL_RUNTIME` — bu bölümü gerçek sonuçlarla DEĞİŞTİR (bölüm 4'teki metodolojiyi izle).

## Görevler (sırayla)

1. **Hazırlık:** `/health` doğrula. Golden 03'ü 300 DPI render et (poppler/JXA) → `page-1..N.png`. `05_Rubrik_Golden.json`'dan her sorunun crop rect'ini al (Q1 iki bölge, Q2–Q6 tek). Crop'ları kes (image crate veya pdftoppm -x -y -W -H; mevcut crop altyapısını kullan). Çalışma dosyaları tempdir'e; golden'a DOKUNMA.
2. **Benchmark döngüsü:** Her soru crop'u için: üretim hattının prompt şablonuyla (student_answer_ocr_service.rs'den) `POST /v1/chat/completions` (image_url data URI, base64 PNG). Yanıtı kaydet. Sonraki isteğe geçmeden önce server'ın boşta olduğundan emin ol (paralel 1 slot; sırayla gönder).
3. **Metrikler:** Yanıtları 05_Rubrik_Golden.json ground truth'i ile karşılaştır: `golden_ocr_metrics` fonksiyonlarıyla CER/WER/exact-match + leakage taraması. JSON structured answer ise alan bazlı eşleşme (06 beklentilerine göre). Raporda soru bazlı tablo: soru no, görsel sayısı, yanıt özeti, CER/WER/exact, leakage.
4. **Kararlar:**
   - **Registration gating kararı:** golden 03 ölçümü (Faz 7: 0–0.75°, <0.01) + bu benchmark sonucuna göre `validate_registration` eşiğini canlı OCR hattına bağla (sahte ret riski yoksa): `preprocess_model_inputs`'ta registration doğrulaması eklensin mi? Karar + gerekçe + (karar evet ise) kod.
   - **Q3 canonical anahtar kararı:** 05 rubric ile golden ground truth arasındaki Q3 madde 5 tutarsızlığı için canonical anahtar seç; nedenini docs'a yaz.
5. **Belgeleme:** `docs/GOLDEN_OCR_SCORING_BENCHMARK.md` bölüm 5'i gerçek sonuçlarla değiştir (tarih, model, quant, sunucu parametreleri, soru tablosu, kararlar). `docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` matrisine kısa Faz 7+ satırı ekle (TD-21 kalıntısı + TD-32 benchmark statüsü).
6. **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml --lib` (tam; eğer `start_job_returns_model_mmproj_missing` ortam testi server yüzünden başarısızsa `--skip` ile koş, not düş), `cargo test --manifest-path src-tauri/Cargo.toml --test golden_tymm_tde_001`, clippy `--all-targets --all-features -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `git diff --check`. HEAD değişmemeli (`fdb8e6e`), stash korunmalı.

## Kabul kriterleri

- Golden 03'teki her soru için gerçek model yanıtı alınmış, CER/WER/exact/leakage ölçülmüş (tabloda).
- Benchmark raporu docs'ta gerçek sonuçlarla (NEEDS_MODEL_RUNTIME kaldırıldı veya kısmi sonuçlar açıkça işaretli).
- Registration gating + Q3 canonical anahtar kararları yazılı gerekçeyle.
- Testler/clippy/fmt/diff yeşil; commit yok; golden dosyaları SHA-256 değişmedi.

## Çıktı

`STATUS: COMPLETED` (veya PARTIAL + neden) + SUMMARY (soru tablosu özeti, kararlar, CHANGED_FILES, VALIDATION sonuçları).
