# Rubrika v3 Local LLM Pipeline Uygulama Planı

Tarih: 2026-08-03  
Kaynak rapor: `docs/girdi_raporu_ortak_sonuc.md:1-892`  
Kapsam: React + TypeScript + Tauri/Rust + llama.cpp local LLM pipeline  
Durum: Yalnız planlama; bu çalışma sırasında kaynak kod değiştirilmemiştir.

Dosya:satır referansları 2026-08-03 tarihli çalışma ağacı snapshot'ına aittir; uygulama PR'ı başlarken ilgili semboller yeniden bulunmalı ve plan referansları rebase edilmelidir.

## 1. Amaç, kapsam ve değişmezler

Bu plan, girdi raporundaki iddiaları mevcut kodla eşleştirir ve raporun Faz 1–4 sırasını uygulanabilir iş paketlerine dönüştürür. Her faz tek başına merge edilebilir, geri alınabilir ve sonraki faz tamamlanmadan üretime alınabilir olmalıdır. Faz geçişi yalnız test ve ölçüm kapıları geçildikten sonra yapılmalıdır.

Plan boyunca aşağıdaki mimari kurallar değişmez kabul edilir:

1. UI eşik, workflow readiness, scoring kabulü veya auto-accept kararı üretmez; backend DTO/snapshot sonucunu gösterir.
2. Her kalıcı state değişikliği typed Tauri backend command üzerinden yapılır ve `ProjectStore` tek yazardır.
3. Render, OCR, scoring, analiz, benchmark ve model startup gibi uzun işlemler job olarak çalışır; command yalnız job kimliği döndürür.
4. Model transport, timeout, empty/reasoning-only çıktı, parse ve schema hataları `AppError` veya reviewable domain sonucu olur; crash ya da normal sıfır puan olmaz.
5. QEP `frozen_for_scoring` kapısı backend'de korunur. OCR/scoring kolaylığı için bypass, test flag'i veya UI fallback'i eklenmez.
6. Placeholder içerik rubrik, cevap anahtarı veya anchor olarak kabul edilmez. Mevcut kontrol noktaları `src-tauri/src/domain/rubric.rs:83-106` ve `src-tauri/src/domain/rubric.rs:137-238` zayıflatılmaz.
7. Teacher-facing UI teknik kod göstermez; teknik ayrıntı diagnostics panelinde kalır.
8. Mevcut proje şeması değişecekse versioned, tek yönlü migration ve migration testi aynı iş paketinde teslim edilir.
9. Benchmark sonucu olmadan KV cache, thread/batch, MTP, image budget, auto-accept veya yeni HTR production varsayılanı değiştirilmez.

## 2. Kod doğrulama matrisi

Durum anlamları:

- **Doğrulandı:** İddia mevcut uygulama yolunda doğrudan kod kanıtına sahip veya iddia edilen eksiklik doğrudan akış/arama ile gösterilebiliyor.
- **Doğrulanamadı:** Kod rapordaki mevcut-durum iddiasıyla çelişiyor ya da gerekli benchmark kanıtı repoda yok. Bu durumlarda sonuç uydurulmamıştır.

