# RubrikaV3 — Local Model Platform Migration Plan

Bu plan, RubrikaV3'ün mevcut Gemma 4 12B + llama.cpp entegrasyonunu bozmadan; yeni yerel modellerin kod değişikliği gerektirmeden eklenebildiği, görev bazında seçilebildiği, capability probe ve golden benchmark ile doğrulandığı model-bağımsız bir platform yapısına geçişi tanımlar.

## Takip Listesi

- [ ] Sprint 0 — Mevcut davranışı kilitle ve migration sınırını tanımla
- [ ] Sprint 1 — `ModelDefinition`, `RuntimeDefinition` ve `TaskProfile` ayrımını getir
- [ ] Sprint 2 — Runtime argümanlarını koddan çıkar ve `LlamaCppRuntimeAdapter` katmanını oluştur
- [ ] Sprint 3 — Model Registry + Capability Negotiation altyapısını kur
- [ ] Sprint 4 — Task → Model Binding ve model router katmanını devreye al
- [ ] Sprint 5 — Experimental / Production model yaşam döngüsü ve Model Laboratuvarı UI'sını ekle
- [ ] Sprint 6 — Golden benchmark karşılaştırma ve promotion gate'lerini bağla
- [ ] Sprint 7 — Gemma özel sabitleri kaldır, compatibility migration yap ve eski profilleri emekli et
- [ ] Sprint 8 — Final regression, rollback doğrulaması ve production gate

---

## 1. Hedef Mimari

Geçiş sonunda hedef akış:

```text
Rubrika Domain
    ↓
Task Contract
    ↓
Model Router
    ↓
Task → Model Binding
    ↓
Model Registry
    ↓
Capability Negotiation
    ↓
Inference Runtime
    ↓
LlamaCppRuntimeAdapter
    ↓
llama-server
    ↓
Local Model
```

İleride aynı `InferenceRuntime` sözleşmesinin altına başka adapter'lar eklenebilir:

```text
InferenceRuntime
├── LlamaCppRuntimeAdapter   ← production/default
├── MlxRuntimeAdapter        ← gelecekte opsiyonel
└── ExternalCompatibleAdapter← yalnız açık izinli kullanım
```

Bu migration'ın temel ilkesi şudur:

> Domain servisleri hiçbir model ailesini, GGUF dosyasını, mmproj yolunu veya llama.cpp argümanını bilmemelidir.

---

# Sprint 0 — Mevcut davranışı kilitle ve migration sınırını tanımla

## Amaç

Mevcut Gemma 4 12B production davranışını referans kabul ederek migration sırasında sessiz regresyon oluşmasını engellemek.

## Yapılacaklar

- Mevcut model akışlarının envanterini çıkar:
  - Question text extraction
  - Rubric extraction
  - Student answer OCR
  - OCR issue correction
  - Semantic scoring
  - Speaking transcript cleanup
  - Speaking rubric evaluation
  - General/analysis model calls
- Mevcut profile ID, prompt version, schema version, policy version ve runtime fingerprint ilişkilerini dokümante et.
- `Gemma 4 12B` mevcut production baseline olarak işaretlensin.
- `MODEL_GATEWAY.md`, `MODEL_RUNTIME_OWNERSHIP.md`, golden benchmark ve ilgili testler migration reference olarak sabitlensin.
- Migration boyunca değiştirilmemesi gereken güvenlik invariant'larını açıkça yaz:
  - UI model endpoint'ine doğrudan çağrı yapmaz.
  - Student-data çağrıları strict-local sınırını korur.
  - Model skoru doğrudan persist edilmez.
  - Semantic scoring canonical rubric level → Rust score mapping kullanır.
  - OCR/scoring invalid JSON veya schema mismatch durumunda fail-closed kalır.
  - Prompt/schema/policy/model/runtime provenance kaybolmaz.

## Kabul Kriterleri

- Mevcut Gemma production davranışı baseline olarak kaydedilmiş olmalı.
- Migration sonrasında karşılaştırılacak regression fixture/golden set listesi net olmalı.
- Hiçbir domain davranışı bu sprintte değişmemeli.

---

# Sprint 1 — `ModelDefinition`, `RuntimeDefinition` ve `TaskProfile` ayrımını getir

## Amaç

Bugünkü `ModelProfile` içinde birleşmiş olan üç farklı sorumluluğu ayırmak:

1. Modelin kimliği ve yetenekleri
2. Runtime'ın nasıl çalıştırılacağı
3. Görevin modelden ne beklediği

