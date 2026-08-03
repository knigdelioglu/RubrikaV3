# Dört Raporun Ortak Sonucu (Girdi Raporu)

Dört rapor birbirini büyük ölçüde doğruluyor. Sorun esas olarak **Gemma 4 12B'nin yetersiz olması değil**; modelin çevresindeki pipeline'ın:

* gereksiz çağrılar yapması,
* görüntü bilgisini modelden önce kaybetmesi,
* dinamik veriyi prompt kurallarıyla karıştırması,
* model kararını gereğinden fazla serbest bırakması,
* aynı girdinin tekrarında aynı sonucu garanti etmemesi.

Uygulamadaki local LLM kullanımı dokuz işlevsel çağrı ve bir completion probe'dan oluşuyor. Ortak sorunlar çağrıların çoğunda tekrar ediyor.

Ben önerileri üç gruba ayırıyorum:

1. **Kesin hata veya gereksiz maliyet — hemen yapılmalı**
2. **Doğru mimari geliştirme — kontrollü aşamada yapılmalı**
3. **Benchmark olmadan uygulanmamalı**

---

# 1. Hemen yapılması gerekenler

Bunlar model kalitesini değiştirmeden güvenlik, doğruluk veya performans kazancı sağlayabilecek işlerdir.

## A. Completion probe'u hot path'ten kaldırmak

Her model çağrısından önce:

```text
Reply with exactly one word: ok
```

ürettirmek gerçek bir generation işlemidir. Health kontrolü olarak kullanılmamalı. Dördüncü rapor da bunun domain çağrısı olmadığını ve gereksiz gecikme oluşturabileceğini doğruluyor.

Doğru ayrım:

```text
/health veya process readiness
→ runtime lease
→ gerçek domain çağrısı
```

Gerçek completion probe yalnız:

* tanılama,
* model ayar testi,
* manuel benchmark

için kullanılmalı.

**Karar: Yapılsın. P0.**

---

## B. Readiness ve lease kontrollerini tek akışta birleştirmek

Job başlangıcı, runtime acquire, lease ve tekrar readiness probe birbirinden bağımsız çalışmamalı.

Tek backend fonksiyonu olmalı:

```text
acquire_ready_runtime_lease(profile)
```

Bu fonksiyon:

* doğru modeli seçmeli,
* gerekiyorsa tek kez başlatmalı,
* readiness'i tek kez doğrulamalı,
* lease döndürmeli,
* aynı job içinde tekrar probe yapmamalı.

Sabit 200 ms uyku yerine startup single-flight veya bounded readiness wait kullanılmalı.

**Karar: Yapılsın. P0.**

---

## C. `needs_review` puanlarını final toplamdan çıkarmak

İkinci rapordaki en önemli doğrudan doğruluk hatası budur. `needs_review=true` olan sonuçlar teknik olarak `scoringApplied=true` kalabiliyor ve frontend toplamına girebiliyor.

Üç ayrı kavram olmalı:

```text
provisionalScore
acceptedScore
finalScore
```

Final toplam yalnız:

* backend tarafından otomatik kabul edilmiş veya
* öğretmen tarafından onaylanmış

kayıtlardan oluşmalı.

`needs_review`, `parse_failed`, `evidence_invalid`, `consistency_review` gibi kayıtlar provisional olabilir ama final notta kullanılmamalı.

**Karar: Kesinlikle yapılsın. P0.**

---

## D. Kriter eşleştirme normalizasyonunu düzeltmek

Kriter tamamlığı case-insensitive kontrol edilirken normalize aşamasının birebir başlık karşılaştırması yapması gerçek bir tutarsızlık. Model yalnız büyük-küçük harf farkıyla kriter döndürdüğünde önce kabul edilip sonra sıfırlanabilir.

En doğru yöntem:

* modelden başlık yerine canonical `criterionId` istemek,
* ID eşleşmesini zorunlu tutmak,
* başlığı yalnız teacher-facing görüntülemek,
* duplicate veya unknown criterion ID'yi reddetmek.

**Karar: Yapılsın. P0.**

---

## E. Prompt kurallarıyla dinamik veriyi ayırmak

Dördüncü raporun en güçlü ortak bulgusu bu. Öğrenci cevabı, soru metni, transcript, görev metni ve rubrik birçok yerde `system` prompt içine gömülüyor. Bunlar değişmez talimat değil, güvenilmeyen veridir.