| No | Rapordaki iddia | Durum | Somut kod kanıtı ve değerlendirme |
|---:|---|---|---|
| 1 | Local LLM pipeline dokuz işlevsel çağrı ve bir completion probe içeriyor. | Doğrulandı | `ModelGateway` use-case yüzeyi `src-tauri/src/services/model_gateway.rs:13-48`; yazılı/speaking scoring ayrımı `src-tauri/src/services/llama_server_gateway.rs:1175-1214`; completion probe `src-tauri/src/services/llama_server_gateway.rs:248-272`. |
| 2 | Completion probe hot path'te gerçek generation maliyeti yaratıyor. | Doğrulandı | Probe promptu `src-tauri/src/services/llama_server_gateway.rs:257`; lease sırasında completion doğrulaması `src-tauri/src/services/model_process_manager.rs:1446`; aynı acquire akışındaki ikinci status probe `src-tauri/src/services/model_process_manager.rs:1476`; managed startup döngüsü `src-tauri/src/services/model_process_manager.rs:704-709`. |
| 3 | Readiness ve lease kontrolleri tekrarlanıyor; sabit 200 ms bekleme var. | Doğrulandı | Önce status sonra lease `src-tauri/src/services/model_runtime_service.rs:173-218`; scoring preflight ve worker acquire tekrarı `src-tauri/src/services/scoring_service.rs:122-160` ve `src-tauri/src/services/scoring_service.rs:330-339`; OCR sabit bekleme `src-tauri/src/services/student_answer_ocr_service.rs:862-867`. Startup lock mevcut (`src-tauri/src/services/model_process_manager.rs:1374`), fakat tek hazır-lease kontratı değil. |
| 4 | `needs_review=true` sonuç `scoringApplied=true` kalıp frontend final toplamına girebiliyor. | Doğrulandı | `scoring_applied` hesabı model `needs_review` değerini dışlamıyor: `src-tauri/src/services/scoring_service.rs:475-477`; review daha sonra ekleniyor: `src-tauri/src/services/scoring_service.rs:483-553`; domain'de provisional/accepted/final ayrımı yok: `src-tauri/src/domain/scoring.rs:55-105`; frontend toplamı yalnız `scoringApplied` kontrol ediyor: `src/pages/scoringViewModel.ts:77-93`. |
| 5 | Criterion completeness case-insensitive iken normalize aşaması exact title kullanıyor. | Doğrulandı | Normalize eşleşmesi ID veya exact başlık: `src-tauri/src/services/scoring_service.rs:729-761`; completeness/contract kontrolü `eq_ignore_ascii_case`: `src-tauri/src/services/scoring_service.rs:772-814`. |
| 6 | Dinamik ve güvenilmeyen veriler birçok use-case'te system prompt içine gömülüyor. | Doğrulandı | Soru hedefi `src-tauri/src/services/question_text_service.rs:1240-1255` ve system kullanımı `src-tauri/src/services/llama_server_gateway.rs:322-339`; rubrik `src-tauri/src/services/rubric_extraction_service.rs:1074-1096` / `src-tauri/src/services/llama_server_gateway.rs:422-455`; OCR `src-tauri/src/services/student_answer_ocr_service.rs:2217-2340` / `src-tauri/src/services/llama_server_gateway.rs:614-638`; scoring JSON'u `src-tauri/src/services/scoring_service.rs:886-936` system olarak gönderiliyor `src-tauri/src/services/llama_server_gateway.rs:1187-1214`; speaking evaluation `src-tauri/src/services/speaking_exam_service.rs:2430-2493`; analysis `src-tauri/src/services/analysis_service.rs:586-593`. Speaking cleanup transcript'i user mesajında taşıyor (`src-tauri/src/services/llama_server_gateway.rs:1022-1038`); bu akış tamamen aynı soruna sahip değil. |
| 7 | Structured output ve prompt/schema provenance use-case'ler arasında tutarsız. | Doğrulandı | `response_format` yalnız OCR, speaking cleanup ve scoring yollarında: `src-tauri/src/services/llama_server_gateway.rs:637`, `src-tauri/src/services/llama_server_gateway.rs:1046`, `src-tauri/src/services/llama_server_gateway.rs:1213`; ortak diagnostics tipi istenen fingerprint/version alanlarını taşımıyor: `src-tauri/src/domain/model.rs:217-235`; OCR prompt sürümleri `src-tauri/src/services/student_answer_ocr_service.rs:36-38`; scoring sürümü `src-tauri/src/services/scoring_service.rs:28`; speaking daha kapsamlı provenance kullanıyor: `src-tauri/src/services/speaking_exam_service.rs:52-57` ve `src-tauri/src/services/speaking_exam_service.rs:3403-3434`. |
| 8 | `critical_term_hint` saf OCR issue correction'a cevap anahtarı/rubrik bilgisi sızdırıyor. | Doğrulandı | Hint hazırlanıp gönderiliyor: `src-tauri/src/services/student_answer_ocr_service.rs:662-671`; kaynak expected answer veya rubric criterion label: `src-tauri/src/services/student_answer_ocr_service.rs:2418-2437`; prompt alanı: `src-tauri/src/services/student_answer_ocr_service.rs:2290-2340`. |
| 9 | Backend OCR güven eşiği 0.72, frontend filtresi 0.6. | Doğrulandı | Backend sabiti ve uygulaması `src-tauri/src/services/llama_server_gateway.rs:31-37` ve `src-tauri/src/services/llama_server_gateway.rs:2786-2789`; frontend sabiti ve kullanımı `src/pages/studentAnswerOcrUi.ts:51` ve `src/pages/studentAnswerOcrUi.ts:356-420`. |
| 10 | Türkçe eşleştirme bazı yerlerde ASCII lowercase kullanıyor. | Doğrulandı | Similarity normalizasyonu `to_ascii_lowercase()` kullanıyor: `src-tauri/src/services/llama_server_gateway.rs:2981-2991`. OCR servisindeki başka bir normalizasyon Unicode `.to_lowercase()` kullanıyor: `src-tauri/src/services/student_answer_ocr_service.rs:2135-2148`; bu nedenle tek canonical normalizer yok. |
| 11 | Speaking evaluation halen mmproj yükleyen standard profile'a bağlı. | **Doğrulanamadı** | Kod tersini gösteriyor: speaking rubric profile text-only ve mmproj boş `src-tauri/src/domain/model.rs:768-799`; text-only arg üretimi `src-tauri/src/domain/model.rs:898-927`; servis `SPEAKING_RUBRIC_PROFILE_ID` ve text capability ile lease alıyor `src-tauri/src/services/speaking_exam_service.rs:2031-2044`; evaluation inputunda image yok `src-tauri/src/services/speaking_exam_service.rs:2193-2225`. Burada migration değil regression kilidi planlanmalıdır. |
| 12 | Model giriş JPEG cache'i mevcut dosyayı silip yeniden yazıyor ve content-key kullanmıyor. | Doğrulandı | Sabit long-edge/quality `src-tauri/src/services/model_input_image_service.rs:15-16`; çıktı yalnız sayfa numarasıyla adlandırılıyor, varsa siliniyor ve yeniden encode ediliyor `src-tauri/src/services/model_input_image_service.rs:275-375`; yalnız manifest atomik `src-tauri/src/services/model_input_image_service.rs:425-448`. |
| 13 | UI preview ile OCR source render ayrılmamış; yaklaşık 144 DPI ve 1800 px sınırı var. | Doğrulandı | Crop UI preview artifact'ından yapılıyor `src-tauri/src/services/student_answer_crop_service.rs:63-69`; macOS PDF scale 2.0 `src-tauri/src/services/pdf_service.rs:57-88` (PDF point ölçeğinden yaklaşık 144 DPI çıkarımıdır); model input yalnız downscale ediyor `src-tauri/src/services/model_input_image_service.rs:377-392`. |
| 14 | Canonical crop modeli tek soru için tek region/tek page varsayıyor; çok sayfa doğrudan partial sayılıyor. | Doğrulandı | Tek `page_index_within_submission` ve bbox modeli `src-tauri/src/domain/student.rs:242-270`; crop lookup `.find` ile tek item seçiyor `src-tauri/src/services/student_answer_crop_service.rs:70-75`; `page_numbers.len() > 1` doğrudan partial işaretliyor `src-tauri/src/services/student_answer_crop_service.rs:342-344`. |
| 15 | OCR crop öncesinde registration/deskew/perspective zinciri yok. | Doğrulandı | Preview doğrudan açılıp normalize bbox ile `crop_imm` yapılıyor `src-tauri/src/services/student_answer_crop_service.rs:410-432`; bbox piksel hesabı `src-tauri/src/services/student_answer_crop_service.rs:531-555`. İlgili akışta registration transformu bulunmuyor. |
| 16 | Crop yoksa full page model OCR'a gidiyor; review işareti olsa da production akışında. | Doğrulandı | Full-page fallback tüm submission sayfalarını seçiyor `src-tauri/src/services/student_answer_crop_service.rs:365-374`; bu görseller model çağrısına gönderiliyor `src-tauri/src/services/student_answer_ocr_service.rs:940-963`; review/fallback uyarıları sonradan ekleniyor `src-tauri/src/services/student_answer_ocr_service.rs:971-1053`. |
| 17 | Beş preprocess varyantı topluca üretiliyor, sonra handwriting-enhanced tercih ediliyor. | Doğrulandı | Varyant listesi `src-tauri/src/services/student_answer_ocr_service.rs:41-47`; tüm varyantları üreten döngü `src-tauri/src/services/student_answer_ocr_service.rs:2477-2503`; handwriting seçimi `src-tauri/src/services/student_answer_ocr_service.rs:2505-2542`. |
| 18 | Risk koşullu seçici ikinci OCR geçişi yok. | Doğrulandı | Ana soru akışı her kayıt için tek gateway çağrısı yapıyor `src-tauri/src/services/student_answer_ocr_service.rs:940-963`; risk sinyaliyle ikinci varyant üretip iki sonucu karşılaştıran bir branch bulunmuyor. |
| 19 | `structuredAnswer` typed değil, serbest JSON taşıyor. | Doğrulandı | Model çıktısı `Option<serde_json::Value>`: `src-tauri/src/domain/model.rs:354-372`; canonical student record aynı tipi taşıyor `src-tauri/src/domain/student.rs:419-483`; frontend `unknown`: `src/api/types.ts:1580-1617`. |
| 20 | Deterministik cevap türleri de model scoring yoluna giriyor. | Doğrulandı | Answer type enum'u `src-tauri/src/domain/question.rs:5-22`; scoring tüm soruları dolaşıp OCR gate sonrası model request'i kuruyor ve gateway çağırıyor `src-tauri/src/services/scoring_service.rs:352-455`; answer-type scorer dispatch yok. |
| 21 | Yazılı scoring modelden serbest numeric score alıyor; speaking level/evidence + Rust mapping'e daha yakın. | Doğrulandı | Yazılı prompt numeric `awardedScore` ister `src-tauri/src/services/scoring_service.rs:917-935`, çıktı tipi numeric `src-tauri/src/domain/model.rs:565-575`; speaking prompt doğrudan puanı yasaklayıp level/evidence ister `src-tauri/src/services/speaking_exam_service.rs:2459-2493`, backend level doğrulama ve puan mapping'i yapar `src-tauri/src/services/speaking_exam_service.rs:2990-3089`. |
| 22 | Mevcut scoring hash'i model/runtime/prompt/schema/policy/calibration fingerprint'lerini kapsamıyor ve candidate cache yok. | Doğrulandı | Hash alanları QEP/project/question/rubric/OCR girdileriyle sınırlı `src-tauri/src/domain/scoring.rs:135-280`; hash'ler kayda yazılıyor `src-tauri/src/services/scoring_service.rs:566-570`, fakat aynı fingerprint için model çağrısını atlayan cache lookup yok. |
| 23 | Teacher-approved anchor ve benzer cevap kümesi altyapısı yok. | Doğrulandı | Scoring domain ve servis akışı `src-tauri/src/domain/scoring.rs:55-280` ve `src-tauri/src/services/scoring_service.rs:352-570` kayıtları bağımsız değerlendiriyor; anchor/cluster/candidate comparison modeli veya lookup'u bulunmuyor. |
| 24 | Öğretmen OCR gate'i halen scoring öncesinde zorunlu; benchmark olmadan gevşetilmemeli. | Doğrulandı | Domain readiness `TeacherApproved` ve `!needs_review` ister `src-tauri/src/domain/scoring.rs:432-437`; servis worker aynı koşulu tekrar doğrular `src-tauri/src/services/scoring_service.rs:379-407`. QEP frozen kontrolü `src-tauri/src/domain/scoring.rs:414-441`. |
| 25 | Question extraction her hedef soru için tüm sayfa inputlarını tekrar kullanıyor. | Doğrulandı | Tüm document model inputları hazırlanıyor `src-tauri/src/services/question_text_service.rs:512-543`; her target request bunların tamamını clone ediyor `src-tauri/src/services/question_text_service.rs:680-734`. |
| 26 | Rubric extraction her hedef soru için tüm sayfaları tekrar gönderiyor. | Doğrulandı | Tüm sayfalar render/prepared ediliyor `src-tauri/src/services/rubric_extraction_service.rs:496-560`; her target request tüm image inputlarını clone ediyor `src-tauri/src/services/rubric_extraction_service.rs:562-605`. |
| 27 | Rubric extraction şeması canonical modeldeki partial credit, zero-score ve common mistakes alanlarını istemiyor. | Doğrulandı | Prompt şeması max points/expected answer/criteria ile sınırlı `src-tauri/src/services/rubric_extraction_service.rs:297-328`; canonical alanlar `src-tauri/src/domain/rubric.rs:41-64`. |
| 28 | İlk rubric parse hatası aynı multimodal isteği yeniden gönderebiliyor. | Doğrulandı | Retry aynı request'i clone ediyor `src-tauri/src/services/rubric_extraction_service.rs:976-1023`; strict prompt yalnız metin eki `src-tauri/src/services/llama_server_gateway.rs:1430-1438`; gateway image payload'ı tekrar gönderiyor `src-tauri/src/services/llama_server_gateway.rs:408-475`. |
| 29 | Analysis modele anonim agregalar gönderiyor, fakat çıktı serbest metin ve metric reference içermiyor. | Doğrulandı | Prompt aggregate metriklerle sınırlı `src-tauri/src/services/analysis_service.rs:570-593`; domain raporu `Option<String>` saklıyor `src-tauri/src/domain/analysis.rs:49-71`; gateway structured response format kullanmıyor `src-tauri/src/services/llama_server_gateway.rs:1099-1127`. |
| 30 | Strict-local privacy policy yok; External mode öğrenci verisini arbitrary endpoint'e gönderebilir. | Doğrulandı | Yalnız External/Managed mode var `src-tauri/src/domain/model.rs:6-41`; external default loopback olsa da arbitrary base URL profile'dan geliyor `src-tauri/src/domain/model.rs:743-755`; gateway client'ta redirect/proxy yasağı yok `src-tauri/src/services/llama_server_gateway.rs:83-97`; UI/API yalnız mode gösteriyor `src/api/types.ts:1338-1362`; mode mutation command'ı `src-tauri/src/commands/model_commands.rs:73-81`. |
| 31 | Dynamic image/output token budget yok. | Doğrulandı | Sabit token limitleri `src-tauri/src/services/llama_server_gateway.rs:31-35`; use-case requestleri sabit limit kullanıyor, örneğin OCR `src-tauri/src/services/llama_server_gateway.rs:614-638`; image long-edge policy sabit `src-tauri/src/services/model_input_image_service.rs:15-16`. |
| 32 | OCR provenance gerçek image metadata'nın bir kısmını taşısa da istenen tam fingerprint/version seti eksik. | Doğrulandı | Image metadata ve diagnostics temel alanları `src-tauri/src/domain/model.rs:170-235`; OCR generation/record provenance `src-tauri/src/domain/student.rs:214-238` ve `src-tauri/src/domain/student.rs:419-483`; schema/policy/model/runtime/sampling fingerprint seti tamamlanmamış. |
| 33 | KV cache `q8_0` yerine turbo3/turbo4 daha iyi olacaktır. | **Doğrulanamadı** | Standard runtime args mevcut q8 ayarını sabitliyor `src-tauri/src/domain/model.rs:811-854`; OCR CER/WER, RAM, latency ve review oranını birlikte karşılaştıran sonuç/fixture repoda yok. Üstünlük yalnız benchmark ile belirlenebilir. |
| 34 | Thread, batch, ubatch ve parallel için önerilen farklı değerler daha iyi olacaktır. | **Doğrulanamadı** | Mevcut sabitler `src-tauri/src/domain/model.rs:823-828`; hedef donanım ve golden workload karşılaştırması yok. Production default önerilemez. |
| 35 | MTP/speculative decoding bu pipeline'ı iyileştirir. | **Doğrulanamadı** | Uyumlu assistant head, multimodal fallback davranışı ve kalite/latency benchmark sonucu repoda yok. Koddan yarar veya güvenlik sonucu çıkarılamaz. |
| 36 | Özel HTR motoru mevcut pipeline'dan daha iyi olacaktır. | **Doğrulanamadı** | Mevcut OCR yolu llama.cpp vision gateway'dir (`src-tauri/src/services/student_answer_ocr_service.rs:940-963`); karşılaştırmalı HTR fixture/benchmark yok. Önce render/registration/crop etkisi ölçülmelidir. |

