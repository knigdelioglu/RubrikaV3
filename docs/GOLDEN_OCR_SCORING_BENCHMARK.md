# Golden OCR / Scoring Benchmark — `tymm_tde_001`

RubrikaV3 sentetik golden sınav paketinin committed test corpus'una dönüştürülmesi
ve OCR/scoring kalite ölçüm metodolojisinin tanımı. Model binary'si olmadan
çalıştırılabilen tüm yapılar burada; gerçek model ölçümleri Faz 7+ benchmark
runner'ı ile bölüm 5'te raporlanır (`modelRuntime=available`).

## 1. Corpus tanımı

Korpus: `testdata/golden/tymm_tde_001/`. Tamamen sentetiktir; gerçek öğrenci verisi
içermez. Beklenen toplam puan: **80/100**.

| Soru | Tip | Beklenen puan | Sayfa/bölge notu |
|------|-----|--------------|------------------|
| Q1 | Open-ended (iki bölgeli) | 18 | sayfa 1 `primary` + sayfa 2 `continuation` |
| Q2 | Table | 20 | 5 alan (Anlatıcı/Mekân/Zaman/özellik/kanıt) |
| Q3 | Matching | 12 | 5 eşleştirme |
| Q4 | Correction table | 10 | 3 satır |
| Q5 | Grammar analysis | 7 | öge + yüklem yapısı |
| Q6 | Open-ended | 13 | şiir yorumu |

## 2. SHA-256 manifest

Committed manifest: `testdata/golden/tymm_tde_001/manifest.sha256`. İmza testi
(`golden_tymm_tde_001::manifest_sha256_verifies_all_golden_files`) her dosyanın
gerçek hash'ini manifest ile karşılaştırır ve 8 beklenen dosyanın tamamının
listede olduğunu doğrular. Dosyalar test sırasında salt-okunur erişilir; yazma
yalnız tempdir'e yapılır.

```text
b863c28bab0a43b16df54b03528a56425494847fba0e61334f7d5b4f4dbb9c0e  01_Bos_Sinav_Kagidi.pdf
9ade83f79d1ca0aadeece9543e1ef771a8048975832bffed7507402bc49d0ed0  02_Doldurulmus_Ornek_Kagit.pdf
837e68375355581643d9871969a93bbc0d09aa128a6a913cd724f8d5cb4128b8  03_Doldurulmus_Tarama_Varyanti.pdf
d86692b36f6c530c319fecd1a5ba4a9577934ae172341bb615063ff9106e5665  04_Cevap_Anahtari_ve_Rubrik.pdf
df63fb2e8536b8ae00e123621313d648b5953f5df892191e213537b4723c75f9  05_Rubrik_Golden.json
8c7c73a6244c3a44b934cb9bdfd7c4543b3bb2c2ebb4936648b5ebd610b432b6  06_Golden_Set_Beklentileri.json
2c5d037ee5777427deb9c9916484bfd91fc4b8989bae524f8d4945363e753864  07_CodeX_Teknik_Borc_Kapanis_Promptu.md
598fbce1bfe0a6f3df8daed1decb38d224d073167d676add67d6c37ce239a001  README.md
```

## 3. Ölçüm metodolojisi

Tüm metrikler saf fonksiyonlardır (`src-tauri/src/services/golden_ocr_metrics.rs`).
Üretim OCR metnini değiştirmez; yalnız karşılaştırma için normalize eder.

- **Normalizasyon:** `text_normalization::normalize_for_comparison` — NFKD +
  Türkçe case folding (`I/İ/ı → i`), noktalama → boşluk, whitespace sıkıştırma.
- **Levenshtein:** karakter seviyesi edit mesafesi (Unicode char, byte değil).
- **CER (Character Error Rate):** normalize edilmiş referans üzerindeki edit
  mesafesi / normalize referans karakter sayısı. Boş referansta `0.0`.
- **WER (Word Error Rate):** kelime seviyesi edit mesafesi / referans kelime
  sayısı.
- **Critical-token error:** referansta bulunması zorunlu kritik token'ların
  (isim, sayı, alan değeri) hipotezde yok olma sayısı (`CriticalTokenReport`).
- **Printed-question leakage:** OCR metninde basılı soru yönergesinin içerik
  kelimelerinin sızması. Türkçe fonksiyon kelimeleri hariç; eşik: en az 2 kelime
  VE oran ≥ 0.5 (`LEAKAGE_MIN_OVERLAP_WORDS`, `LEAKAGE_MIN_OVERLAP_RATIO`).
