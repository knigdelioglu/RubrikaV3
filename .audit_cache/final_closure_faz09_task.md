# Final Technical Debt Closure — FAZ 9 (Yapısal Şema Uyumu & Modüler Sınırlar)

## Kapsam

Proje kökü: `/Users/kadir/Desktop/RubriKa/RubrikaV3`

"Final Technical Debt Closure" kampanyasının **Faz 9'u: yapısal şema uyumu ve modüler sınırlar**. Kapsam: **TD-24** (Ham `Project` + `types.ts` monolit), **TD-26** (Büyük servisler / `AppState`), **korpus bbox düzeltmesi** (Faz 7+ bulgusu: `06_Golden_Set_Beklentileri.json` bbox y-ekseni PDF alt-sol kaynaklı, `crop_rect_normalized` üst-sol bekliyor).

**Yetki ve yasaklar:**
- Production kodunda değişiklik serbest. Git commit oluşturma. Kullanıcıya ait değişiklikleri silme/stash/geri alma (WIP korunur). `stash@{0}` korunur.
- Migration/repair/cleanup çalıştırma. Gerçek kullanıcı projesine dokunma. `testdata/golden/tymm_tde_001/` dosyalarını DEĞİŞTİRME (manifest var; bbox düzeltmesi golden dosyalarında DEĞİL, tüketici kodda/runner'da yapılır).
- Llama-server 127.0.0.1:8080 çalışıyor; `start_job_returns_model_mmproj_missing` ortam testi bu yüzden başarısız olabilir → `--skip` ile koş, NOT düş.
- **Refactor disiplini (AGENTS.md):** En küçük güvenli değişiklik. Monolitleri "kırma zorunluluğu" yok; yalnız güvenli, davranış değiştirmeyen taşımalar yapılır. Riskli büyük refactor'lar ertelenir ve gerekçesiyle matrise yazılır. Frontend değişikliği varsa `npm run typecheck` + `npm test` zorunlu.
- Plan sunma, onay isteme yok. Sonunda STATUS raporu.

## Mevcut durum (doğrula; tekrar keşfetme)

- `docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` matrisi: TD-24 "Ham Project + types.ts monolit" CONFIRMED (Tur 8); TD-26 "Büyük servisler / AppState" CONFIRMED (`performance_service.rs` 2778 satır vb.); TD-31/36/38/39 CONFIRMED (Faz 10'a bırakıldı — DOKUNMA).
- `src-tauri/src/commands/project_commands.rs` + `src/api/types.ts` (frontend): monolit alanları.
- `src-tauri/src/services/performance_service.rs` (2778+ satır) + AppState sahipliği.
- Faz 7+ bulgusu: `benchmark_details.json` runner'ı `y_top=1−(y_bottom+h)` dönüşümü uyguluyor (golden 06 bbox alt-sol y → üst-sol). Dönüşüm şu an runner'da; production crop hattıyla tutarlılık kontrol edilecek.

## Görevler (sırayla)

1. **TD-24 değerlendirme + sınırlı iyileştirme:** `types.ts` monolitini incele. Güvenli iyileştirme: domain alanına göre bölme YALNIZCA import grafiği basitse ve `npm run typecheck` + ilgili `npm test` yeşil kalıyorsa yapılır. Bölme riskliyse (döngüsel import, çok sayıda dokunulmamış dosya) → ertelenebilir olarak işaretle, gerekçe yaz. Her iki durumda da karar matrise yazılır. **Zorlama yok.**
2. **TD-26 değerlendirme + sınırlı iyileştirme:** `performance_service.rs` ve AppState sahipliğini incele. Yalnız güvenli taşıma yapılabilir: örneğin açık bir alt alanı (net sınırı olan) ayrı modüle taşımak — davranış değişikliği YOK, testler aynı kalmalı. Taşıma riskliyse ertelenebilir işaretle + gerekçe. **Zorlama yok.**
3. **Korpus bbox düzeltmesi:** Golden 06 bbox konvansiyonunu incele (PDF alt-sol y kaynağı). Golden dosyaları DEĞİŞMEZ. Çözüm: (a) dönüşümü `golden_ocr_benchmark` runner'ında tek, dokümante edilmiş fonksiyona al (zaten uygulanıyorsa doğrula + test ekle), (b) `docs/GOLDEN_OCR_SCORING_BENCHMARK.md`'ye konvansiyon farkını açıkça yaz (bölüm: "Corpus koordinat konvansiyonu"), (c) production crop hattının bu farktan ETKİLENMEDİĞİNİ doğrula (golden bbox'lar production'a giriyorsa dönüşüm uygula; girmiyorsa dokunma + not). Kararı gerekçelendir.
4. **Docs:** `FINAL_TECHNICAL_DEBT_CLOSURE.md`'ye FAZ 9 bölümü: TD-24/TD-26 kararları (kapatıldıysa ne yapıldı; ertelendiyse neden), korpus bbox kararı.
5. **Doğrulama:** Rust: `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --skip start_job_returns_model_mmproj_missing`, `cargo test --manifest-path src-tauri/Cargo.toml --test golden_tymm_tde_001`, clippy `--all-targets --all-features -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `git diff --check`. Frontend dokunulduysa: `npm run typecheck` + `npm test`. HEAD `fdb8e6e` değişmemeli, stash korunmalı, golden SHA-256 değişmemeli.

## Kabul kriterleri

- TD-24 ve TD-26 için yazılı karar: yapıldı (testlerle) veya ertelendi (gerekçeyle). Matris güncellenmiş.
- Korpus bbox: dönüşüm tek yerde, dokümante, testli (runner testi); production etkilenmediği kanıtlanmış veya dönüşüm eklenmiş.
- Tüm doğrulamalar yeşil; commit yok; golden değişmedi; stash korundu.

## Çıktı

`STATUS: COMPLETED` + SUMMARY (kararlar, ne yapıldı/ertelendi, CHANGED_FILES, VALIDATION).