**Doğrulama özeti:** 36 iddianın 31'i kodda doğrulandı, 5'i doğrulanamadı. Doğrulanamayanlar speaking'in mevcut profile durumu ile dört benchmark-bağımlı performans iddiasıdır.

## 3. Ortak teslim ve test sözleşmesi

Her fazda aşağıdaki teslim koşulları geçerlidir:

1. Yeni/yenilenen command için typed Rust input/output, TypeScript DTO, `docs/API_CONTRACTS.md:16-448` güncellemesi, command contract testi ve structured error bulunur.
2. Yeni model çağrısı yalnız `ModelGateway` üzerinden yapılır. Prompt, schema ve validation frontend'e taşınmaz.
3. Yeni uzun işlem `JobManager` üzerinden progress/failure eventleri üretir ve correlation ID'yi request → job → model diagnostics → project log zincirinde korur.
4. Project state/artifact metadata commit'i yalnız `ProjectStore` mutasyonu içinde, atomik ve conflict-aware yapılır.
5. Kaynak değişikliği yapılacak ilk fazda gerçek smoke entrypoint tanımlanır: mevcut `package.json:4-18` ve `scripts/tauri-dev.mjs:1` üzerinde `npm run tauri:dev -- --smoke` argümanı non-interactive, bounded ve exit-code üreten hale getirilir. Şu an bağımsız bir smoke scripti yapılandırılmış değildir; UI/command değişikliği smoke olmadan tamamlanmış sayılmaz.
6. Her fazın son kalite kapısı:

   ```bash
   npm run typecheck
   npm run lint
   npm test
   npm run cargo:fmt
   npm run cargo:clippy
   npm run cargo:test
   npm run tauri:dev -- --smoke
   ```

7. Benchmark işaretli maddelerde ayrıca anonim fixture manifesti, baseline, candidate, donanım bilgisi ve şu ortak metrikler kaydedilir: model çağrısı/job, p50/p95 süre, prefill/decode, image/input/output token, CPU, peak RAM, retry, OCR CER/WER, kritik terim hata oranı, schema başarı oranı, exact-repeat, teacher review ve teacher correction oranı. Yalnız tokens/second karar için yeterli değildir.

---

# Faz 1 — Hata, güvenlik ve gereksiz maliyet

## Faz hedefi

Model kalitesini bilinçli biçimde değiştirmeden hot-path maliyetlerini, doğrudan scoring doğruluk hatalarını, prompt/schema güvenlik açıklarını, local-only sınırını ve cache semantiğini düzeltmek.

## Faz kapsamı ve bağımsızlık sınırı

- P0 maddeler bu fazda tamamlanır; daha sonraki OCR/scoring mimarisi beklenmez.
- Persisted şema değişiklikleri geriye uyumlu optional/default alanlarla ve tek yönlü migration ile yapılır.
- Faz 2–4 feature'ları kapalı kalır; mevcut teacher approval ve QEP frozen kapıları aynen korunur.
- Phase exit için eski projeler açılmalı, mevcut OCR/scoring kayıtları migration sonrası aynı anlama sahip olmalı ve final toplam artık review kayıtlarını içermemelidir.

## Değiştirilecek ana dosyalar

`src-tauri/src/services/model_process_manager.rs:1355-1528`, `src-tauri/src/services/model_runtime_service.rs:173-218`, `src-tauri/src/services/llama_server_gateway.rs:248-2991`, `src-tauri/src/services/scoring_service.rs:122-936`, `src-tauri/src/domain/scoring.rs:55-441`, `src-tauri/src/domain/model.rs:6-235`, `src-tauri/src/services/student_answer_ocr_service.rs:662-2542`, `src-tauri/src/services/model_input_image_service.rs:275-448`, `src/pages/scoringViewModel.ts:77-97`, `src/pages/ScoringPage.tsx:177-220`, `src/pages/studentAnswerOcrUi.ts:51-420`, `src/api/types.ts:594-649`, `src/api/types.ts:1338-1362`, `src/api/commands.ts:1003-1112`, ilgili command/modül/test ve dokümantasyon dosyaları.

## Uygulama maddeleri

### 1.1 Completion probe'u hot path'ten kaldır — P0

- **Değişiklik:** `acquire_lease` ve normal domain job'ları yalnız process identity + bounded `/health` readiness kullanmalı. Completion generation yalnız açık `probe_model_server`/doctor/benchmark çağrısında kalmalı. Status DTO'da `healthVerifiedAt` ile `completionProbeVerifiedAt` ayrı gösterilmeli.
- **Dosyalar:** `src-tauri/src/services/model_process_manager.rs:704-709`, `src-tauri/src/services/model_process_manager.rs:1355-1528`, `src-tauri/src/services/llama_server_gateway.rs:248-272`, `src-tauri/src/commands/model_commands.rs:23-31`, `src/api/types.ts:1338-1362`, `docs/MODEL_GATEWAY.md:9-12`, `docs/MODEL_RUNTIME_OWNERSHIP.md:88-99`.
- **Dikkat:** `/health` başarılı olmadan lease verilmez; fakat health ile model-output kalitesi aynı şeymiş gibi raporlanmaz. Manual probe structured `AppError` üretir ve domain state'i değiştirmez.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml model_process_manager`; yeni spy-gateway testi normal acquire sırasında completion endpoint çağrısının `0`, manual probe sırasında `1` olduğunu doğrular.

### 1.2 Runtime readiness ve lease'i tek backend akışında birleştir — P0

- **Değişiklik:** `acquire_ready_runtime_lease(profile, consumer, operation, correlation_id)` tek public servis kontratı olur. Aynı job context'i verified runtime instance/fingerprint taşır; worker yeniden preflight yapmaz. Sabit 200 ms kaldırılıp startup single-flight lock altında deadline/backoff'lu health wait uygulanır.
- **Dosyalar:** `src-tauri/src/services/model_runtime_service.rs:173-218`, `src-tauri/src/services/model_process_manager.rs:1355-1528`, `src-tauri/src/services/student_answer_ocr_service.rs:862-867`, `src-tauri/src/services/scoring_service.rs:122-160`, `src-tauri/src/services/scoring_service.rs:330-339`, `docs/MODEL_RUNTIME_OWNERSHIP.md:72-99`, `docs/API_CONTRACTS.md:422-428`.
- **Dikkat:** Startup single-flight mevcut lock'u genişletmeli, paralel ikinci process başlatmamalı. Lease RAII/finally yolunda release edilmeli; cancellation, timeout ve panic olmayan hata yolları test edilmelidir.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml model_runtime_service`; 50 concurrent acquire testi tek startup, tek readiness dizisi, benzersiz lease ve sıfır negative ref-count doğrular.

### 1.3 `needs_review` puanlarını final toplamdan çıkar — P0

- **Değişiklik:** Backend canonical `ScoringDecisionState` (`provisional`, `auto_accepted`, `teacher_approved`, `rejected`, `failed`) ve `ScoringSummaryDto` (`provisionalScore`, `acceptedScore`, `finalScore`) üretir. `needsReview`, parse/evidence/consistency failure veya onaysız model sonucu final toplamına girmez. UI toplam hesaplamaz; backend summary'yi gösterir.
- **Dosyalar:** `src-tauri/src/domain/scoring.rs:55-105`, `src-tauri/src/domain/scoring.rs:414-441`, `src-tauri/src/services/scoring_service.rs:475-570`, `src-tauri/src/commands/scoring_commands.rs:17-71`, `src/api/types.ts:594-649`, `src/api/commands.ts:1003-1035`, `src/pages/scoringViewModel.ts:77-97`, `src/pages/ScoringPage.tsx:177-220`, `docs/API_CONTRACTS.md:350-363`, `docs/WORKFLOW_STATES.md:39-43`.
- **Dikkat:** Eski `scoringApplied=true, needsReview=true` kayıtları migration sırasında provisional olur; awarded score silinmez, yalnız final hesaptan çıkar. Öğretmen onayı typed backend command ile state değiştirir. `scoringApplied=false` ve score null model hatası normal sıfıra çevrilmez. QEP frozen kontrolü `src-tauri/src/domain/scoring.rs:414-441` aynen kalır.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml scoring`; `node --test --experimental-strip-types src/pages/scoringViewModel.test.ts`; regression: reviewable 7/10 görünür provisional olur, final toplam `0`/eksik kalır; teacher approval sonrası final `7` olur.

### 1.4 Criterion eşleşmesini canonical ID sözleşmesine geçir — P0

- **Değişiklik:** Model outputunda `criterionId` zorunlu olur. Backend yalnız frozen rubric ID'siyle eşler; unknown/duplicate/missing ID structured validation failure ve review üretir. Başlık eşleşmesi yalnız eski kayıt migration/salvage aşamasında Türkçe canonical normalizer ile yapılır, production model sonucu için fallback olmaz.
- **Dosyalar:** `src-tauri/src/services/scoring_service.rs:729-814`, `src-tauri/src/services/scoring_service.rs:886-936`, `src-tauri/src/domain/model.rs:565-575`, `src-tauri/src/domain/scoring.rs:55-105`, `docs/MODEL_GATEWAY.md:34-42`.
- **Dikkat:** Eksik kriter `0 puan aldı` olarak değil `scoringApplied=false/needsReview=true` olarak sonuçlanır. Frozen rubric ID'leri değişirse QEP invalidation uygulanır; UI teknik criterion hata kodlarını teacher-facing etikete çevirir.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml scoring_service`; exact ID success, case-only title with valid ID success, missing/unknown/duplicate ID failure ve QEP-not-frozen regression testleri.

### 1.5 Backend/UI confidence policy'yi tekleştir — P0