- **Structured-field exact match:** normalize edilmiş birebir eşleşme
  (`structured_field_exact_match`); sıra + uzunluk zorunlu
  (`structured_fields_all_exact`).
- **Yüzdelikler:** `percentile` / `percentile_p50` / `percentile_p95` (en yakın
  sıra, sıralı kopya üzerinde).

### Rapor DTO

`GoldenOcrBenchmarkReport` (`golden_ocr_metrics.rs`) — serde kontratı:

```text
schemaVersion, examId, generatedAt, modelRuntime (available|needs_model_runtime),
corpusManifestSha256, perQuestion[GoldenQuestionMetric], aggregate[GoldenAggregateMetric]
```

`GoldenQuestionMetric` model bağımlı alanlar (`cer`, `wer`,
`criticalTokenMissing`, `printedQuestionLeakage`, `structuredExactMatch`,
`durationMsP50/P95`, `imageTokenCount`, `modelCallCount`, `retryCount`,
`peakMemoryBytes`) dahil `None` olabilir; model yokken rapor yapısal/preview
raporudur, PASS iddiası değildir. p50/p95 süre, image token, çağrı, retry ve
peak memory alanları bu DTO aracılığıyla Faz 7+ benchmark runner'ı tarafından
doldurulur.

## 4. Model gerektirmeyen testler (bu ortamda ÇALIŞTI)

`cargo test --manifest-path src-tauri/Cargo.toml --test golden_tymm_tde_001`
→ **12 passed, 0 failed** (~5s; poppler/JXA renderer mevcut). FAZ 7'de 3 test
eklendi (deskew/registration/DPI sınır doğrulaması).

| Test | Ne doğrular |
|------|------------|
| `manifest_sha256_verifies_all_golden_files` | 8 dosyanın SHA-256'ı manifest ile eşleşir; eksik dosya yok |
| `golden_contracts_parse_and_expected_score_is_consistent` | 05+06 ayrışır; rubrik toplamı 100; beklenen toplam 80; 80=Q1..Q6 toplamı |
| `blank_exam_is_renderable_and_has_four_pages` | 01: 4 sayfa, tümü render edilebiliyor |
| `filled_exam_regions_crop_within_bounds` | 02: 06 bbox'ları [0,1] içinde; crop sayfa sınırları içinde, boş değil |
| `scanned_variant_is_valid_and_renderable_with_bounded_crops` | 03: geçerli 4 sayfa PDF; bbox'lar render sınırları içinde |
| `q1_has_primary_and_continuation_regions_in_document_order` | Q1 iki bölge: `Primary`(sayfa 1) + `Continuation`(sayfa 2), sıralama korunur |
| `golden_answer_types_match_structured_answer_variants` | Q1-Q6 answer_type'ları domain `AnswerType` + `StructuredAnswer` varyantıyla uyumlu; yanlış varyant fail-closed |
| `metric_functions_are_clean_against_golden_ground_truth` | CER/WER=0 (özdeş); Q2 alanları tam eşleşir; Q6 basılı yönerge sızıntısı yok |
| `benchmark_report_dto_documents_needs_model_runtime` | DTO serde kontratı; `modelRuntime=needs_model_runtime` |
| `scanned_variant_deskew_accepts_every_page_within_golden_bounds` | 03: 4 sayfanın tamamı deskew'e kabul; ölçülen açılar 0–0.75° |
| `scanned_variant_registration_deviation_stays_within_golden_bounds` | 03: sistematik sapma üretim eşiği 0.12 altında (ölçülen ≤ 0.01) |
| `golden_render_dpi_normalizes_to_fixed_ocr_target` | 144 DPI girdi → 300 DPI hedef; 96–600 aralık doğrulaması |

Birim testleri: `cargo test --lib golden_ocr_metrics` → **16 passed** (Levenshtein,
CER, WER, critical-token, leakage pozitif/negatif/stopword, structured exact,
percentile, DTO round-trip).

## 5. Faz 7+ benchmark sonuçları — gerçek model (ÇALIŞTI)

Gerçek model üzerinde golden 03 (tarama varyantı) benchmark'ı çalıştırıldı. Bu bölümdeki
`NEEDS_MODEL_RUNTIME` durumu kaldırıldı; model-dependent alanlar gerçek ölçümlerle
dolduruldu. Rapor DTO'su `modelRuntime=available`.

### 5.1 Ortam ve sunucu