## Yeni Domain Yapıları

### `ModelDefinition`

Örnek alanlar:

```text
id
family
display_name
model_path
mmproj_path?
format
quantization?
capabilities
context_limit?
metadata
model_fingerprint
lifecycle_state
```

`capabilities` en az şu alanları desteklemeli:

```text
text
vision
structured_json
json_schema
thinking_control
multimodal_projector_required
```

### `RuntimeDefinition`

Örnek alanlar:

```text
id
engine
server_path
host
port
context_size
gpu_layers
flash_attention
parallel
batch_size
ubatch_size
kv_cache_type_k
kv_cache_type_v
reasoning_mode
extra_args
```

### `TaskProfile`

Örnek görev alanları:

```text
id
use_case
required_capabilities
prompt_version
schema_version
policy_version
temperature
top_k
top_p
seed
max_tokens
timeout_seconds
response_format
```

## Migration Kuralı

Mevcut `ModelProfile` hemen silinmemeli.

Geçici compatibility adapter kullanılmalı:

```text
Legacy ModelProfile
        ↓
ProfileMigrationAdapter
        ↓
ModelDefinition + RuntimeDefinition
```

## Kabul Kriterleri

- Domain katmanında model kimliği ile runtime preset ayrılmış olmalı.
- Mevcut Gemma profilinin yeni yapılara lossless dönüşümü mümkün olmalı.
- Legacy config açıldığında kullanıcı ayarı kaybolmamalı.

---

# Sprint 2 — Runtime argümanlarını koddan çıkar ve `LlamaCppRuntimeAdapter` katmanını oluştur

## Amaç

`build_model_server_args()` ve preset tabanlı Gemma/runtime özel davranışlarını veri odaklı hale getirmek.

## Yeni Arayüz

```text
InferenceRuntime
├── start(runtime, model)
├── stop(instance)
├── health(instance)
├── probe(instance)
├── capabilities(instance)
├── complete(request)
├── fingerprint(instance)
└── preview_args(runtime, model)
```

İlk implementasyon:

```text
LlamaCppRuntimeAdapter
```

## Koddan Çıkarılacak Sabit Davranışlar

- `-ngl 99`
- sabit context size
- sabit batch / ubatch
- KV cache tipi
- `--parallel`
- `--reasoning off`
- image min/max token argümanları
- mmproj kullanım zorunluluğu
- preset'e bağlı runtime farkları

Bunlar `RuntimeDefinition` ve capability negotiation üzerinden üretilmeli.

## Güvenlik Kuralları

- Kullanıcı tarafından verilen keyfi `extra_args` doğrudan çalıştırılmamalı.
- Allowlist/denylist uygulanmalı.
- `--host` strict-local modda loopback dışına çıkamamalı.
- Port ownership ve managed-process identity mekanizması korunmalı.
- Runtime fingerprint argümanlardan deterministik üretilmeli.

## Kabul Kriterleri

- Gemma 4 mevcut production server args çıktısı semantik olarak eşdeğer kalmalı.
- `ModelRuntimePreset` production karar mekanizması olmaktan çıkarılmalı.
- Speaking cleanup ve speaking evaluation için ayrı model process başlatılmamalı; mevcut shared-runtime davranışı korunmalı.

---

# Sprint 3 — Model Registry + Capability Negotiation altyapısını kur

## Amaç

Yeni bir GGUF/model eklendiğinde Rubrika'nın modeli tanıyıp görev uyumluluğunu otomatik belirleyebilmesi.

## Model Registry

Registry en az şunları saklamalı:

```text
model_definition
runtime_definition
capability_probe_result
model_fingerprint
runtime_fingerprint
last_verified_at
benchmark_status
lifecycle_state
```

## Capability Probe

Yeni model için sıralı probe:

```text
file validation
    ↓
runtime startup
    ↓
health probe
    ↓
text completion probe
    ↓
structured JSON probe
    ↓
JSON schema probe
    ↓
vision probe (model vision bildiriyorsa)
    ↓
thinking/reasoning behavior probe
    ↓
capability manifest
```

## Capability Sonuçları

Örnek:

```text
text: pass
vision: pass
structured_json: pass
json_schema: partial
thinking_control: pass
```

`partial` capability production görevi için otomatik kabul edilmemeli.

## Kabul Kriterleri