Doğru yapı:

```text
system:
- değişmez görev politikası
- güvenlik kuralları
- çıktı kontratı

user:
- structured input data
- question text
- student answer
- rubric
- transcript
- metrics
```

Dinamik veriler mümkünse JSON veri bloğu olarak gönderilmeli. String interpolation ile prompt cümlesinin içine gömülmemeli.

Bu değişiklik özellikle:

* soru çıkarma,
* rubrik çıkarma,
* OCR,
* scoring,
* speaking cleanup,
* speaking evaluation,
* analysis

için yapılmalı.

**Karar: Yapılsın. P0.**

---

## F. Bütün model çağrılarını versioned schema kontratına bağlamak

JSON zorlama şu anda tutarsız. Bazı akışlarda var, bazı akışlarda yok. Prompt sürümlemesi de eksik veya birbiriyle uyuşmuyor.

Her model isteği şu provenance alanlarına sahip olmalı:

```text
useCase
promptVersion
schemaVersion
policyVersion
modelFingerprint
runtimeFingerprint
samplingParameters
```

Her structured çağrıda:

* destekleniyorsa `response_format`,
* mümkünse JSON schema/grammar,
* her durumda backend schema validation

kullanılmalı.

Schema desteği server tarafından sağlanmasa bile backend validation zorunlu kalmalı.

**Karar: Yapılsın. P0/P1.**

---

## G. OCR issue correction'dan cevap anahtarı ipucunu çıkarmak

`critical_term_hint`, OCR ikinci görüşüne rubrikten veya cevap anahtarından beklenen doğru terimi taşıyor. Bu, modelin görseldeki yazıyı okumak yerine doğru cevaba yaklaşmasına neden olabilir. Rapor bunu en kritik prompt risklerinden biri olarak sınıflandırıyor.

İki aşama ayrılmalı:

1. **Saf görsel ikinci okuma**

   * yalnız crop,
   * observed text,
   * konum bilgisi.

2. **Bağlamsal öneri**

   * öğretmen incelemesi için,
   * otomatik canonical OCR'a uygulanmadan,
   * cevap anahtarı kanıt sayılmadan.

Saf OCR çağrısına rubric/answer-key ipucu gönderilmemeli.

**Karar: Kesinlikle yapılsın. P0.**

---

## H. Backend ve frontend güven eşiklerini tekleştirmek

Backend `0.72`, UI filtresi `0.6` kullanıyorsa bazı backend-review kayıtları öğretmenin düşük güven filtresinde görünmeyebilir. Türkçe normalizasyonda `to_ascii_lowercase()` kullanılması da `İ/ı`, `I/i`, `Ç/ç` gibi eşleşmeleri bozabilir.

Tek canonical policy tanımlanmalı:

```text
OcrReviewPolicy
- lowConfidenceThreshold
- criticalTermThreshold
- emptyInkMismatchThreshold
- secondPassThreshold
```

Frontend bu değerleri backend DTO'sundan almalı; kendi eşiklerini üretmemeli.

Unicode-aware Türkçe normalizasyon kullanılmalı.

**Karar: Yapılsın. P0.**

---

## I. Speaking değerlendirmesini gerçek text-only profile geçirmek

Speaking rubric evaluation yalnız transcript/metin kullanıyorsa mmproj yükleyen `standard` profile bağlı olmamalı.

Önce profil kullanımında gerçekten görüntü bulunmadığı doğrulanmalı. Ardından:

* text-only runtime,
* mmproj yok,
* aynı model dosyası,
* aynı prompt/schema,
* aynı golden çıktılar

ile regression testi yapılmalı.

**Karar: Doğrulanırsa yapılsın. P0/P1.**

---

## J. Model giriş JPEG cache'ini gerçek cache yapmak

Mevcut çıktı bulunmasına rağmen silinip tekrar oluşturuluyorsa cache semantiği yanlış.

Cache anahtarı:

```text
source hash
+ crop regions
+ alignment transform
+ preprocess mode
+ resize policy version
+ JPEG quality
+ encoder version
```

olmalı.

Cache sonucu:

* manifest ile doğrulanmalı,
* atomik yazılmalı,
* source veya policy değişince invalidated olmalı.

**Karar: Yapılsın. P1.**

---