| Parametre | Değer |
|-----------|-------|
| Tarih | 2026-08-06 |
| Model | Gemma 4 12B (IT), `gemma-4-12B-it-qat-UD-Q4_K_XL.gguf`, multimodal (mmproj) |
| Sunucu | llama-server (OpenAI uyumlu `POST /v1/chat/completions`), `127.0.0.1:8080`, `--temp 0 --top-k 1 --reasoning off` |
| İstek | `temperature 0`, `top_k 1`, `max_tokens 4096`, `response_format json_object`, `stream false` |
| Render | golden 03, `pdftoppm -r 300` (A4 → 2481×3508), deskew (üretim fonksiyonu) |
| Pipeline | üretim prompt kontratı `student_answer_ocr_v4_typed_user_data` + `LlamaServerGateway::extract_student_answer_ocr` (üretim parser'ı) + deterministik preprocess varyant seçimi + üretim JPEG hazırlığı (`ModelInputImageService::prepare_inputs`) |
| Metrikler | `golden_ocr_metrics` (CER/WER/critical-token/leakage/structured exact) |
| Rapor | `benchmark_report.json` (DTO `rubrika.golden.ocr.benchmark.v1`, `modelRuntime=available`), soru bazlı ham yanıtlar `results/{q}/` — çalışma dosyaları tempdir'de tutuldu; golden dosyaları salt-okunur |

### 5.2 Soru tablosu (scanned variant, 300 DPI)

| Soru | Tip | Sayfa | CER | WER | Structured exact | Kritik token eksik | Basılı yönerge sızıntısı | Süre (sn) |
|------|-----|-------|-----|-----|------------------|--------------------|--------------------------|-----------|
| Q1 | open-ended (iki bölge) | 1+2 | **0.0** | **0.0** | — | 0 | hayır | 51.7 |
| Q2 | table | 2 | **0.0** | **0.0** | false \* | 0 | hayır | 49.4 |
| Q3 | matching | 3 | **0.0** | **0.0** | true | 0 | hayır | 34.4 |
| Q4 | correction table | 3 | 2.14 † | 1.8 † | false \* | 0 | hayır | 79.7 |
| Q5 | grammar analysis | 4 | **0.0** | **0.0** | false \* | 0 | hayır | 92.1 |
| Q6 | open-ended (şiir) | 4 | **0.0** | **0.0** | — | 0 | hayır | 83.0 |

Özet (aggregate): CER p50=0.0, p95=2.14; WER p50=0.0, p95=1.8; toplam 6 model çağrısı, 0 retry.
Image token sayısı ve peak memory bu sunucu/`ModelInputImageService` sürümünde raporlanmıyor
(`None`); p50/p95 süre tek çalıştırma için her soruda ölçülene eşittir.

- \* **Structured-schema bulgusu:** model metin OCR'ında kusursuz (Q1/Q2/Q3/Q5/Q6 CER=0.0, kritik
  token eksik 0), fakat typed `kind`-etiketli `structuredAnswer`'ı tablo/eşleştirme/düzeltme/dil
  bilgisi için doğru şemada üretmiyor; üretim parser'ı `structured_answer_invalid` → `needsReview`
  olarak fail-closed işaretler. Bu, tasarlanmış davranıştır (öğretmen onay kuyruğu); OCR metni
  kaybolmaz. Yapısal sorular için şema-uyumlu çözüm (grammar/json_schema kısıtlı decode veya
  prompt iyileştirme) Faz 8 adayıdır.
- † Q4 CER, metriğin tasarımından kaynaklıdır: referans yalnız 3 düzeltilmiş cümleyi içerir, model
  ise tablonun tamamını (yanlış + düzeltilmiş + etiketler) transkripsiyon etmiştir. Üç düzeltme de
  metinde mevcuttur (kritik token eksik 0). İçerik doğru, kapsam üretim promptunun "yalnız öğrenci
  cevabı" yönergesinden daha geniştir.

### 5.3 Bulgular ve kararlar