- **Değişiklik:** Versioned `OcrReviewPolicy` domain tipi ve DTO eklenir; threshold'lar backend snapshot/readiness cevabında taşınır. Gateway, OCR service ve UI aynı policy version/fingerprint'i kullanır. Frontend `0.6` sabitini kaldırır ve yalnız backend'in `needsReview`, reason labels ve policy DTO'sunu render eder.
- **Dosyalar:** `src-tauri/src/services/llama_server_gateway.rs:31-37`, `src-tauri/src/services/llama_server_gateway.rs:2786-2789`, `src-tauri/src/services/student_answer_ocr_service.rs:971-1053`, `src-tauri/src/domain/student.rs:419-483`, `src/pages/studentAnswerOcrUi.ts:51-420`, `src/api/types.ts:1580-1617`, `src/api/commands.ts:819-873`, `docs/MODEL_GATEWAY.md:53-60`, `docs/API_CONTRACTS.md:202-218`.
- **Dikkat:** UI threshold yorumlayıp readiness üretmez. Policy mutation ileride gerekirse ayrı typed backend command, audit log ve scoring invalidation ile yapılır; bu fazda sabit versioned backend policy yeterlidir.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml student_answer_ocr`; `node --test --experimental-strip-types src/pages/studentAnswerOcrUi.test.ts`; 0.60–0.719 confidence kaydı UI filtresinde kaybolmadan review olarak görünmelidir.

### 1.6 Türkçe Unicode normalizasyonunu merkezileştir — P0

- **Değişiklik:** Locale-aware/case-fold + Unicode normalization yapan tek Rust helper oluşturulur; similarity, legacy criterion salvage, critical token ve exact duplicate hazırlığı bunu kullanır. Normalize edilmiş metin yalnız karşılaştırma/cache anahtarı içindir; öğrenci OCR metni değiştirilmez.
- **Dosyalar:** `src-tauri/src/services/llama_server_gateway.rs:2981-2991`, `src-tauri/src/services/student_answer_ocr_service.rs:2135-2148`, `src-tauri/src/services/text_normalization.rs` (yeni; satır yok), `src-tauri/src/services/mod.rs:1-32`, ilgili servis test modülleri.
- **Dikkat:** `I/İ/ı/i`, combining marks, `Ç/ç`, `Ğ/ğ`, `Ö/ö`, `Ş/ş`, `Ü/ü`, whitespace ve noktalama fixture'ları ayrı test edilir. Normalizer evidence quote exact-match doğrulamasını gevşetmez; orada orijinal metin span'ı korunur.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml text_normalization`; criterion/similarity regression suite.

### 1.7 Prompt system-data ayrımını tüm use-case'lere uygula — P0

- **Değişiklik:** Her use-case için immutable system policy ve typed/serialized user data ayrılır. Question text, student answer, rubric, transcript ve metrics JSON user content'e taşınır. Prompt injection sınırları explicit delimiter yerine typed message builder ile korunur.
- **Dosyalar:** `src-tauri/src/services/question_text_service.rs:1240-1255`, `src-tauri/src/services/rubric_extraction_service.rs:1074-1096`, `src-tauri/src/services/student_answer_ocr_service.rs:2217-2340`, `src-tauri/src/services/scoring_service.rs:886-936`, `src-tauri/src/services/speaking_exam_service.rs:2430-2493`, `src-tauri/src/services/analysis_service.rs:570-593`, `src-tauri/src/services/llama_server_gateway.rs:322-1214`, `src-tauri/src/services/prompt_contract.rs` (yeni; satır yok), `docs/MODEL_GATEWAY.md:28-60`.
- **Dikkat:** OCR user data'ya rubric/expected answer eklenmez. Speaking cleanup'ın mevcut user transcript ayrımı korunur. Prompt değişimi version artırır ve golden fixture karşılaştırması olmadan eski cache'i reuse etmez.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml prompt`; fake gateway request-capture testleri dinamik öğrenci verisinin system message'ta bulunmadığını ve her use-case'in user payload schema'sına uyduğunu doğrular.

### 1.8 Versioned schema ve provenance kontratını ortaklaştır — P0 çekirdek, P1 genişletme

- **Değişiklik:** `ModelInvocationContract` ve `ModelProvenance` ortak tipleri eklenir: `useCase`, `promptVersion`, `schemaVersion`, `policyVersion`, `modelFingerprint`, `runtimeFingerprint`, `samplingParameters`. Structured use-case'ler capability destekliyorsa JSON schema/grammar, değilse JSON object kullanır; her durumda backend schema + domain validation zorunludur.
- **Dosyalar:** `src-tauri/src/domain/model.rs:170-235`, `src-tauri/src/services/model_gateway.rs:13-48`, `src-tauri/src/services/llama_server_gateway.rs:322-1214`, `src-tauri/src/services/student_answer_ocr_service.rs:36-38`, `src-tauri/src/services/scoring_service.rs:28`, `src-tauri/src/services/speaking_exam_service.rs:52-57`, `src-tauri/src/domain/student.rs:214-238`, `src-tauri/src/domain/student.rs:419-483`, `src/api/types.ts:1338-1362`, `src/api/types.ts:1580-1617`, `docs/MODEL_GATEWAY.md:60`, `docs/API_CONTRACTS.md:471-480`.
- **Dikkat:** Server schema capability'si runtime probe/cache ile belirlenir; unsupported durumda validation kapanmaz. Raw response path diagnostics'te kalır, teacher UI'ya taşınmaz. Model file hash hesaplama uzun sürüyorsa startup/background job sonucu profile fingerprint'e bağlanır.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml model`; her gateway requestinde eksiksiz provenance, unsupported schema fallback, invalid schema review/failure ve project migration testleri.

### 1.9 `critical_term_hint` bilgisini saf OCR correction'dan çıkar — P0

- **Değişiklik:** Issue correction inputu yalnız crop/observed text/konum/image-quality verisi taşır. Rubrik/expected answer kaynaklı hint alanı request ve prompttan silinir. İstenirse ayrı `ContextualReviewSuggestion` yalnız öğretmen incelemesine sunulur; canonical OCR'a otomatik uygulanmaz ve scoring kanıtı sayılmaz.
- **Dosyalar:** `src-tauri/src/services/student_answer_ocr_service.rs:662-671`, `src-tauri/src/services/student_answer_ocr_service.rs:2290-2340`, `src-tauri/src/services/student_answer_ocr_service.rs:2418-2437`, `src-tauri/src/domain/model.rs:354-372`, `src-tauri/src/services/llama_server_gateway.rs:614-730`, `src/api/types.ts:1580-1617`.
- **Dikkat:** OCR prompt/inputta expected answer, rubric criteria, key concepts, partial credit veya zero-score koşulu kalmamalı. Contextual öneri eklenirse ayrı provenance/review-only alanı ve teacher-facing açıklama zorunludur.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml student_answer_ocr`; request-capture regression testi answer key'deki benzersiz terimin hiçbir pure OCR/correction payload'ında bulunmadığını doğrular.

### 1.10 Speaking evaluation text-only profilini regression ile kilitle — P0 doğrulama, davranış değişikliği yok

- **Değişiklik:** Mevcut text-only profil davranışı korunur; standard/mmproj migration yapılmaz çünkü kod zaten hedef durumda. Profile capability, args, provenance ve request image absence için regression testleri eklenir. Doküman mevcut durumu açıkça kaydeder.
- **Dosyalar:** `src-tauri/src/domain/model.rs:768-799`, `src-tauri/src/domain/model.rs:898-927`, `src-tauri/src/services/speaking_exam_service.rs:2031-2044`, `src-tauri/src/services/speaking_exam_service.rs:2193-2225`, `docs/MODEL_GATEWAY.md:44-49`, `docs/SPEAKING_EXAM_ENGINE.md:1-9`.
- **Dikkat:** Rubric evaluation ile transcript cleanup profile/provenance kimlikleri karıştırılmaz. Test, args içinde `--mmproj` bulunmadığını ve requestte image content olmadığını doğrular; golden scoring sonucu değişmemelidir.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml speaking_exam`; mevcut golden speaking fixture'ları + exact runtime arg snapshot.

### 1.11 Strict-local privacy policy ekle — P0/P1

- **Değişiklik:** `PrivacyMode::{StrictLocal, ExplicitExternal}` eklenir; default `StrictLocal` olur. Strict mode loopback URL, managed/verified runtime, model fingerprint, redirect reddi, proxy devre dışı ve öğrenci verisi use-case'lerinde external profile reddi uygular. External açma state değişikliği typed command, açık kullanıcı onayı, audit event ve güçlü UI uyarısı gerektirir.
- **Dosyalar:** `src-tauri/src/domain/model.rs:6-41`, `src-tauri/src/domain/model.rs:743-755`, `src-tauri/src/services/llama_server_gateway.rs:83-97`, `src-tauri/src/services/model_process_manager.rs:1446-1492`, `src-tauri/src/commands/model_commands.rs:16-21`, `src-tauri/src/commands/model_commands.rs:73-81`, `src/api/types.ts:1338-1362`, `src/api/commands.ts:1086-1112`, `src/pages/SettingsPage.tsx:1`, `docs/PRIVACY_LOGGING_AND_PUBLIC_ERRORS.md:1`, `docs/API_CONTRACTS.md:394-442`.
- **Dikkat:** Redirect policy her hop'ta, IPv4/IPv6 loopback ve DNS rebinding'e karşı doğrulanır; environment proxy bypass edilir. External consent secret/student data loglamaz. Eski external profile taşıyan projeler sessizce çağrı yapmaz; blocked state + suggested action verir.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml privacy`; loopback allow, private/public hostname deny, redirect deny, proxy-ignore, explicit consent audit ve frontend teacher-facing warning testleri; `npm run typecheck && npm test`.

### 1.12 Gerçek JPEG content cache oluştur — P1

