# Final Technical Debt Closure — FAZ 7 (OCR Görüntü Hattı: Deskew/Registration/DPI/Preprocess)

## Kapsam

Proje kökü: `/Users/kadir/Desktop/RubriKa/RubrikaV3`

"**Final Technical Debt Closure**" kampanyasının yedinci uygulama aşaması. FAZ 0+1, 2, 3, 4, 5, 6 tamamlandı (`docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` — tek otorite matris; FAZ 7 sonucuyla güncelle). Bu faz iş emrinin 6. bölümünü kapsar: **OCR görüntü hattı** — TD-21 (tarama deskew/registration/DPI normalizasyonu), TD-22 (preprocess varyant seçimi), TD-28 (OCR sonucu persistence atomicity) ve golden corpus ile OCR pipeline entegrasyon doğrulaması (model gerektirmeyen kısımlar). Kaynak: `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` §P1-5, §P2-9, §P1-6 (TD-28), `testdata/golden/tymm_tde_001/` (03_Doldurulmus_Tarama_Varyanti.pdf — skew/raster/contrast varyantı).

**Yetki ve yasaklar:**
- Production kodunda değişiklik serbest. Git commit oluşturma. Kullanıcıya ait değişiklikleri silme, stash yapma, geri alma (WIP korunur). `stash@{0}` korunur.
- Migration/repair/cleanup çalıştırma. Gerçek kullanıcı projesine dokunma. Golden dosyalarını DEĞİŞTİRME.
- **Model MEVCUT ve ÇALIŞIYOR** (kullanıcı teyidi): llama-server 127.0.0.1:8080'de dinliyor, `/health` = `{"status":"ok"}`. Model: `/Users/kadir/Desktop/llm/models/gemma-4-12B-it-qat-UD-Q4_K_XL.gguf` + mmproj `/Users/kadir/Desktop/llm/models/mmproj-F32.gguf`. Server'ı SEN başlatma (ben başlattım; gerekirse yalnız `/health` ile doğrula). API: OpenAI uyumlu `/v1/chat/completions` (image_url = `data:image/png;base64,...`). Her istek için timeout 180s; toplam benchmark 40 dk'yı aşarsa kalan kısmı PARTIAL olarak raporla, takılıp bekleme.
- Plan sunma, onay isteme yok. Doğrudan uygula, sonunda STATUS raporu ver.

## Mevcut durum (doğrula; tekrar keşfetme)

- Mevcut hattı oku: `src-tauri/src/services/student_scan_service.rs` (scan pipeline, deskew/registration durumu), `student_answer_crop_service.rs` (crop + `crop_rect_normalized` — Faz 6'da expose edildi), `student_answer_ocr_service.rs` (preprocess/OCR), `model_input_image_service.rs` (render/DPI), `llama_server_gateway.rs` (model çağrıları), `src-tauri/src/jobs/` (OCR job'ları), `src-tauri/src/domain/model.rs` (typed output DTO'ları), `src-tauri/src/domain/errors.rs`.
- Faz 5'ten gelen: `page_window_service.rs`; Faz 6'dan gelen: `golden_ocr_metrics.rs` + `tests/golden_tymm_tde_001.rs` + `docs/GOLDEN_OCR_SCORING_BENCHMARK.md`.
- TD-21/22/28 matris durumunu `docs/FINAL_TECHNICAL_DEBT_CLOSURE.md`'den oku.
- 03_Doldurulmus_Tarama_Varyanti.pdf golden'dır: skew/contrast/raster varyantı; bbox'lar 06_Golden_Set_Beklentileri.json'da.

## 6. OCR görüntü hattı (TD-21, TD-22, TD-28)

## 6.1 Deskew/registration/DPI (TD-21)

- Taranmış girdi (03 varyantı gibi) için deskew: satır açısı tahmini (Hough/yan projeksiyon basitliğinde — saf fonksiyon, `image` kütüphanesi mevcut) + döndürme; küçük açılar (-3°..+3°) ve sınırlar (≥8° ise OLD_CRIME/`DeskewOutOfRange` hatası — girdiyi reddet, sessiz düzeltme yapma).
- Registration: beklenen bbox grid'i ile hizalama sapması ölçümü (`crop_rect_normalized` tabanlı); sapma eşik üstündeyse typed hata. Golden 03 üzerinde grid kayıtlı mı — 06'daki bbox'larla doğrula; eşikleri golden'ın ölçülerine göre belirle.
- DPI normalizasyonu: render hedef DPI (300) sabit; girdi meta/DPI farkını doğrulayan ve normalleştiren saf fonksiyon + testler.
- Bütün deskew/registration fonksiyonları saf (girdi: görüntü/bounding verisi; çıktı: görüntü/ölçüm) — model çağrısı yok.

## 6.2 Preprocess varyant seçimi (TD-22)

- Varyant üretimi: aynı crop'tan N varyant (grayscale/adaptive threshold/contrast+sharp, mevcut fonksiyonlar) — saf fonksiyon.
- Varyant seçimi: model olmadan deterministik seçim kuralı belirle ve uygula (ör. görüntü istatistikleri: ortalama/standart sapma/kenar yoğunluğu → varyant skoru; en yüksek skor seçilir; eşik altı ise DEFAULT). Kural saf fonksiyon + birim testler.
- Model çağrısı seçim yapmaz; seçim modelden önce olur ve `preprocess_variant` adı diagnostics'e yazılır (Faz 5'teki provenance deseni).
- Birden fazla varyant modele gidiyorsa (fallback) sıra ve gerekçe diagnostics'e yazılır; tek varyantla başarı durumunda ikinci çağrı yok.