# 2. OCR tarafında yapılması gereken asıl mimari geliştirme

Üçüncü raporun en önemli sonucu şu: **OCR doğruluğundaki ana kayıp prompttan önce meydana geliyor.**

## A. İnsan önizlemesi ile OCR render'ını ayırmak

Mevcut yaklaşık 144 DPI render, küçük veya ince el yazısında bilgi kaybettirebilir. Crop küçültülüyor veya büyütülmeden 1800 px sınara gönderiliyor.

Doğru mimari:

```text
düşük/orta çözünürlüklü UI preview
+
300–400 DPI OCR source render
```

Ancak yüksek çözünürlüklü tam sayfayı doğrudan modele göndermek doğru değil. Şu sıra kullanılmalı:

```text
yüksek çözünürlükte render
→ sayfa hizalama
→ yüksek çözünürlüklü crop
→ crop kalite analizi
→ kontrollü resize/upscale
→ model input
```

Böylece model token maliyeti kontrol altında tutulurken el yazısı ayrıntısı korunur.

**Karar: Yapılsın. P1 ve golden set zorunlu.**

---

## B. Multi-region ve multi-page crop modeli

Tek soru için yalnız tek sayfa ve tek dikdörtgen varsayımı uzun cevaplarda veri kaybı yaratabilir.

Yeni domain yapısı örneği:

```text
QuestionAnswerTemplate
- questionId
- regions[]
  - pageOffset
  - order
  - normalizedBbox
  - regionRole
  - continuationPolicy
```

Böylece:

* bir cevap birden çok kutudan,
* birden çok sayfadan,
* devam alanlarından

oluşabilir.

`page_numbers.len() > 1` doğrudan `partial_answer_suspected` anlamına gelmemeli. Gerçek coverage kontrolü yapılmalı.

**Karar: Yapılsın. P1.**

---

## C. Sayfa registration, deskew ve perspektif düzeltme

Aynı normalized crop'un her taramaya uygulanması, sayfa birkaç derece eğik veya kaymışsa yanlış bölgeyi kesebilir. Rapor, crop doğruluğu açısından registration'ı kontrast işlemlerinden daha önemli görüyor.

Önerilen zincir:

```text
page boundary detection
→ deskew
→ perspective correction
→ template anchor registration
→ aligned crop
→ ink-aware padding
```

Bu işi sadece görüntü filtresi ekleyerek çözmeye çalışmamak gerekir.

**Karar: Yapılsın. P1; test fixture gerektirir.**

---

## D. Full-page OCR'ı normal production yolu olmaktan çıkarmak

Crop yokken bütün sayfayı modele gönderip review'a düşürmek güvenli görünse de modelin hangi cevabı çıkardığı belirsizdir.

İki mod olmalı:

```text
Production OCR:
crop/region zorunlu

Experimental full-page OCR:
review-only
scoring'e geçemez
açık uyarı gösterir
```

**Karar: Yapılsın. P1.**

---

## E. Preprocess'i adaptive ve lazy yapmak

Beş varyantın tamamını üretip yalnız `handwriting-enhanced` kullanmak hem maliyetli hem mantıksal olarak zayıf.

Önerilen yapı:

```text
image quality profile
→ varsayılan tek varyant
→ ilk OCR
→ risk sinyali yoksa bitir
→ risk varsa alternatif varyantı tembel üret
→ ikinci OCR
→ karşılaştır
```

Kalite profili en az:

* kontrast,
* stroke yoğunluğu,
* blur,
* clipping,
* boşluk/mürekkep oranı,
* crop boyutu,
* skew

ölçmeli.

**Karar: Yapılsın ama golden set sonrasında. P1.**

---

## F. Seçici ikinci OCR geçişi

Her cevabı iki kez okutmak gereksiz maliyet yaratır. Ancak rapordaki seçici ikinci geçiş önerisi güçlüdür.

İkinci geçiş tetikleyicileri:

* confidence düşük,
* crop kenara dayanıyor,
* image ink ile empty result çelişiyor,
* structured schema eksik,
* sayı/sembol ağırlıklı cevap,
* `finish_reason=length`,
* kritik terim belirsiz,
* ilk iki deterministic analiz çelişkili.

İkinci sonuç ilk sonucu otomatik ezmemeli. Uyuşmazlık varsa karşılaştırmalı teacher review açılmalı.