- **Değişiklik:** Cache key; source hash, ordered crop regions, alignment transform, preprocess mode, resize policy version, JPEG quality ve encoder version'dan üretilir. Hit'te decode/encode yapılmaz. JPEG temp dosyaya yazılıp fsync/rename ile atomik yayımlanır; manifest aynı transaction kimliğini taşır.
- **Dosyalar:** `src-tauri/src/services/model_input_image_service.rs:15-16`, `src-tauri/src/services/model_input_image_service.rs:275-448`, `src-tauri/src/platform/paths.rs:1`, `src-tauri/src/services/project_store.rs:1`, `src-tauri/src/domain/model.rs:170-200`, `docs/API_CONTRACTS.md:471-480`.
- **Dikkat:** Cache artifact domain truth değildir; kayıp/bozuk cache yeniden üretilebilir. Metadata state'i `ProjectStore` dışında yazılmaz. Concurrent aynı key için single-writer/temp-file collision testi gerekir. Öğrenci verisi hash/log policy'sine uyulur.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml model_input_image_service`; ikinci aynı çağrıda mtime/hash değişmez ve encoder çağrılmaz; source/policy/region değişiminde miss; yarım temp/bozuk manifest recovery testleri.

## Faz 1 doğrulama ve çıkış kriteri

1. Normal OCR/scoring/speaking/analysis job'ında completion probe sayısı sıfırdır.
2. Parallel lease testi tek startup/readiness akışı gösterir; fixed sleep yoktur.
3. `needsReview` model skoru provisional görünür, final toplama girmez.
4. Criterion, prompt/schema, privacy ve leakage negatif testleri geçer.
5. Speaking golden sonucu aynı kalır ve mmproj yüklenmez.
6. JPEG ikinci çalıştırmada gerçek cache hit verir.
7. Ortak kalite komutlarının tamamı ve gerçek llama.cpp smoke testi geçer.

---

# Faz 2 — OCR görüntü zinciri ve extraction verimliliği

## Faz hedefi

OCR doğruluğunun prompt öncesindeki kayıplarını azaltmak; yüksek çözünürlüklü kaynak, registration, çoklu crop, adaptive preprocess, seçici ikinci geçiş, typed answer ve tam provenance oluşturmak. Aynı P1 döneminde question/rubric extraction'ın gereksiz tüm-belge tekrarlarını ve pahalı parse retry'sini azaltmak.

## Faz kapsamı ve bağımsızlık sınırı

- **[BENCHMARK ZORUNLU]** Bu faz başlamadan anonim gerçek öğrenci taramalarından lisans/izin kontrollü golden set ve baseline manifesti hazırlanır.
- Faz 2 yalnız OCR/extraction candidate üretimini değiştirir. Teacher approval ve QEP frozen/scoring gate korunur.
- Yeni crop şeması eski tek-region template'i kayıpsız olarak `regions[0]` biçiminde migrate eder.
- Her alt özellik ayrı feature flag/policy version ile açılır; baseline'dan kötüleşirse bağımsız kapatılabilir.

## Değiştirilecek ana dosyalar

`src-tauri/src/services/pdf_service.rs:57-130`, `src-tauri/src/services/student_answer_crop_service.rs:63-555`, `src-tauri/src/domain/student.rs:242-483`, `src-tauri/src/services/ocr_image_preprocess_service.rs:1-660`, `src-tauri/src/services/student_answer_ocr_service.rs:940-2542`, `src-tauri/src/services/model_input_image_service.rs:15-448`, `src-tauri/src/domain/model.rs:170-372`, `src-tauri/src/services/llama_server_gateway.rs:614-730`, `src-tauri/src/services/question_text_service.rs:512-734`, `src-tauri/src/services/rubric_extraction_service.rs:297-1023`, `src-tauri/src/domain/rubric.rs:41-64`, frontend DTO/review bileşenleri ve ilgili tests/fixtures.

## Uygulama maddeleri

### 2.1 OCR'a özel yüksek çözünürlüklü render — P1, [BENCHMARK ZORUNLU]

- **Değişiklik:** UI preview ve OCR source render iki artifact family olur. OCR job 300/350/400 DPI policy seçeneklerinden versioned default kullanır; tam sayfa modele gitmez, yüksek çözünürlüklü source registration/crop sonrası bounded inputa dönüşür. Render cache key PDF checksum + page + DPI + renderer version içerir.
- **Dosyalar:** `src-tauri/src/services/pdf_service.rs:57-130`, `src-tauri/src/services/student_answer_crop_service.rs:63-69`, `src-tauri/src/services/model_input_image_service.rs:377-392`, `src-tauri/src/domain/document.rs:1`, `src-tauri/src/jobs/job_manager.rs:1`, `src-tauri/src/domain/job.rs:1`, `src-tauri/src/services/ocr_render_service.rs` (yeni; satır yok).
- **Dikkat:** Render uzun iştir ve OCR job stage/progress'i olarak çalışır. UI preview değişmez. Peak RAM/disk sınırı, cancellation ve artifact cleanup test edilir.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml ocr_render`; golden A/B: 144 DPI baseline'a karşı 300/350/400 DPI CER/WER, kritik terim, latency ve RAM; yalnız kabul eşiğini geçen policy default olabilir.

### 2.2 Registration, deskew ve perspektif düzeltme — P1, [BENCHMARK ZORUNLU]

- **Değişiklik:** Page boundary → deskew → perspective correction → template anchor registration → aligned crop → ink-aware padding zinciri eklenir. Transform matrisi, confidence ve failure reason provenance'a yazılır; düşük confidence raw bbox fallback yaparsa otomatik review zorunlu olur.
- **Dosyalar:** `src-tauri/src/services/student_answer_crop_service.rs:410-555`, `src-tauri/src/domain/student.rs:242-270`, `src-tauri/src/services/page_registration_service.rs` (yeni; satır yok), `src-tauri/tests/fixtures/ocr_registration/` (yeni; satır yok).
- **Dikkat:** Sessiz geometrik fallback yasaktır. Transform yalnız artifact üretir; template/domain mutation ancak typed teacher command ile yapılır. Rotasyon, shift, perspective ve partial-page fixture'ları gereklidir.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml page_registration`; bbox IoU/coverage eşiği, transform round-trip ve registration failure → review testleri; golden CER/WER karşılaştırması.

### 2.3 Multi-region ve multi-page crop domain modeli — P1

- **Değişiklik:** `QuestionAnswerTemplate { questionId, regions[] }` eklenir; region `pageOffset`, `order`, `normalizedBbox`, `regionRole`, `continuationPolicy` taşır. Coverage gerçek expected region setine göre hesaplanır; `page_numbers.len()>1` tek başına partial nedeni olmaktan çıkar.
- **Dosyalar:** `src-tauri/src/domain/student.rs:242-270`, `src-tauri/src/services/student_answer_crop_service.rs:70-75`, `src-tauri/src/services/student_answer_crop_service.rs:311-374`, crop template command dosyaları, `src/api/types.ts:1580-1617`, crop UI bileşenleri, `docs/API_CONTRACTS.md:141-208`.
- **Dikkat:** Region sırası deterministik olmalı ve model payloadında korunmalı. Eski single-region project migration'ı kayıpsızdır. UI unsaved rectangle state tutabilir, fakat save/coverage/readiness kararı backend command sonucudur.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml student_answer_crop`; migration, ordered two-region, two-page continuation, missing required region ve optional region testleri; ilgili frontend component testleri.

### 2.4 Full-page OCR'ı açık review-only moda al — P1

- **Değişiklik:** Production OCR readiness crop/region zorunlu tutar. Ayrı `ExperimentalFullPageReviewOnly` job mode'u typed inputla açılır; çıktısı canonical scoring-ready OCR olamaz, `needsReview=true` ve non-approvable-for-scoring provenance taşır. UI açık uyarı ve disabled reason gösterir.
- **Dosyalar:** `src-tauri/src/services/student_answer_crop_service.rs:365-374`, `src-tauri/src/services/student_answer_ocr_service.rs:940-1053`, `src-tauri/src/domain/student.rs:419-483`, OCR command dosyaları, `src-tauri/src/services/workflow_engine.rs:95-120`, `src/api/types.ts:1580-1617`, OCR page/UI, `docs/API_CONTRACTS.md:202-218`, `docs/WORKFLOW_STATES.md:33-40`.
- **Dikkat:** Experimental sonuç teacher tarafından metin düzeltme referansı olabilir ama scoring gate'i geçemez; UI override eklenmez. Production fallback crop yoksa structured `CROP_REGION_MISSING/OCR_NOT_READY` döndürür.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml student_answer_ocr`; crop-less production rejection, experimental success + forced review, approval denemesi rejection ve scoring blocked regression; frontend warning testi.

### 2.5 Adaptive ve lazy preprocess — P1, [BENCHMARK ZORUNLU]

- **Değişiklik:** Önce image quality profile (contrast, stroke density, blur, clipping, ink ratio, crop size, skew) hesaplanır. Versioned policy tek başlangıç varyantı seçer; alternatifler yalnız ikinci-pass trigger oluşursa üretilir. Üretilen/atlanılan varyantlar diagnostics'e yazılır.
- **Dosyalar:** `src-tauri/src/services/ocr_image_preprocess_service.rs:1-660`, `src-tauri/src/services/student_answer_ocr_service.rs:41-47`, `src-tauri/src/services/student_answer_ocr_service.rs:2477-2556`, `src-tauri/src/domain/model.rs:170-200`, `src-tauri/src/services/image_quality_service.rs` (yeni; satır yok).
- **Dikkat:** Tek bir handwriting filtresi tüm türlere default yapılmaz. Policy teacher content'ini değiştirmez; artifact cache key Faz 1.12 alanlarını kullanır. Quality metric failure raw image + review ile sonuçlanır.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml ocr_image_preprocess`; varyant üretim sayacı testi; golden set üzerinde CER/WER, correction rate, runtime ve RAM non-regression kapısı.

### 2.6 Seçici ikinci OCR geçişi — P1, [BENCHMARK ZORUNLU]

- **Değişiklik:** Versioned risk evaluator confidence, edge clipping, ink-empty mismatch, schema eksikliği, numeric/symbol yoğunluğu, `finish_reason=length` ve preprocess disagreement sinyallerini üretir. Risk varsa alternatif crop/preprocess ile ikinci model call yapılır. Sonuçlar deterministic comparator ile `agree`, `prefer_first`, `prefer_second`, `conflict` olur; conflict teacher comparison review açar.
- **Dosyalar:** `src-tauri/src/services/student_answer_ocr_service.rs:940-1053`, `src-tauri/src/services/student_answer_ocr_service.rs:2477-2556`, `src-tauri/src/services/llama_server_gateway.rs:614-730`, `src-tauri/src/domain/student.rs:419-483`, `src-tauri/src/services/ocr_second_pass_service.rs` (yeni; satır yok), OCR review frontend bileşenleri.
- **Dikkat:** İkinci sonuç ilkini sessizce ezmez; iki invocation provenance'ı saklanır. Kritik terim trigger'ı answer key bilgisinden değil yalnız görsel/observed text belirsizliğinden gelir. Job progress/cancellation iki call'ı da kapsar.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml ocr_second_pass`; her trigger, no-risk tek-call, agreement, conflict, timeout ve cancellation testleri; golden set call-rate/accuracy/review-rate karşılaştırması.