- Yeni model registry'ye eklenebilmeli.
- Model kod değişikliği olmadan capability probe'dan geçebilmeli.
- Uyumlu olmadığı task'lar otomatik olarak seçilemez olmalı.
- Probe sonucu model fingerprint ile ilişkilendirilmeli; model dosyası değişirse doğrulama geçersizleşmeli.

---

# Sprint 4 — Task → Model Binding ve model router katmanını devreye al

## Amaç

Her görevin bağımsız model seçebilmesini sağlamak.

## Yeni Binding Modeli

```text
student_answer_ocr        → gemma4-12b
ocr_issue_correction      → gemma4-12b
rubric_extraction         → gemma4-12b
semantic_scoring          → gemma4-12b
speaking_cleanup          → gemma4-12b
speaking_evaluation       → gemma4-12b
```

Yeni model eklendiğinde örneğin yalnız scoring değiştirilebilmeli:

```text
semantic_scoring          → experimental-model-x
```

Diğer görevler Gemma'da kalmalı.

## Router Sorumlulukları

Router şu kontrolleri yapmalı:

1. TaskProfile bulunuyor mu?
2. Task için binding var mı?
3. Model registry'de mevcut mu?
4. Model gerekli capability'lere sahip mi?
5. Model production kullanımına izinli mi?
6. Runtime uygun mu?
7. Privacy policy izin veriyor mu?
8. Runtime lease alınabiliyor mu?

Herhangi biri başarısızsa typed error + recovery action üretmeli.

## Fallback Politikası

Sessiz fallback yapılmamalı.

Örnek:

```text
scoring → Model X
Model X unavailable
```

sistem otomatik Gemma'ya dönmemeli.

Doğru davranış:

```text
MODEL_BINDING_UNAVAILABLE
Suggested action:
- Gemma'ya geç
- Model X'i başlat
- Binding'i değiştir
```

Bu karar audit/provenance açısından görünür kalmalı.

## Kabul Kriterleri

- Aynı exam pipeline içinde farklı görevlerde farklı modeller kullanılabilmeli.
- Provenance hangi task'ın hangi model/runtime ile çalıştığını açıkça göstermeli.
- Kullanıcı onayı olmadan silent model substitution yapılmamalı.

---

# Sprint 5 — Experimental / Production model yaşam döngüsü ve Model Laboratuvarı UI'sını ekle

## Amaç

Yeni modelleri gerçek öğrenci akışına girmeden güvenli biçimde denemek.

## Lifecycle

```text
Imported
  ↓
Probing
  ↓
Compatible
  ↓
Experimental
  ↓
Benchmark Verified
  ↓
Production
```

Alternatif son durumlar:

```text
Unsupported
Probe Failed
Benchmark Failed
Disabled
```

## Model Laboratuvarı UI

Ayarlar altında yeni yüzey:

```text
Ayarlar
└── Yerel Modeller
    ├── Modeller
    ├── Runtime
    ├── Görev Atamaları
    └── Benchmark
```

### Model Kartı

Gösterilecek bilgiler:

- Model adı
- Model ailesi
- GGUF path
- Quantization
- Dosya boyutu
- Vision/mmproj durumu
- Capability sonuçları
- Lifecycle state
- Son doğrulama zamanı
- Benchmark durumu
- Kullanıldığı görevler

### İşlemler

- Model ekle
- Model dosyası seç
- mmproj seç
- Runtime seç
- Probe çalıştır
- Benchmark çalıştır
- Experimental yap
- Production'a yükselt
- Devre dışı bırak
- Task binding değiştir

## Güvenlik

Gerçek öğrenci verisi `Imported`, `Probing`, `Compatible` veya `Unsupported` model durumlarına gönderilmemeli.

Experimental model gerçek öğrenci verisinde ancak açıkça seçilmiş güvenli deney modu varsa kullanılmalı; varsayılan production akışına otomatik girmemeli.

## Kabul Kriterleri

- Yeni model UI üzerinden eklenebilmeli.
- Kullanıcı kod veya config JSON düzenlemek zorunda kalmamalı.
- Production modeli tek tıkla yanlışlıkla değiştirilememeli.

---

# Sprint 6 — Golden benchmark karşılaştırma ve promotion gate'lerini bağla

## Amaç

Yeni model seçimini sezgisel değil, ölçülebilir kalite verisine bağlamak.

## Kullanılacak Mevcut Metrikler

- CER
- WER
- Critical-token missing
- Printed-question leakage
- Structured-field exact match
- Schema success rate
- Retry count
- Model call count
- p50 latency
- p95 latency
- Peak memory (ölçülebiliyorsa)