**Karar: Yapılsın. P1.**

---

## G. Answer type'a özel `structuredAnswer` şemaları

Serbest `serde_json::Value` uzun vadede kabul edilmemeli.

Örneğin:

```text
MultipleChoiceAnswer
MatchingAnswer
OrderedSlotsAnswer
NumericAnswer
TableAnswer
CorrectionTableAnswer
SentenceAnnotationAnswer
GrammarAnalysisAnswer
OpenTextAnswer
```

Her biri typed Rust enum/struct olmalı.

Schema geçersizse:

```text
scoringApplied = false
needsReview = true
```

olmalı.

**Karar: Yapılsın. P1.**

---

# 3. Scoring mimarisinde yapılması gerekenler

## A. Deterministik cevap türlerini modelden çıkarmak

İkinci raporun en güçlü mimari önerilerinden biri budur. Çoktan seçmeli, doğru-yanlış, eşleştirme, sıralama, sayısal, boşluk doldurma ve yapılandırılmış tablolar mümkün olduğunca Rust tarafından puanlanmalı.

Model yalnız gerçekten semantik değerlendirme gerektiren:

* açık uçlu,
* yorum,
* gerekçelendirme,
* anlam ilişkisi,
* kanıt değerlendirmesi

gibi alanlarda kullanılmalı.

Bu değişiklik:

* hız,
* tutarlılık,
* açıklanabilirlik,
* öğretmen yükü

açısından çok değerlidir.

**Karar: Yapılsın. P1/P2.**

---

## B. Modelin doğrudan serbest puan vermesini azaltmak

Modelin `7.4` veya `6/10` gibi serbest sayı üretmesi yerine canonical seviye seçmesi daha güvenlidir.

Örnek:

```text
none
limited
partial
complete
```

Fakat seviyeler her kriter için açıkça tanımlanmalı:

```text
CriterionLevel
- id
- title
- requiredConditions
- disqualifyingConditions
- score
- evidenceRequired
```

Model:

* `levelId`,
* evidence,
* missing requirement,
* contradiction

döndürür.

Rust:

* level → score,
* criterion total,
* evidence validation,
* final score

hesaplar.

Speaking tarafı zaten buna yakın bir mimari kullanıyor ve rapor bunu güçlü buluyor.

**Karar: Yapılsın; fakat rubrik migration ve golden scoring seti gerektirir. P2.**

---

## C. Exact scoring fingerprint ve cache

Aynı cevap için yüzde 100 aynı sonucu ancak cache ile garanti edebiliriz. Sabit seed tek başına yeterli değildir. İkinci rapor model, runtime, prompt ve schema fingerprint'lerinin mevcut hash'e dahil olmadığını belirtiyor.

Ancak burada önemli bir ayrım yapılmalı:

### Model candidate cache

Aynı tam fingerprint için model cevabını yeniden çağırmadan geri döndürebilir.

### Final decision cache

Yalnız:

* deterministik scorer sonucu veya
* teacher-approved karar

final olarak yeniden kullanılmalı.

Ham model sonucu yanlışsa exact cache yanlışlığı da kalıcılaştırabilir. Bu yüzden cache:

```text
candidate cache ≠ final teacher decision
```

olmalı.

Fingerprint en az:

```text
QEP hash
answer hash
OCR generation hash
prompt version
schema version
policy version
model file hash
runtime version
sampling parameters
anchor/calibration version
```

içermeli.

**Karar: Yapılsın. P1/P2.**

---

## D. Anchor cevaplar ve benzerlik kümeleri

Bu fikir pedagojik olarak değerli fakat daha karmaşık. Sistem şu anda benzer cevapları karşılaştırmıyor ve teacher-approved cevapları anchor olarak kullanmıyor.

Doğru kullanım:

* exact duplicate → güvenli cache adayı,
* yüksek benzerlik → karşılaştırma ve tutarlılık uyarısı,
* teacher-approved örnek → referans,
* yakın cevap → puanı otomatik kopyalama yok.

Negasyon, sayı, birim, tarih ve kritik kavram farkları benzerlik skorundan bağımsız kontrol edilmeli.

Bu işi ilk aşamada embedding modeliyle başlatmak zorunlu değil. Önce:

* normalized exact match,
* token overlap,
* BM25/FTS,
* kritik kavram farkı,
* sayısal değer farkı