### 2.7 Answer-type typed `StructuredAnswer` şemaları — P1

- **Değişiklik:** `serde_json::Value` yerine tagged Rust enum ve TypeScript discriminated union eklenir: multiple choice, matching, ordered slots, numeric, table, correction table, sentence annotation, grammar analysis ve open text. AnswerType → allowed schema mapping backend'de yapılır. Invalid/mismatched schema reviewable OCR failure olur ve scoring uygulanmaz.
- **Dosyalar:** `src-tauri/src/domain/model.rs:354-372`, `src-tauri/src/domain/student.rs:419-483`, `src-tauri/src/domain/question.rs:5-22`, `src-tauri/src/services/student_answer_ocr_service.rs:2217-2267`, `src/api/types.ts:1580-1617`, OCR review components, `src-tauri/src/domain/structured_answer.rs` (yeni; satır yok).
- **Dikkat:** Eski arbitrary JSON kayıtları migration/salvage ile parse edilir; başarısız olanlar `needsReview=true` kalır, veri silinmez. Frontend domain validation yapmaz. Placeholder slot/value canonical cevap sayılmaz.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml structured_answer`; her variant round-trip/schema, wrong answer type, incomplete table ve migration testleri; `npm run typecheck` ve OCR component tests.

### 2.8 Dynamic image/output token budget — P1, [BENCHMARK ZORUNLU]

- **Değişiklik:** Use-case, crop geometry, region sayısı, expected schema ve runtime capability'den bounded `InvocationBudget` üretilir. Seçim request metadata/provenance'a yazılır. Hard min/max güvenlik sınırları kalır; model kendi budget'ını belirlemez.
- **Dosyalar:** `src-tauri/src/services/llama_server_gateway.rs:31-35`, `src-tauri/src/services/llama_server_gateway.rs:614-638`, `src-tauri/src/services/model_input_image_service.rs:15-16`, `src-tauri/src/domain/model.rs:170-235`, `src-tauri/src/services/model_budget_policy.rs` (yeni; satır yok).
- **Dikkat:** Output token azaltımı truncation/schema başarısını düşürmemeli; image budget yüksek-res full page göndermek için kullanılmaz. Production default yalnız benchmark eşiğiyle değişir.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml model_budget_policy`; boundary/property tests; golden set p95, finish_reason=length, schema success, CER/WER ve RAM A/B.

### 2.9 OCR provenance ve gerçek model input metadata — P1

- **Değişiklik:** Her OCR attempt source checksum/page, region IDs/order, render DPI/renderer, registration transform/confidence, preprocess policy/variant, resize dimensions, JPEG cache key, invocation contract/fingerprint, budget ve response diagnostics taşır. UI teacher view yalnız friendly özet, developer view full metadata gösterir.
- **Dosyalar:** `src-tauri/src/domain/model.rs:170-235`, `src-tauri/src/domain/student.rs:214-238`, `src-tauri/src/domain/student.rs:419-483`, `src-tauri/src/services/student_answer_ocr_service.rs:940-1053`, `src-tauri/src/services/model_input_image_service.rs:275-448`, `src/api/types.ts:1580-1617`, diagnostics UI ve export service.
- **Dikkat:** Provenance gerçek modele giden final artifact'ı göstermeli; ara varyantı final diye kaydetmemeli. Path'ler diagnostic exportta privacy/redaction policy'sine uyar. Missing old metadata `unknown` olarak açık kalır, uydurulmaz.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml student_answer_ocr`; source→crop→preprocess→request zincir hash testi, migration ve diagnostic redaction testleri; frontend developer-panel testleri.

### 2.10 Question/rubric extraction için sınırlı sayfa penceresi — P1, [BENCHMARK ZORUNLU]

- **Değişiklik:** PDF text/marker/layout analizi question-to-page candidate üretir; hedef sayfa + gerektiğinde ±1 pencere kullanılır, düşük confidence'ta bounded geniş fallback job stage'i çalışır. Her target request kullanılan sayfa listesini provenance'a yazar.
- **Dosyalar:** `src-tauri/src/services/question_text_service.rs:512-734`, `src-tauri/src/services/rubric_extraction_service.rs:496-605`, `src-tauri/src/services/pdf_service.rs:57-130`, `src-tauri/src/services/page_window_service.rs` (yeni; satır yok), extraction job events/tests.
- **Dikkat:** Tek sayfa zorunluluğu yoktur; devam eden soru/rubrik kaybı fallback ile review'a görünür. Aynı document render artifact'ı cache'ten paylaşılır, fakat target provenance ayrıdır.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml page_window`; marker success, ±1 continuation, no-marker fallback, wrong-page prevention; golden documents üzerinde model call/image sayısı ve extraction field recall A/B.

### 2.11 Rubric extraction schema'sını canonical rubric modeliyle eşitle — P1

- **Değişiklik:** Prompt/output schema canonical `RubricState` alanlarından versioned contract üretir: expected answer, key concepts, criteria, partial credit notes/hints, zero-score conditions, common mistakes, source/status/warnings sınırları. Model suggested içerik üretir; teacher confirmation olmadan authoritative olmaz.
- **Dosyalar:** `src-tauri/src/services/rubric_extraction_service.rs:297-328`, `src-tauri/src/services/rubric_extraction_service.rs:1074-1096`, `src-tauri/src/domain/rubric.rs:41-64`, `src-tauri/src/services/llama_server_gateway.rs:422-475`, rubric DTO/UI files, `docs/API_CONTRACTS.md:269-318`.
- **Dikkat:** Placeholder detector `src-tauri/src/domain/rubric.rs:83-106` korunur. Prompt ve domain alan listesi iki ayrı elle yazılmış truth olmamalı; schema builder canonical DTO'dan türemeli. Yeni extracted alanların hepsi `suggested` kalır.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml rubric_extraction`; full canonical fixture, missing field, placeholder rejection, teacher confirmation ve old project migration tests.

### 2.12 Parse retry zincirini ucuzlat — P1, [BENCHMARK ZORUNLU]

- **Değişiklik:** Retry sırası: schema/grammar ilk çağrı → deterministic JSON salvage → raw response ile text-only JSON repair → son çare bounded multimodal retry. Text-only repair image taşımamalı; her attempt reason/cost/provenance kaydetmeli.
- **Dosyalar:** `src-tauri/src/services/rubric_extraction_service.rs:976-1023`, `src-tauri/src/services/llama_server_gateway.rs:408-475`, `src-tauri/src/services/llama_server_gateway.rs:1430-1438`, `src-tauri/src/services/model_gateway.rs:13-48`, `src-tauri/src/services/structured_output_repair_service.rs` (yeni; satır yok).
- **Dikkat:** Repair modeli kayıp domain alanı uyduramaz; yalnız verilen raw texti schema'ya dönüştürür. Repair failure suggested rubric üretmez. Multimodal retry job cancellation/deadline ve call budget'a uyar.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml structured_output_repair`; salvage success'te ikinci model call `0`, repair requestte image `0`, final retryde bounded call; golden malformed-response setinde field recall ve cost A/B.

## Faz 2 doğrulama ve çıkış kriteri

1. Golden set kaynak/izin manifesti ve baseline sonucu versioned olarak saklanır; öğrenci kimlikleri bulunmaz.
2. OCR production yolu yalnız registered/aligned region inputuyla çalışır; full-page sonuç scoring-ready olamaz.
3. Tek ve çoklu region/page coverage testleri geçer.
4. Adaptive + second-pass policy baseline'a göre tanımlı CER/WER, critical term ve review-rate eşiklerini karşılar; aksi halde feature flag kapalı kalır.
5. `StructuredAnswer` dışı arbitrary JSON canonical yeni kayda yazılmaz.
6. Her OCR sonucundan gerçek model input artifact/provenance zincirine ulaşılabilir.
7. Extraction page-window ve parse-retry model call/image maliyetini düşürürken field recall non-regression gösterir.
8. Ortak kalite komutları ve gerçek llama.cpp OCR/schema smoke testi geçer.

---

# Faz 3 — Deterministik scoring

## Faz hedefi

Deterministik cevapları modelden çıkarmak, semantik scoring'i canonical rubric level/evidence sözleşmesine bağlamak, exact fingerprint/cache ile tekrarlanabilirliği sağlamak ve provisional/final kararını tamamen backend'de yönetmek.

## Faz kapsamı ve bağımsızlık sınırı

- Faz 3, Faz 2 typed answer şemaları yoksa yalnız mevcut güvenli answer type'lar için adapter ile açılabilir; unsupported türler mevcut reviewable model yolunda kalır.
- Her scorer/level policy answer type veya question bazında feature flag taşır; tüm soru türlerinin aynı release'te migrate edilmesi gerekmez.
- QEP frozen gate ve OCR teacher approval gate zayıflatılmaz.
- Candidate cache, final teacher decision cache'ten fiziksel ve semantik olarak ayrılır.

## Değiştirilecek ana dosyalar

`src-tauri/src/domain/scoring.rs:55-441`, `src-tauri/src/services/scoring_service.rs:352-936`, `src-tauri/src/domain/model.rs:565-575`, `src-tauri/src/domain/rubric.rs:41-64`, `src-tauri/src/domain/question.rs:5-22`, scoring commands/API/UI, ProjectStore/artifact services ve yeni deterministic scorer/cache/consistency modülleri.

## Uygulama maddeleri

### 3.1 Deterministik answer-type scorer'lar — P1/P2

- **Değişiklik:** Backend dispatch; multiple choice, true/false, matching, ordering, numeric, fill-in ve uygun structured table türlerini pure Rust scorer'a yönlendirir. Her scorer canonical answer/rubric, typed student answer ve policy version alıp criterion evidence + score veya reviewable failure döndürür. Semantik türler model gateway'e devam eder.
- **Dosyalar:** `src-tauri/src/domain/question.rs:5-22`, `src-tauri/src/domain/scoring.rs:55-105`, `src-tauri/src/services/scoring_service.rs:352-455`, `src-tauri/src/domain/structured_answer.rs` (Faz 2 yeni), `src-tauri/src/services/deterministic_scoring_service.rs` (yeni; satır yok).
- **Dikkat:** Numeric tolerance/unit/locale ve partial credit yalnız confirmed/frozen rubric policy'sinden gelir; frontend karar vermez. Unsupported/malformed input sıfır değil review olur. Model çağrısı yapılmadığı diagnostics'te açıkça görülür.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml deterministic_scoring`; her tür success/boundary/invalid input, Turkish decimal, unit mismatch, partial credit ve QEP-not-frozen tests; spy gateway deterministic türde `0` call doğrular.