1. **Korpus bbox y-ekseni kaynağı hatası (yeni, TD-32 yakın) — ÇÖZÜLDÜ (Faz 9):**
   `06_Golden_Set_Beklentileri.json` bbox'ları PDF doğal (alt-sol başlangıçlı) y koordinatı kullanır;
   üretim crop matematiği (`crop_rect_normalized`) ve domain `NormalizedBBox` sözleşmesi üst-sol
   başlangıçlı y bekler. Bbox'lar aynen `crop_rect_normalized`'a beslenirse 5/6 soruda yanlış bölge
   kırpılır (Q1, Q2'nin alanlarını okur; Q3/Q4 içerikleri takas olur; Q6 boş görünür; yalnız Q5
   tesadüfen hizalanır). **Golden dosyaları değiştirilmedi.** Faz 9'da dönüşüm tek paylaşılan
   fonksiyona alındı (`golden_ocr_metrics::corpus_bbox_bottom_left_y_to_top_left`, `y_top =
   1 − (y_bottom + h)`); hem benchmark runner hem `region_from_golden`/`regions_for_page` test
   yardımcıları ve 06 bbox'larını `crop_rect_normalized`'a besleyen tüm golden tüketicileri bu
   dönüşümü uygular. Regresyon: `corpus_bboxes_are_converted_to_top_left_before_cropping`
   (entegrasyon testi) + 3 birim testi (bkz. bölüm 8).
2. **Registration gating kararı: EVET — üretim hattına bağlandı (TD-21 kalıntısı kapandı).**
   Golden 03 (kasıtlı eğik/düşük kontrast tarama) sayfa bazında registration sapması 0.004–0.010
   (sayfa 4 maks. 0.035) ölçer; eşik 0.12 → sahte ret riski yok. Gate yalnızca tüm cevap ızgarasını
   sistematik kayan sayfaları reddeder; boş sayfalar muaf. Kod:
   `student_answer_crop_service::build_sources` Production dalında
   `validate_page_registration` (ölçüm + `validate_registration(DEFAULT_MAX_REGISTRATION_DEVIATION)`),
   typed `RegistrationOutOfRange` fail-closed; regresyon testi
   `production_rejects_systematically_misregistered_page_and_accepts_aligned_one`.
3. **Q3 canonical anahtar kararı: rubrik `answer_key` (`5-E`) canonical'dır.** Öğrenci kâğıdı
   gerçekten "1-A, 2-B, 3-C, 4-D, 5-A" gösterir (OCR CER=0.0). `06 ocr_ground_truth.q3` bu
   öğrenci işaretinin OCR'ıdır (yanlış `5-A`), anahtar değildir; `05 answer_key` doğru anahtardır.
   Rubrik anahtarıyla öğrenci 4/5 eşleşme = 12/15 puan alır ve `expected_scoring.q3 = 12` ile birebir
   tutarlıdır. Böylece bölüm 6'daki "bilinen tutarsızlık" **tutarsızlık değil, öğrencinin 5. maddede
   yaptığı gerçek hatadır**; 06'nın q3 alanı doğru biçimde OCR ground truth'udur.
4. **Eşik durumu (önerilen kalite kapıları):** vector/tarama CER≤0.05 ve WER≤0.08 kapıları Q1/Q2/Q3/Q5/Q6
   için sağlanır (0.0). Q4 CER/WER referans kapsamı nedeniyle kapıyı karşılamaz (yukarıdaki † notu).
   `printed_question_leakage` tüm sorularda `false` (kapı sağlandı). `structured_field_exact_match`
   kapısı (1.0) yapısal sorularda model şema-uyumsuzluğu nedeniyle sağlanamadı (bulgu 5.2 \*).
   Deterministik scoring (beklenen toplam 80) bu fazda yapısal şema parse'ına bağlı olduğundan
   çalıştırılmadı; yapısal çıktı uyumu sağlandığında Faz 8'de ölçülür.

### 5.4 Faz 7 notu (OCR görüntü hattı altyapısı — geçmiş kayıt)

Faz 7, `NEEDS_MODEL_RUNTIME` listesini gerçek OCR metni üretmeden destekleyen altyapıyı kurmuştu
(deskew/registration/DPI saf fonksiyonları, deterministik preprocess varyant seçimi, OCR persistence
atomicity). Bu fazda o altyapı gerçek model üzerinde çalıştırıldı: deskew sayfa başına 0–0.75°,
registration 0.004–0.010, preprocess varyant seçimi deterministik (`CleanGrayscale`/`Original`
gerekçeli). O altyapı kaydı doğrulandı; bu fazda PASS iddiası üretilen metriklerle sınırlıdır.

## 6. Golden veri notları