## 6.3 OCR sonuç persistence atomicity (TD-28)

- OCR job sonucu persistence: aynı işlem içinde birden çok state güncellemesi varsa (ocr result + status + artifact) tek atomik commit (transaction veya tek dosya atomik replace — proje desenine uy).
- Kısmi yazma olursa hiçbir state değişmemiş olmalı (ya hep ya hiç). Test: kasıtlı hata enjeksiyonu ile kısmi yazmanın gerçekleşmediğini kanıtla.
- Job/state makinesiyle çelişme yok; mevcut `job_manager.rs` desenini koru.

## 6.4 Golden entegrasyonu (03 varyantı, model gerektirmeyen)

- `tests/golden_tymm_tde_001.rs` deseninde (veya aynı hedef dosyada ek test modülü): 03 PDF render edilir (tempdir), crop bbox'ları 06'daki normalized bbox'larla doğrulanır, deskew/registration fonksiyonları golden ölçülerinde sınır içinde çalışır; 01/02 de regresyon için dokunulmaz.
- Model benchmark'ı `NEEDS_MODEL_RUNTIME` olarak `docs/GOLDEN_OCR_SCORING_BENCHMARK.md` bölüm 5'te güncellenir (Faz 7'nin gerçek OCR metni üretmediğini, altyapıyı doğruladığını açıkça yazar).
- `docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` matrisinde TD-21, TD-22, TD-28 güncelle.

## ÇALIŞMA SÖZLEŞMESİ

- Kapsam dışı dosyaları değiştirme; gereksiz refactor/biçimlendirme yapma.
- `git reset/clean/checkout --/restore`, force push, rebase, geçmiş değiştiren komutlar yasak. Hiçbir koşulda git commit/branch/tag/PR oluşturma.
- Yeni saf fonksiyonlar `#[cfg(test)]` modüllerinde birim testlerle; corpus entegrasyonu `src-tauri/tests/` altında (golden hedef dosyası mevcut — ona ekleme yapabilirsin).
- Golden dosyaları üzerinde hiçbir yazma işlemi yok.
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

- `cargo test --manifest-path src-tauri/Cargo.toml --test golden_tymm_tde_001` (exit 0)
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` (TAM lib; regresyon yok)
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check` (bozuksa yalnız değiştirdiğin dosyaları formatla)
- Frontend değişikliği varsa: `npm run typecheck`, `npm run lint`, `npm test -- --run`
- `git diff --check`

Tam suite (check:all, smoke, build, integration) FAZ 11'e aittir.

## Kabul kriterleri (bu faz)

- Deskew (saf, açı sınırlı, out-of-range reddi typed hatayla), registration sapma ölçümü (eşik + typed hata), DPI normalizasyonu fonksiyonları + birim testleri.
- Preprocess varyant üretimi + deterministik istatistik tabanlı seçim kuralı + diagnostics'e varyant adı; gereksiz ikinci model çağrısı yok.
- OCR persistence atomik (kısmi yazma enjeksiyon testi geçiyor; kısmi state yok).
- Golden 03 entegrasyon testi mevcut ve geçiyor; 01/02 regresyonu kırılmadı.
- TD-21, TD-22, TD-28 matriste güncellendi. Benchmark doc'ta NEEDS_MODEL_RUNTIME dürüst bölüm güncellendi. Yeni commit yok; kullanıcı değişiklikleri korunmuş.
