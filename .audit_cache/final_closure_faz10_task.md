# Final Technical Debt Closure — FAZ 10 (Kalan Borçlar & Nihai Kararlar)

## Kapsam

Proje kökü: `/Users/kadir/Desktop/RubriKa/RubrikaV3`

"Final Technical Debt Closure" kampanyasının **Faz 10'u: kalan açık borçların nihai değerlendirmesi**. Kapsam: **TD-31** (Rubrik şablonları frontend'de), **TD-36** (Speaking retry sabit 2s), **TD-38** (Rapor O(n) tarama), **TD-39** (PDF window.print vs pdf_service) + Faz 9'da ertelenen yapısal işin kararı.

**Yetki ve yasaklar:**
- Production kodunda değişiklik serbest ama **sınırlı**: bu fazda amaç "her borcu kapatmak" DEĞİL, her borç için **dürüst nihai karar** vermektir: kapat (güvenliyse, testle) veya ertelenebilir/kabul edilebilir olarak işaretle (gerekçeyle). Zorlama yok; AGENTS.md "en küçük güvenli değişiklik".
- Git commit oluşturma. Kullanıcı değişikliklerini silme/stash/geri alma (WIP korunur). `stash@{0}` korunur. Migration/repair/cleanup çalıştırma. `testdata/golden/tymm_tde_001/` dosyalarını DEĞİŞTİRME.
- Llama-server 127.0.0.1:8080 çalışıyor; `start_job_returns_model_mmproj_missing` ortam testi `--skip` ile koşulur (kod kaynaklı değil).
- Frontend değişikliği varsa `npm run typecheck` + `npm test` zorunlu.
- Plan sunma, onay isteme yok. Sonunda STATUS raporu.

## Mevcut durum (doğrula; tekrar keşfetme)

- Matris (docs/FINAL_TECHNICAL_DEBT_CLOSURE.md):
  - TD-31 CONFIRMED — `performanceOrganizationUi.ts` şablon kataloğu; not: "Kabul edilebilir/ertele; publish backend doğrulaması var".
  - TD-36 CONFIRMED — `speaking_exam_service.rs` retry sabit 2s.
  - TD-38 CONFIRMED — `get_performance_report` roster taraması O(n).
  - TD-39 CONFIRMED — `PerformanceScoringPage.tsx` window.print vs `pdf_service`.
  - FAZ 9 ertelenen: `performance_service.rs` mantık parçalama + AppState bölme (yüksek riskli, gerekçeli ertelendi).
- Faz 10'da bu borçların her biri için nihai karar verilecek.

## Görevler (sırayla)

1. **Her TD için dürüst değerlendirme** (TD-31, TD-36, TD-38, TD-39):
   - Kodda mevcut davranışı doğrula (kısa).
   - Kapatma **güvenli ve küçük** ise yap: ör. TD-38 gerçekten O(n) ama n küçükse ve iyileştirme tek noktadaysa; TD-36 retry süresi sabit 2s ise basit konfigüre edilebilir yapılabilir (ör. env/const + test) — yalnız davranışı bozmayan minimum adım. TD-39 için: window.print'in pdf_service ile değiştirilmesi büyük işse → kabul edilebilir olarak işaretle + gerekçe (ör. mevcut print akışı çalışıyor, pdf_service ayrı özellik).
   - Aksi halde **ERLENDİ/ERTELENDİ** olarak matriste işaretle (TD-31 için zaten "kabul edilebilir" notu var — doğrula ve onayla).
   - **Hiçbir borç "yapıldı" diye işaretlenmez; yalnız gerçekten yapıldıysa ALREADY_FIXED/PARTIAL güncellenir.**
2. **Faz 9 ertelemesinin kararı:** `performance_service.rs` mantık parçalama + AppState bölme → bu kampanya kapsamında bilinçli erteleme kararı yaz (risk/gerekçe/kapsam-dışı notu), matriste işaretle. Kod değişikliği YAPMA.
3. **Docs:** `FINAL_TECHNICAL_DEBT_CLOSURE.md`'ye FAZ 10 bölümü: her TD için karar + gerekçe tablosu; kampanya sonrası durum özeti (kaç TD kapandı, kaçı ertelendi/erteleme).
4. **Doğrulama:** Değişiklik yapıldıysa ilgili hedefli testler; her durumda: `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --skip start_job_returns_model_mmproj_missing`, `cargo test --manifest-path src-tauri/Cargo.toml --test golden_tymm_tde_001`, clippy `--all-targets --all-features -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `git diff --check`; frontend değiştiyse `npm run typecheck` + `npm test`. HEAD `fdb8e6e` değişmemeli, stash korunmalı, golden SHA-256 değişmemeli.

## Kabul kriterleri

- TD-31/36/38/39 + Faz 9 ertelemesi için yazılı, dürüst, gerekçeli nihai karar (kapatıldıysa test kanıtı; ertelendiyse neden).
- Kampanya sonu özeti docs'ta (kapanan/ertelenen sayıları).
- Doğrulamalar yeşil; commit yok; golden değişmedi; stash korundu.

## Çıktı

`STATUS: COMPLETED` + SUMMARY (her TD kararı, CHANGED_FILES, VALIDATION).