Scoring için ek metrikler:

- Golden level agreement
- Criterion ID validity
- Exact-evidence validity
- Review rate
- Invalid-schema rate
- Direct-score leakage/rejection count

## Benchmark Sonucu

Örnek karşılaştırma:

```text
Metric                 Gemma 4 12B    Model X
OCR CER p50             0.00           0.01
OCR WER p50             0.00           0.02
Critical token miss     0              0
Schema success          83%            100%
Scoring agreement       94%            96%
Latency p50             51 s           27 s
Peak memory             9.8 GB         7.1 GB
```

## Promotion Gate

Model `Production` statüsüne geçmeden önce task bazlı threshold uygulanmalı.

Örneğin OCR için:

```text
critical token missing == 0
printed leakage == 0
schema failure <= threshold
CER/WER baseline regression <= threshold
```

Scoring için:

```text
unknown criterion ID == 0
invalid canonical level ID == 0
positive score without exactEvidence == 0
schema failure <= threshold
golden agreement >= threshold
```

Threshold'lar kod içine dağılmamalı; versioned benchmark policy olmalı.

## Kabul Kriterleri

- Model benchmark sonucu registry'ye kaydedilmeli.
- Model fingerprint değişirse eski benchmark geçersiz sayılmalı.
- Task bazlı production promotion gate uygulanmalı.
- Kullanıcı Gemma ile yeni modeli aynı ekranda karşılaştırabilmeli.

---

# Sprint 7 — Gemma özel sabitleri kaldır, compatibility migration yap ve eski profilleri emekli et

## Amaç

Yeni mimari production'da kanıtlandıktan sonra eski model-spesifik yapıları kaldırmak.

## Kaldırılacak / Dönüştürülecek Yapılar

Aşağıdaki gibi model ve görevi birbirine bağlayan sabit kimlikler migration sonrası compatibility dışında kullanılmamalı:

```text
gemma4-ocr-q8
speaking_transcript_cleanup_12b
speaking_rubric_evaluation_12b
```

Yerine:

```text
ModelDefinition:
  gemma4-12b

TaskProfile:
  student_answer_ocr
  speaking_transcript_cleanup
  speaking_rubric_evaluation
```

## Config Migration

Mevcut `model_profiles.json` için:

```text
legacy config
   ↓
backup
   ↓
parse
   ↓
ModelDefinition oluştur
   ↓
RuntimeDefinition oluştur
   ↓
Task binding oluştur
   ↓
validate
   ↓
new config atomic write
```

Migration başarısız olursa eski dosya korunmalı ve uygulama fail-safe recovery ekranı göstermeli.

## Deprecation

- `ModelRuntimePreset` önce deprecated yapılmalı.
- Bir release boyunca read compatibility korunmalı.
- Yeni config yazımı yalnız yeni schema ile yapılmalı.
- Sonraki cleanup fazında legacy serializer kaldırılmalı.

## Kabul Kriterleri

- Gemma yalnızca registry'deki bir model olarak kalmalı.
- Domain servislerinde `gemma` string/sabit bağımlılığı kalmamalı.
- Legacy kullanıcı config'i veri kaybı olmadan migrate olmalı.

---

# Sprint 8 — Final regression, rollback doğrulaması ve production gate

## Amaç

Yeni model platformunun mevcut Rubrika güvenlik/kalite davranışını bozmadığını kanıtlamak.

## Regression Paketleri

### Model Gateway

- Health probe
- Completion probe
- Vision probe
- Structured JSON
- JSON schema
- Reasoning-only response
- Empty response
- Invalid JSON
- Invalid schema
- Timeout
- Oversized request/response
- Redirect rejection
- Strict-local enforcement

### Runtime

- Managed process identity
- Port ownership
- Start/stop
- Lease lifecycle
- Drain behavior
- Shared runtime reuse
- Model fingerprint
- Runtime fingerprint
- Crash recovery

### OCR

- Golden OCR corpus
- CER/WER
- Critical token
- leakage
- structured answer
- review policy

### Scoring

- Canonical criterion ID enforcement
- Canonical level mapping
- Exact evidence
- Direct score rejection
- Invalid schema fail-closed
- Deterministic answer types → Rust-only scoring

### Speaking

- Text-only cleanup
- No mmproj invariant
- Shared runtime
- Speaking rubric evidence
- Deterministic ceilings

## Rollback Testi

