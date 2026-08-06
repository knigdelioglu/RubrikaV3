# Final Technical Debt Closure — FAZ 5 (Soru/Rubrik Model Çağrısı Verimliliği)

## Kapsam

Proje kökü: `/Users/kadir/Desktop/RubriKa/RubrikaV3`

"**Final Technical Debt Closure**" kampanyasının beşinci uygulama aşaması. FAZ 0+1, 2, 3, 4 tamamlandı (`docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` — tek otorite matris; oku ve FAZ 5 sonucuyla güncelle). Bu faz yalnız iş emrinin 7. bölümünü kapsar: **TD-19** (extraction her soru için tüm sayfaları tekrar gönderiyor) ve **TD-20** (rubrik parse retry tam multimodal tekrar). Kaynak: `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` §P1-17/P1-18 ve `docs/UYGULAMA_PLANI.md` 2.10/2.12.

**Yetki ve yasaklar:**
- Production kodunda değişiklik serbest. Git commit oluşturma. Kullanıcıya ait değişiklikleri silme, stash yapma, geri alma (49+ değişik dosya WIP'tir, korunur). Mevcut `stash@{0}` korunur.
- Migration/repair/cleanup çalıştırma. Gerçek kullanıcı projesine dokunma.
- Golden set (testdata/golden/tymm_tde_001) bu fazın değişikliklerini ÖLÇMEK için kullanılabilir ama model binary'si yoksa model benchmark'ı çalıştırma; PASS uydurma.
- Plan sunma, onay isteme, explore/keşif ajanı kullanma yok. Doğrudan uygula, sonunda STATUS raporu ver.

## Mevcut durum (doğrula; tekrar keşfetme)

- `question_text_service.rs` ~:752: `model_input_images: all_prepared_inputs.clone()` — her hedef soru isteğinde tüm sayfa inputları gönderiliyor.
- `rubric_extraction_service.rs` ~:606: aynı desen; retry ~:1002-1039 `request.clone()` (görseller dahil) ile tam multimodal ikinci çağrı yapıyor, yalnız `strictJsonOnly=true, attempt=2`.
- `llama_server_gateway.rs` ~:1781-1822: tüm görseller base64 "high" detail ile user mesajına konuyor.
- `student_answer_crop_service.rs`/`model_input_image_service.rs`: sayfa görselleri cache'li (content-addressed JPEG).
- Önceki turlar TD-19/TD-20'ye dokunmadı (matriste CONFIRMED durumda — doğrula).

## 7. Soru/rubrik model çağrısı verimliliğini tamamla

- Her soru için tüm sayfaları tekrar gönderme.
- Question-to-page/region map oluştur (PDF text/marker/layout analizi; soru numarası→sayfa adayı; bölge/region bilgisi mevcutsa onu kullan).
- İlk çağrı hedef sayfa/region; düşük confidence/not_visible durumunda ±1 pencere; son çare geniş fallback (bounded, job stage olarak).
- Rubrik parse retry'de görselleri yeniden göndermeden deterministic salvage veya text-only JSON repair kullan (mevcut `parse_partial`/salvage yardımcılarını değerlendir).
- Full multimodal retry yalnız açık retry reason ile (ör. schema'da görselden gelmesi gereken alan eksikse ve text-only repair başarısızsa).
- Model çağrısı, image token ve prefill metriklerini diagnostics'e yaz (kullanılan sayfa listesi/region seti, retry reason, image sayısı, attempt sayısı provenance'da).

Davranış korunmalı: soru/rubrik çıkarım sonuçları (field recall) gerilememeli; teacher confirmation akışı değişmez; QEP freeze gate etkilenmez.

## ÇALIŞMA SÖZLEŞMESİ

- Kapsam dışı dosyaları değiştirme; gereksiz refactor/biçimlendirme yapma.
- `git reset/clean/checkout --/restore`, force push, rebase, geçmiş değiştiren komutlar yasak. Hiçbir koşulda git commit/branch/tag/PR oluşturma.
- Model çağrısı içermeyen testlerle doğrula: request-capture/spy testleri (hedef sayfa seti gönderiliyor, tüm sayfalar değil), retry zinciri testleri (salvage/repair başarılıysa ikinci multimodal çağrı 0), ±1 pencere ve fallback testleri. Mevcut gateway test desenlerini (fake gateway/request capture) kullan.
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

- Hedefli: `cargo test --manifest-path src-tauri/Cargo.toml --lib question_text`, `--lib rubric_extraction`, `--lib llama_server_gateway`
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` (TAM lib)
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check` (bozuksa yalnız değiştirdiğin dosyaları formatla)
- Frontend değişikliği varsa: `npm run typecheck`, `npm run lint`, `npm test -- --run`
- `git diff --check`

Tam suite (check:all, smoke, build, integration) FAZ 11'e aittir; bu fazda çalıştırma.

## Kabul kriterleri (bu faz)

- Extraction istekleri hedef sayfa/region setiyle sınırlı (spy testi: tüm-sayfa clone yok).
- Düşük confidence/not_visible'ta ±1 pencere; geniş fallback yalnız son çare ve bounded (testli).
- Rubrik retry: salvage/text-only repair başarılıysa ikinci görsel çağrısı 0; multimodal retry yalnız açık reason ile (testli).
- Diagnostics/provenance: attempt sayısı, kullanılan sayfa/region listesi, retry reason, image sayısı yazılıyor.
- TD-19/TD-20 matriste FAZ 5 ile güncellendi.
- Yeni commit yok; kullanıcı değişiklikleri korunmuş.