### 3.2 Exact scoring fingerprint ve candidate cache — P1/P2

- **Değişiklik:** Fingerprint QEP, answer, OCR generation, prompt/schema/policy, model file, runtime, sampling ve calibration/anchor version içerir. Candidate cache ham model/deterministic proposal saklar; lookup exact byte-level fingerprint eşleşmesi ister. Cache hit provenance ve audit event üretir.
- **Dosyalar:** `src-tauri/src/domain/scoring.rs:135-280`, `src-tauri/src/services/scoring_service.rs:416-570`, `src-tauri/src/domain/model.rs:170-235`, `src-tauri/src/services/scoring_cache_service.rs` (yeni; satır yok), `src-tauri/src/services/project_store.rs:1`, diagnostics/export docs.
- **Dikkat:** Candidate cache teacher-approved final değildir. Yanlış model sonucu cache hit olsa da review state'i korunur. Artifact atomik yazılır, metadata yalnız ProjectStore ile commit edilir; model/policy değişimi doğal cache miss üretir.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml scoring_cache`; identical fingerprint second run `0` model call, her fingerprint bileşeni değişiminde miss, corrupt cache recovery ve concurrency tests.

### 3.3 Provisional/final score ayrımını scoring lifecycle'a tamamla — P1/P2

- **Değişiklik:** Faz 1 summary düzeltmesi persisted lifecycle'a genişletilir: model candidate/provisional, deterministic accepted, auto-accepted (policy izinli), teacher-approved final ve rejected/failed. Backend final assessment/export/analysis yalnız final kararları tüketir.
- **Dosyalar:** `src-tauri/src/domain/scoring.rs:55-105`, `src-tauri/src/services/scoring_service.rs:192-247`, `src-tauri/src/services/scoring_service.rs:475-570`, `src-tauri/src/commands/scoring_commands.rs:17-71`, analysis/export services, `src/api/types.ts:594-649`, `src/pages/ScoringPage.tsx:177-220`.
- **Dikkat:** State transition matrix backend'de deterministic olmalı. Onaysız review sonucu analysis/export final nota sızmamalı. Manual score audit/provenance ile saklanır; model score overwrite edilmez.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml scoring_state`; transition/property tests, analysis/export exclusion regression; frontend yalnız backend summary render testi.

### 3.4 Structured rubric level modeli — P2, [GOLDEN SCORING SET ZORUNLU]

- **Değişiklik:** Criterion'a versioned levels eklenir: ID, title, required/disqualifying conditions, score, evidenceRequired. Eski numeric/max-only rubrikler explicit migration assistant veya teacher review gerektirir; model otomatik authoritative level uyduramaz.
- **Dosyalar:** `src-tauri/src/domain/rubric.rs:41-64`, rubric command/service/UI, QEP build/freeze domain files, `src-tauri/src/domain/scoring.rs:135-280`, `docs/API_CONTRACTS.md:269-363`.
- **Dikkat:** Rubric level değişikliği frozen QEP'yi invalidated yapar. Placeholder level/condition kabul edilmez. Migration sonucu suggested olur ve teacher confirmation ister.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml rubric_levels`; schema/migration/placeholder/QEP invalidation/freeze gate tests; rubric component testleri.

### 3.5 Model level/evidence üretir, Rust puanı hesaplar — P2, [GOLDEN SCORING SET ZORUNLU]

- **Değişiklik:** Yazılı semantic scoring outputu numeric score yerine `criterionId`, `levelId`, exact evidence, missing requirement ve contradiction döndürür. Rust canonical frozen rubricten level→score map eder, evidence span doğrular, criterion/final toplamı hesaplar.
- **Dosyalar:** `src-tauri/src/services/scoring_service.rs:729-936`, `src-tauri/src/domain/model.rs:565-575`, `src-tauri/src/domain/scoring.rs:55-105`, `src-tauri/src/services/llama_server_gateway.rs:1175-1214`; referans desen `src-tauri/src/services/speaking_exam_service.rs:2459-2493` ve `src-tauri/src/services/speaking_exam_service.rs:2990-3089`.
- **Dikkat:** Speaking kodu kopyalanıp god service oluşturulmaz; ortak küçük validator/mapping primitive'leri ayrılır. Invalid evidence/level normal sıfır olmaz. Modelin döndürdüğü herhangi bir direct score alanı reddedilir/ignore edilir ve diagnostics'e yazılır.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml semantic_scoring`; level mapping, evidence exact span, contradiction ceiling, unknown ID, injected direct score ve malformed JSON tests; golden teacher-score agreement.

### 3.6 Exact duplicate reuse — P2

- **Değişiklik:** Aynı question/QEP/policy kapsamında Türkçe-normalize + raw hash ile yüzde yüz exact duplicate belirlenir. Yalnız deterministic result veya teacher-approved final decision yeniden kullanılabilir; ham benzer model proposal final olarak kopyalanmaz. Reuse provenance source record ID ve decision version taşır.
- **Dosyalar:** `src-tauri/src/services/scoring_cache_service.rs` (Faz 3.2 yeni), `src-tauri/src/services/scoring_service.rs:352-570`, `src-tauri/src/domain/scoring.rs:55-280`, `src-tauri/src/services/text_normalization.rs` (Faz 1 yeni).
- **Dikkat:** Question, rubric/QEP, OCR generation, numeric/unit ve negation farkı reuse'u engeller. Öğrenci kimliği fingerprint'e karar girdisi değildir ama audit source/target ayrı tutulur. Teacher override yeni decision version oluşturur.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml exact_duplicate_scoring`; Turkish case/whitespace equivalence, negation/number/unit/QEP difference miss ve approved-source reuse tests.

### 3.7 Consistency review — P2

- **Değişiklik:** Aynı exact/near-exact answer cluster'ında farklı final/provisional level veya score varsa `consistency_review` reason üreten backend service eklenir. Bu reason final auto-accept'i engeller; UI öğretmene friendly karşılaştırma gösterir.
- **Dosyalar:** `src-tauri/src/services/scoring_consistency_service.rs` (yeni; satır yok), `src-tauri/src/domain/scoring.rs:55-105`, `src-tauri/src/services/scoring_service.rs:475-570`, `src/api/types.ts:594-649`, scoring review components.
- **Dikkat:** Consistency service puanı otomatik kopyalamaz/değiştirmez. Teknik reason code yalnız diagnostics'te, UI'da Türkçe açıklama. Recalculation state change ise backend job/command + ProjectStore transaction kullanır.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml scoring_consistency`; same answer/different score, legitimate rubric/QEP difference, resolved teacher decision ve UI label tests.

## Faz 3 doğrulama ve çıkış kriteri

1. Desteklenen deterministik türlerde model call sıfır ve sonuç byte-for-byte tekrarlanabilirdir.
2. Semantic model yalnız canonical level/evidence önerir; Rust dışında score hesaplanmaz.
3. Exact fingerprint ikinci çalıştırmada candidate cache hit verir; fingerprint bileşeni değişince miss olur.
4. Final total/export/analysis yalnız accepted/final kararları içerir.
5. Duplicate reuse yalnız deterministic veya teacher-approved kaynaktan gelir; consistency conflict review açar.
6. QEP frozen ve OCR teacher approval negatif testleri değişmeden geçer.
7. Ortak kalite komutları ve scoring smoke/golden set geçer.

---

# Faz 4 — Kalibrasyon ve gelişmiş tutarlılık

## Faz hedefi

Öğretmen kararlarını güvenli kalibrasyon verisine dönüştürmek, benzer cevaplarda tutarlılık desteği vermek, yalnız ölçülmüş risk sınıflarında auto-accept değerlendirmek, analysis çıktısını metriklere bağlamak ve runtime/HTR kararlarını benchmark kanıtıyla almak.

## Faz kapsamı ve bağımsızlık sınırı

- Faz 4 özellikleri default kapalı ve policy-versioned başlar.
- Anchor/cluster öğretmen karar destek aracıdır; yakın cevapta puan kopyalama yapmaz.
- Auto-accept ancak holdout golden set, random audit ve geri alma kapılarıyla açılır; QEP frozen gate hiçbir koşulda kaldırılmaz.
- Runtime ve HTR değişiklikleri benchmark raporu kabul edilmeden production config'e yazılmaz.

## Değiştirilecek ana dosyalar

Scoring domain/service/cache/consistency modülleri, `src-tauri/src/services/analysis_service.rs:570-593`, `src-tauri/src/domain/analysis.rs:49-71`, `src-tauri/src/services/llama_server_gateway.rs:1099-1127`, `src-tauri/src/domain/model.rs:811-854`, analysis/scoring/OCR frontend DTO ve pages, diagnostics/export ve benchmark fixture/tooling.

## Uygulama maddeleri

### 4.1 Teacher-approved anchor cevaplar — P2/P3

- **Değişiklik:** Öğretmen ayrı typed command ile onaylı final decision'ı question/QEP/policy kapsamında anchor yapabilir veya anchor statüsünü kaldırabilir. Anchor immutable version, source record, teacher action, evidence ve calibration version taşır.
- **Dosyalar:** `src-tauri/src/domain/scoring.rs:55-280`, `src-tauri/src/commands/scoring_commands.rs:17-71`, `src-tauri/src/services/scoring_service.rs:192-247`, `src-tauri/src/services/scoring_anchor_service.rs` (yeni; satır yok), `src/api/types.ts:594-649`, `src/api/commands.ts:1003-1035`, scoring review UI, `docs/API_CONTRACTS.md:350-363`.
- **Dikkat:** Yalnız teacher-approved ve placeholder içermeyen kayıt anchor olabilir. Rubric/QEP/policy değişimi anchor'ı silmez fakat stale/ineligible yapar. Model önerisi kendiliğinden anchor olamaz.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml scoring_anchor`; create/revoke, permission/audit, stale QEP, placeholder ve ProjectStore atomic mutation tests; frontend command/loading/error feedback tests.