Aşağıdaki senaryo özellikle doğrulanmalı:

```text
Gemma Production
    ↓
Model X Experimental
    ↓
Model X task binding
    ↓
Model X failure
    ↓
User explicitly switches binding back
    ↓
Gemma Production
```

Rollback'ta:

- eski model dosyası yeniden seçilmek zorunda kalmamalı,
- prompt/schema/policy değişmemeli,
- provenance doğru kalmalı,
- cache yanlış model sonucunu reuse etmemeli.

## Production Gate

Migration tamamlandı sayılmadan önce:

```text
[PASS] Legacy config migration
[PASS] Gemma baseline regression
[PASS] Model registry
[PASS] Capability negotiation
[PASS] Task router
[PASS] Runtime adapter
[PASS] Strict-local privacy
[PASS] Golden OCR benchmark
[PASS] Golden scoring benchmark
[PASS] Speaking regression
[PASS] Rollback
[PASS] CI
```

---

# Veri ve Config Şeması Önerisi

Önerilen yeni config yapısı kavramsal olarak:

```text
model_platform.json

models[]
runtimes[]
task_profiles[]
bindings[]
benchmark_results[]
active_runtime_instances[]   # yalnız runtime state için; kalıcı olmaması tercih edilir
```

Örnek ilişki:

```text
ModelDefinition
  id = gemma4-12b

RuntimeDefinition
  id = llama-local-default

TaskProfile
  id = student_answer_ocr

Binding
  task = student_answer_ocr
  model = gemma4-12b
  runtime = llama-local-default
```

---

# Cache ve Provenance Kuralları

Yeni mimaride cache key en az şunları içermeli:

```text
task_profile_id
prompt_version
schema_version
policy_version
model_fingerprint
runtime_fingerprint
sampling_parameters
input_hash
```

Model binding değiştiğinde eski model sonucu yeni model sonucu gibi reuse edilmemeli.

`ModelProvenance` geriye dönük izlenebilirliği korumalı ve mümkünse aşağıdaki kimlikleri açıkça taşımalı:

```text
model_definition_id
runtime_definition_id
task_profile_id
binding_id
model_fingerprint
runtime_fingerprint
benchmark_policy_version
```

---

# Hata Taksonomisine Eklenecek Önerilen Kodlar

```text
MODEL_REGISTRY_ENTRY_NOT_FOUND
MODEL_CAPABILITY_MISMATCH
MODEL_CAPABILITY_UNVERIFIED
MODEL_BINDING_NOT_FOUND
MODEL_BINDING_UNAVAILABLE
MODEL_NOT_PRODUCTION_APPROVED
MODEL_PROBE_FAILED
MODEL_BENCHMARK_REQUIRED
MODEL_BENCHMARK_FAILED
MODEL_CONFIG_MIGRATION_FAILED
MODEL_RUNTIME_ADAPTER_UNSUPPORTED
```

Mevcut typed error yaklaşımı korunmalı; kullanıcıya teknik kod yerine recovery action gösterilmeli.

---

# Non-Goals

Bu migration kapsamında ilk aşamada yapılmayacaklar:

- llama.cpp production runtime'ını kaldırmak
- Cloud LLM'i varsayılan yapmak
- Scoring karar yetkisini modele vermek
- Modelin verdiği sayısal skoru doğrudan persist etmek
- Prompt/schema validation'ı modele bırakmak
- Tüm modelleri tek görev için otomatik ensemble yapmak
- Otomatik silent fallback
- Gerçek öğrenci verisini benchmark corpus'u yapmak

---

# Başarı Tanımı

Migration başarılı sayıldığında aşağıdaki kullanım mümkün olmalı:

```text
Yeni model indir
    ↓
Rubrika > Yerel Modeller > Model Ekle
    ↓
GGUF seç
    ↓
Gerekirse mmproj seç
    ↓
Capability probe
    ↓
Golden benchmark
    ↓
Experimental
    ↓
Task için seç
    ↓
Karşılaştır
    ↓
Promotion gate PASS
    ↓
Production
```

Yeni model eklemek için artık Rust koduna yeni bir Gemma/Qwen/model-ailesi özel `enum`, profile ID veya `build_*_args()` dalı eklemek gerekmemelidir.

Mevcut Gemma 4 12B modeli ise bu geçiş sonunda özel durum olmaktan çıkar ve aynı registry/runtime/task sözleşmelerini kullanan doğrulanmış production model olarak çalışmaya devam eder.