- **Q3 madde 5 — ÇÖZÜLDÜ (Faz 7+):** `05_Rubrik_Golden.json` `answer_key["5"] = "E"`, fakat
  `06_Golden_Set_Beklentileri.json` `ocr_ground_truth.q3 = "1-A, 2-B, 3-C, 4-D, 5-A"`. Faz 7+
  benchmark'ı bunun **kendi içinde tutarsızlık değil, öğrencinin gerçek hatası** olduğunu kanıtladı:
  öğrenci kâğıdı gerçekten `5-A` işaretler (OCR CER=0.0); 06'nın q3 alanı doğru biçimde "öğrencinin
  yazdığı işaretlerin OCR ground truth'u", 05 `answer_key` ise doğru cevap anahtarıdır. **Rubrik
  anahtarı canonical kabul edildi** → öğrenci 4/5 = 12/15, `expected_scoring.q3 = 12` ile birebir
  tutarlı. Golden dosyaları değiştirilmedi.
- **Bbox y-ekseni kaynağı — ÇÖZÜLDÜ (Faz 9):** `06` bbox'ları PDF alt-sol y kaynağı kullanır;
  üretim crop matematiği üst-sol bekler. Aynen beslenirse bölgeler içerikle çakışmaz (yalnız Q5
  hizalanır). Dönüşüm tek paylaşılan fonksiyondadır (`golden_ocr_metrics::corpus_bbox_bottom_left_y_to_top_left`)
  ve runner + tüm golden test tüketicileri uygular (bölüm 8). Golden dosyaları değiştirilmedi;
  üretim crop hattı golden bbox verisi tüketmediği için etkilenmez.
- Beklenen puan 80 = Q1..Q6 toplamıyla tutarlıdır (rubrik anahtarı puan hesabına bu korpus sürümünde
  doğrudan katılmaz; deterministik scoring Faz 8 kapsamı — yapısal şema uyumu sağlandığında).

## 7. Faz 7+ eşikleri (önerilen)

`06_Golden_Set_Beklentileri.json` `quality_gates` alanından:

| Kapı | Hedef |
|------|-------|
| Vector render CER | ≤ 0.01 |
| Vector render WER | ≤ 0.02 |
| Tarama varyantı CER | ≤ 0.05 |
| Tarama varyantı WER | ≤ 0.08 |
| Printed question leakage | yasak (`false`) |
| Structured field exact match | 1.0 (tüm alanlar) |
| Beklenen toplam puan | tam 80 |

## 8. Corpus koordinat konvansiyonu

`06_Golden_Set_Beklentileri.json` `regions[].bbox_normalized` koordinatları **PDF kullanıcı
uzayında** saklanır: `[x, y, width, height]`, burada `y` alt-sol kaynaklıdır (sayfanın altından
yukarı doğru büyür, `y_bottom`). Üretim crop matematiği `crop_rect_normalized` ve domain
`NormalizedBBox` sözleşmesi ise **üst-sol kaynaklı** `y` bekler (görüntü koordinatı: `y` yukarıdan
aşağı büyür).

Bu bir corpus kusurudur (golden dosyaları düzeltilmez; tüketen kod dönüşümü uygular). Dönüşümün
tek otoritesi paylaşılan saf fonksiyondur:

```text
corpus_bbox_bottom_left_y_to_top_left(y_bottom, height) = clamp(1 - (y_bottom + height), 0, 1)
```

Uygulama noktaları (Faz 9):

- `src-tauri/src/services/golden_ocr_metrics.rs` — fonksiyon + 3 birim testi.
- `src-tauri/src/bin/golden_ocr_benchmark.rs` `normalize_bbox` — runner crop'ları bu dönüşümle
  üretilir (bölüm 5.2 ölçümleri bu konvansiyonla alınmıştır).
- `src-tauri/tests/golden_tymm_tde_001.rs` `region_from_golden` / `regions_for_page` ve
  `filled_exam_regions_crop_within_bounds` / `scanned_variant_is_valid_and_renderable_with_bounded_crops`
  içindeki inline bbox yapıları — golden bbox'ları `crop_rect_normalized`'a beslemeden önce dönüştürür.
- Regresyon testi `corpus_bboxes_are_converted_to_top_left_before_cropping`: korpustaki **her** bbox'ın
  dönüşümden geçtiğini (kimlik dönüşümü olmadığını) kanıtlar.

Üretim etki analizi: golden 06 bbox'ları üretim crop hattına (`student_answer_crop_service`) **girmez**;
üretim yalnız öğretmen tarafından UI üzerinden tanımlanmış üst-sol crop şablonlarını
(`studentAnswerCropTemplate`) okur. Dolayısıyla konvansiyon farkı üretim davranışını etkilemez;
dönüşüm yalnız benchmark runner ve golden test tüketicileri için gereklidir.