### 4.2 Benzer cevap kümeleri — P2/P3, [BENCHMARK ZORUNLU]

- **Değişiklik:** İlk sürüm embedding olmadan normalized exact, token overlap, BM25/FTS, critical concept ve numeric/unit/negation farkı üretir. Cluster read model/backend job olarak hesaplanır; UI karşılaştırma ve önceliklendirme gösterir.
- **Dosyalar:** `src-tauri/src/services/scoring_consistency_service.rs` (Faz 3 yeni), `src-tauri/src/services/answer_similarity_service.rs` (yeni; satır yok), `src-tauri/src/domain/scoring.rs:55-280`, job/command files, scoring review UI.
- **Dikkat:** Yakın cevap puan kopyalamaz ve final kararı değiştirmez. Student identity model inputu/similarity feature'ı değildir. Büyük sınıflarda hesap uzun job ve cancellable olmalıdır.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml answer_similarity`; negation/number/unit/date/critical-concept adversarial fixtures; labeled pair precision/recall benchmark ve UI cluster testleri.

### 4.3 Öğretmen düzeltmelerine dayalı confidence kalibrasyonu — P2/P3, [BENCHMARK ZORUNLU]

- **Değişiklik:** OCR/scoring correction events anonim/aggregate calibration datasetine dönüşür. Use-case/answer type/image-quality/policy version bazında observed error rate ve calibrated risk bucket hesaplanır. Raw student answer varsayılan calibration exportuna girmez.
- **Dosyalar:** OCR/scoring update command/service files, `src-tauri/src/domain/student.rs:419-483`, `src-tauri/src/domain/scoring.rs:55-105`, `src-tauri/src/services/calibration_service.rs` (yeni; satır yok), diagnostics/export, privacy docs.
- **Dikkat:** Online model/policy sessizce kendini değiştirmez. Calibration sonucu versioned candidate policy olur ve holdout onayı bekler. Küçük örneklemde auto-accept üretilmez; class/student leakage önlenir.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml calibration`; leakage/redaction, minimum sample, train/holdout split, version invalidation ve reproducibility tests; calibration reliability/Brier/ECE raporu.

### 4.4 Seçici OCR/scoring auto-accept — P3, [BENCHMARK VE ÜRÜN ONAYI ZORUNLU]

- **Değişiklik:** Backend `AutoAcceptPolicy` yalnız holdout'ta onaylanmış low-risk bucket'ları kabul eder; random audit sampling ve kill switch zorunludur. UI policy sonucunu gösterir, threshold hesaplamaz. İlk release'te mevcut `TeacherApproved` gate default olarak korunur.
- **Dosyalar:** `src-tauri/src/domain/scoring.rs:432-437`, `src-tauri/src/services/scoring_service.rs:379-407`, OCR readiness/workflow engine, typed policy command/API, settings UI, `docs/WORKFLOW_STATES.md:35-43`.
- **Dikkat:** QEP frozen gate `src-tauri/src/domain/scoring.rs:414-441` asla gevşetilmez. Full-page/second-pass-conflict/invalid evidence/unknown provenance/consistency-review auto-accept olamaz. Policy değişimi audit ve prior candidate invalidation üretir.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml auto_accept`; default-off, every hard-block reason, random audit, kill switch, stale policy ve QEP regression tests; holdout false-accept üst sınırı karşılanmadan flag açılamaz.

### 4.5 Metric reference içeren structured analysis — P2

- **Değişiklik:** Analysis outputu `claim`, `metricRefs`, `recommendation`, `evidenceStatus` typed schema olur. Backend her metricRef'i canonical aggregate metric registry'ye bağlar; bulunmayan/çelişkili claim review/unsupported olarak işaretlenir. UI metrik linki ve teacher-facing açıklama gösterir.
- **Dosyalar:** `src-tauri/src/services/analysis_service.rs:570-593`, `src-tauri/src/domain/analysis.rs:49-71`, `src-tauri/src/services/llama_server_gateway.rs:1099-1127`, analysis commands, `src/api/types.ts:1`, `src/pages/AnalysisPage.tsx:1`, `src/pages/analysisUi.test.ts:1`, `docs/API_CONTRACTS.md:372-382`.
- **Dikkat:** Modele yalnız anonim aggregate veri gönderme davranışı korunur. Model claim'i backend metriği olmadan gerçek diye gösterilmez. Analysis uzun job olarak kalır; raw öğrenci cevabı prompta girmez.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml analysis`; valid/missing/contradictory metricRef, invalid JSON ve privacy request-capture tests; `node --test --experimental-strip-types src/pages/analysisUi.test.ts`.

### 4.6 Runtime ve KV-cache benchmark'ları — P2/P3, [BENCHMARK ZORUNLU]

- **Değişiklik:** Offline/local benchmark runner aynı versioned golden workload üzerinde q8_0/turbo3/turbo4, thread, batch, ubatch, parallel ve yalnız uyumluysa MTP adaylarını çalıştırır. Runner config üretir fakat production profile değiştirmez; kabul edilen config ayrı reviewed backend profile change olarak uygulanır.
- **Dosyalar:** `src-tauri/src/domain/model.rs:811-854`, `src-tauri/src/services/model_process_manager.rs:1355-1528`, `src-tauri/src/bin/pipeline_bench.rs` (yeni; satır yok), `src-tauri/tests/fixtures/pipeline_bench/` (yeni; satır yok), benchmark docs/diagnostic export.
- **Dikkat:** Hedef donanım MacBook Air M4 16 GB dahil her hardware fingerprint için ayrı sonuç tutulur. MTP yalnız uyumlu assistant head ve multimodal fallback testi varsa denenebilir. Tokens/second tek kabul metriği değildir; kalite/review/RAM non-regression zorunludur.
- **Doğrulama:** `cargo test --manifest-path src-tauri/Cargo.toml pipeline_bench`; fixture reproducibility/config isolation tests; tam benchmark komutu release ortamında çalıştırılır ve signed/result manifesti review edilir. Kazanan yoksa mevcut q8/default korunur.

### 4.7 Gerekirse özel HTR motoru — P3, [ÖNCE BENCHMARK ZORUNLU]

- **Değişiklik:** Yalnız Faz 2 sonrası residual handwriting error taxonomy kabul eşiğini aşarsa `HandwritingRecognizer` backend trait ve local adapter spike yapılır. HTR çıktısı typed OCR candidate olur; teacher review, provenance, privacy ve scoring gate'leri aynı kalır. Production enable ayrı karar paketidir.
- **Dosyalar:** `src-tauri/src/services/student_answer_ocr_service.rs:940-963`, `src-tauri/src/services/model_gateway.rs:13-48`, `src-tauri/src/services/handwriting_recognizer.rs` (yeni; satır yok), OCR domain/provenance/job files, local model packaging/config.
- **Dikkat:** Yeni HTR root cause olan render/registration/crop eksiklerini maskelemez. Network/cloud adapter StrictLocal'da reddedilir. Lisans, model boyutu, RAM, dil kapsamı ve student-data privacy ayrı release gate'idir.
- **Doğrulama:** Aynı holdout crop setinde mevcut vision OCR'a karşı CER/WER, critical term, table/annotation schema, p95, RAM ve review/correction oranı. Belirlenmiş kalite-maliyet eşiği geçmezse hiçbir production kod yolu açılmaz.

## Faz 4 doğrulama ve çıkış kriteri

1. Anchor yalnız explicit teacher command ile oluşur; stale QEP/policy anchor'ı reuse edilmez.
2. Similarity yalnız review önceliği sağlar; puan otomatik kopyalamaz.
3. Calibration reproducible ve privacy-safe'tir; küçük örneklem policy değiştirmez.
4. Auto-accept default-off'tur; holdout false-accept, random audit ve kill-switch kapıları geçmeden açılmaz.
5. Analysis claim'lerinin tamamı valid metricRef taşır veya unsupported olarak görünür.
6. KV/runtime config yalnız benchmark manifestiyle değişir; sonuç yoksa mevcut default korunur.
7. HTR yalnız residual hata kanıtı ve A/B üstünlüğü varsa ayrı production kararı olur.
8. Ortak kalite komutları, Tauri smoke ve privacy/golden benchmark doğrulamaları geçer.

---

# 4. Fazlar arası önerilen teslim sırası

1. Faz 1 tek release train olarak değil, 1.1–1.4 correctness/hot-path, 1.5–1.9 policy/prompt, 1.10–1.12 runtime/privacy/cache şeklinde küçük PR'lara bölünür. Her PR bağımsız test ve migration kanıtı taşır.
2. Faz 2'ye geçmeden anonim golden set baseline dondurulur. 2.1–2.4 geometry/source, 2.5–2.9 OCR inference contract, 2.10–2.12 extraction verimliliği olarak ayrı teslim edilir.
3. Faz 3 answer type bazında kademeli açılır. Önce deterministic scorer + candidate cache, sonra rubric levels/semantic mapping, en son duplicate/consistency devreye alınır.
4. Faz 4'te hiçbir auto-accept/runtime/HTR adımı salt geliştirme tamamlandı diye açılmaz; benchmark ve ürün/pedagoji onayı ayrı release gate'idir.

# 5. Nihai sayısal özet

- **Faz sayısı:** 4
- **Numaralı uygulama maddesi:** 38 (Faz 1: 12, Faz 2: 12, Faz 3: 7, Faz 4: 7)
- **Kodda doğrulanan iddia:** 31
- **Doğrulanamayan iddia:** 5
- **Doğrulanamama dağılımı:** 1 speaking mevcut-durum iddiası (kod zaten text-only), 4 benchmark-bağımlı performans/HTR iddiası
- **Bu planlama çalışmasında kaynak kod değişikliği:** 0
- **Oluşturulan tek dosya:** `docs/UYGULAMA_PLANI.md`