kullanılabilir.

**Karar: Yapılsın ama ilk iki fazdan sonra. P2/P3.**

---

## E. Öğretmen yükünü azaltma önerisi hemen uygulanmamalı

Bütün yüksek güvenli OCR sonuçlarını otomatik onaylamak cazip olsa da gerçek öğrenci verisinde CER/WER benchmark'ı olmadan mevcut teacher gate gevşetilmemeli.

İlk aşamada:

* öğretmene review önceliği sunulabilir,
* düşük riskli kayıtlar ayrı grupta gösterilebilir,
* rastgele kalite örnekleme yapılabilir,

ama canonical `TeacherApproved` şartı hemen kaldırılmamalı.

Golden set doğrulamasından sonra seçici auto-accept düşünülebilir.

**Karar: Şimdilik gate korunmalı.**

---

# 4. Soru ve rubrik çıkarma tarafı

## A. Tüm sayfaları her soru için göndermemek

İlk, üçüncü ve dördüncü rapor bunu birlikte doğruluyor. Soru veya rubrik için her hedefte tüm belgeyi tekrar göndermek görüntü prefill maliyetini katlıyor. Dördüncü rapor soru çıkarma sırasında modelin hedef dışındaki soruları da gördüğünü belirtiyor.

Doğru yapı:

```text
PDF text/marker analysis
→ question-to-page candidates
→ hedef sayfa
→ gerekirse ±1 sayfa
→ belirsizlikte geniş fallback
```

Tek sayfa zorunluluğu doğru değil; **sınırlı sayfa penceresi** doğru çözümdür.

**Karar: Yapılsın. P1.**

---

## B. Rubrik prompt'unu canonical rubrik modeliyle eşitlemek

Mevcut prompt, canonical modelde bulunan:

* partial credit,
* zero score conditions,
* common mistakes

gibi alanları istemiyor. Bu yüzden kaynak cevap anahtarındaki bilgi parse aşamasında kaybolabilir.

Rubrik prompt ve schema doğrudan canonical Rubric DTO'dan üretilmeli. Prompt ve domain modeli ayrı ayrı elle tutulmamalı.

Retry sırasında schema yeniden ve eksiksiz verilmelidir.

**Karar: Yapılsın. P1.**

---

## C. Parse retry zincirini ucuzlatmak

İlk parse hatasında tüm multimodal isteği tekrarlamak yerine:

```text
schema/grammar ilk çağrı
→ deterministic salvage
→ text-only JSON repair
→ son çare multimodal retry
```

uygulanmalı.

Text-only repair çağrısına görseller yeniden gönderilmemeli.

**Karar: Yapılsın. P1.**

---

# 5. Analysis raporu

Analysis şu anda anonim agregalarla çalıştığı için veri güvenliği bakımından iyi; fakat serbest metin iddiaları metriklere otomatik bağlanmıyor.

Önerilen structured output doğru:

```text
claim
metricRefs
recommendation
evidenceStatus
```

Ancak bu OCR ve scoring doğruluğu kadar acil değil.

**Karar: Daha sonra yapılsın. P2.**

---

# 6. Local-only çalışma

Uygulamanın "local" olması kullanıcı beklentisiyse, varsayılan URL'nin loopback olması yeterli değildir. External profile desteği öğrenci verisinin dış endpoint'e gönderilebilmesi anlamına gelir.

Ben global olarak external desteği tamamen silmek yerine bir policy öneriyorum:

```text
PrivacyMode::StrictLocal
PrivacyMode::ExplicitExternal
```

`StrictLocal` varsayılan olmalı ve şunları zorunlu tutmalı:

* loopback URL,
* tercihen managed process,
* model fingerprint,
* remote redirect reddi,
* proxy kullanımını engelleme,
* öğrenci verisi taşıyan use-case'lerde external profile reddi.

External kullanım ancak açık kullanıcı onayı ve güçlü uyarıyla açılmalı.

**Karar: Kullanıcının local-only hedefi varsa yapılsın. P0/P1.**

---

# 7. Benchmark yapılmadan uygulanmaması gerekenler

Şu öneriler doğrudan production varsayılanı yapılmamalı:

## KV cache `q8_0 → turbo3/turbo4`

A/B test gerektirir. Ölçülecekler:

* RAM,
* prefill,
* decode,
* OCR CER/WER,
* rubrik alan kaybı,
* scoring tutarlılığı,
* teacher review oranı.

## Threads, batch, ubatch ve parallel

Donanıma ve gerçek prompt boyutlarına bağlıdır. MacBook Air M4 16 GB üzerinde golden workload ile benchmark edilmelidir.

## MTP/speculative decoding

Multimodal çağrılarda fallback yapıyorsa OCR/rubrik için öncelikli değildir. Uyumlu 12B assistant head olmadan denenmemeli.

## Özel HTR motoru

Uzun vadede iyi bir mimari olabilir; fakat önce yüksek çözünürlük, registration ve doğru crop uygulanmalı. Bunlar düzeltilmeden yeni HTR motoru eklemek kök nedeni gizleyebilir.

---

# Benim önerdiğim tek çalışma sırası

## Faz 1 — Hata, güvenlik ve gereksiz maliyet

1. Completion probe'u hot path'ten kaldır.
2. Runtime readiness ve lease'i birleştir.
3. `needs_review` puanlarını final toplamdan çıkar.
4. Criterion ID eşleştirmesini düzelt.
5. Backend/UI confidence policy'yi tekleştir.
6. Türkçe Unicode normalizasyonunu düzelt.
7. Prompt/system-data ayrımını uygula.
8. Versioned schema ve provenance ekle.
9. `critical_term_hint` bilgisini saf OCR correction'dan çıkar.
10. Speaking evaluation'ı text-only profile geçir.
11. Strict local policy ekle.
12. Gerçek JPEG content cache oluştur.

Bu faz doğruluk davranışını bilinçli şekilde değiştirmeden uygulanabilir.

---

## Faz 2 — OCR görüntü zinciri

1. OCR'a özel yüksek çözünürlüklü render.
2. Registration/deskew/perspective.
3. Multi-region ve multi-page crop.
4. Full-page OCR'ı review-only moda al.
5. Adaptive preprocess.
6. Seçici ikinci okuma.
7. Answer-type typed structured schemas.
8. Dynamic image/output token budget.
9. OCR provenance ve gerçek model input metadata.

Bu faz için gerçek öğrenci taramalarından anonim golden set gerekir.

---

## Faz 3 — Deterministik scoring

1. Deterministik answer-type scorers.
2. Scoring fingerprint ve candidate cache.
3. Provisional/final score ayrımı.
4. Structured rubric levels.
5. Model level/evidence üretir; Rust puanlar.
6. Exact duplicate reuse.
7. Consistency review.

---

## Faz 4 — Kalibrasyon ve gelişmiş tutarlılık

1. Teacher-approved anchor cevaplar.
2. Benzer cevap kümeleri.
3. Teacher düzeltmelerine dayalı confidence kalibrasyonu.
4. Seçici OCR/scoring auto-accept.
5. Structured analysis with metric references.
6. Runtime ve KV-cache benchmark'ları.
7. Gerekirse özel HTR motoru.

---

# Ölçüm yapılmadan hiçbir performans kararını kabul etmeyin

Yeni benchmark altyapısı şu metrikleri birlikte ölçmeli:

```text
iş başına model çağrısı
p50/p95 toplam süre
prefill süresi
decode süresi
görüntü token sayısı
input/output token sayısı
CPU zamanı
peak RAM
retry oranı
OCR CER/WER
kritik terim hata oranı
structured schema başarı oranı
scoring exact-repeat oranı
teacher review oranı
teacher correction oranı
```

Yalnız `tokens/second` ölçmek yanıltıcı olur.

## Nihai değerlendirme

Dört raporun önerilerinin çoğu doğru; fakat hepsi aynı anda uygulanmamalı.

**Şimdi yapılması gereken esas paket:**

```text
hot-path optimizasyonu
+ prompt/schema güvenliği
+ scoring correctness düzeltmeleri
+ gerçek image cache
```

Ardından:

```text
yüksek çözünürlüklü OCR
+ registration
+ multi-region crop
+ adaptive ikinci okuma
```

Son olarak:

```text
deterministik scoring
+ scoring fingerprint/cache
+ rubric levels
+ anchor/consistency
```

Modeli değiştirmek, KV cache quant değiştirmek, MTP eklemek veya yeni HTR motoruna geçmek şu anda ilk adım olmamalı.
