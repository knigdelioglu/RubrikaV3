# RubrikaV3 — Local Model Platform Migration Plan

Amaç: mevcut Gemma 4 12B + llama.cpp production davranışını bozmadan RubrikaV3'ü model-bağımsız, görev bazlı, capability-probe ve benchmark-gated bir local inference platformuna dönüştürmek.

## Takip Listesi

- [x] Sprint 0 — Mevcut davranışı kilitle ve migration sınırını tanımla
- [ ] Sprint 1 — `ModelDefinition`, `RuntimeDefinition` ve `TaskProfile` ayrımını getir
- [ ] Sprint 2 — Runtime argümanlarını koddan çıkar ve `LlamaCppRuntimeAdapter` katmanını oluştur
- [ ] Sprint 3 — Model Registry + Capability Negotiation altyapısını kur
- [ ] Sprint 4 — Task → Model Binding ve model router katmanını devreye al
- [ ] Sprint 5 — Experimental / Production model yaşam döngüsü ve Model Laboratuvarı UI'sını ekle
- [ ] Sprint 6 — Golden benchmark karşılaştırma ve promotion gate'lerini bağla
- [ ] Sprint 7 — Gemma özel sabitleri kaldır, compatibility migration yap ve eski profilleri emekli et
- [ ] Sprint 8 — Final regression, rollback doğrulaması ve production gate

> Test kuralı: Sprint 0–7 kod ve dokümantasyon işi tamamlanmadan test/regression çalıştırılmaz. Testler yalnız Sprint 8'de toplu çalıştırılır.

---

## Hedef Mimari

```text
Rubrika Domain
    ↓
Task Contract / TaskProfile
    ↓
Model Router
    ↓
Task → Model Binding
    ↓
Model Registry
    ↓
Capability Negotiation
    ↓
InferenceRuntime
    ↓
LlamaCppRuntimeAdapter
    ↓
llama-server
    ↓
Local Model
```

Gelecekte aynı runtime sözleşmesinin altına `MlxRuntimeAdapter` veya explicit-consent external adapter eklenebilir. Production/default runtime llama.cpp olarak kalır.

Temel invariant: domain servisleri model ailesini, GGUF dosyasını, mmproj yolunu veya llama.cpp argümanlarını bilmez.

---

# Sprint 0 — Baseline ve migration sınırı

**Durum: tamamlandı.** Referans: `docs/LOCAL_MODEL_PLATFORM_BASELINE.md`.

Kilitlenen alanlar:

- question/rubric extraction
- student OCR + issue correction
- semantic scoring
- speaking cleanup/evaluation
- analysis/general model calls
- strict-local privacy
- canonical rubric → Rust score mapping
- fail-closed JSON/schema davranışı
- prompt/schema/policy/model/runtime provenance
- process ownership, lease ve shared-runtime davranışı
- golden OCR/scoring corpus ve mevcut regression dokümanları

---

# Sprint 1 — Domain ayrımı

Yeni yapılar:

### `ModelDefinition`

- id, family, display name
- model path / optional mmproj
- format / quantization
- declared capabilities
- context limit
- model fingerprint
- lifecycle state
- metadata

### `RuntimeDefinition`

- id, engine, server path
- host/port
- context/gpu/flash-attention
- parallel/batch/ubatch
- KV cache K/V
- reasoning mode
- güvenli allowlisted extra args

### `TaskProfile`

- id/use case
- required capabilities
- prompt/schema/policy version
- sampling parameters
- timeout/max tokens
- response format

Legacy `ModelProfile` hemen silinmez; compatibility migration yeni yapılara lossless dönüşüm sağlar.

**Kabul:** model kimliği, runtime ve task contract birbirinden bağımsızdır; legacy kullanıcı ayarı kaybolmaz.

---

# Sprint 2 — Runtime adapter

`InferenceRuntime` sözleşmesi:

```text
start
stop
health
probe
capabilities
complete
fingerprint
preview_args
```

İlk adapter: `LlamaCppRuntimeAdapter`.

Koddan çıkarılacak kararlar:

- gpu layers
- context size
- batch/ubatch
- KV cache
- parallel
- reasoning
- image token argümanları
- mmproj gereksinimi

Güvenlik:

- `extra_args` allowlist/denylist
- strict-local host override engeli
- deterministic runtime fingerprint
- mevcut process identity/port ownership/lease korunur
- speaking shared runtime korunur

---

# Sprint 3 — Registry ve capability negotiation

Registry saklar:

```text
ModelDefinition
RuntimeDefinition
CapabilityManifest
model/runtime fingerprint
last verified at
benchmark status
lifecycle state
```

Probe zinciri:

```text
file validation
→ runtime startup
→ health
→ text completion
→ JSON object
→ JSON schema
→ vision (gerekliyse)
→ thinking control
→ capability manifest
```

Capability sonucu `pass | partial | fail | unverified` olabilir. Production task için `partial/unverified` otomatik kabul edilmez. Model fingerprint değişirse eski probe geçersiz olur.

---

# Sprint 4 — Task binding ve router

Örnek:

```text
student_answer_ocr   → gemma4-12b
rubric_extraction    → gemma4-12b
semantic_scoring     → experimental-model-x
speaking_cleanup     → gemma4-12b
```

Router sırası:

1. TaskProfile
2. binding
3. registry entry
4. required capabilities
5. lifecycle/production izinleri
6. runtime uyumu
7. privacy policy
8. runtime lease

Sessiz fallback yasaktır. Binding unavailable ise typed error + recovery action döner; model otomatik Gemma'ya değiştirilmez.

Provenance task profile, binding, model ve runtime kimliğini taşır.

---

# Sprint 5 — Model yaşam döngüsü ve Model Laboratuvarı

Lifecycle:

```text
Imported → Probing → Compatible → Experimental → BenchmarkVerified → Production
```

Alternatif durumlar:

```text
Unsupported | ProbeFailed | BenchmarkFailed | Disabled
```

Ayarlar > Yerel Modeller yüzeyi:

- Modeller
- Runtime
- Görev Atamaları
- Benchmark

Model kartında model/family/path/quantization/vision/mmproj/capabilities/lifecycle/verification/benchmark/task bindings gösterilir.

İşlemler: model ekle, GGUF/mmproj seç, runtime seç, probe, benchmark, experimental, production promotion, disable, task binding değiştir.

Imported/Probing/Compatible/Unsupported modeller gerçek öğrenci production verisi alamaz. Experimental kullanım explicit deney seçimi gerektirir.

---

# Sprint 6 — Benchmark ve promotion gate

Mevcut golden altyapısı yeniden kullanılır.

OCR metrikleri:

- CER/WER
- critical-token missing
- printed-question leakage
- structured-field exact
- schema success
- retry/model-call count
- p50/p95 latency
- peak memory mevcutsa

Scoring metrikleri:

- canonical level agreement
- criterion ID validity
- exact-evidence validity
- review rate
- invalid-schema rate
- direct-score leakage/rejection

Versioned `BenchmarkPolicy` task bazlı threshold taşır. Model fingerprint değişirse benchmark invalid olur.

Promotion fail-closed'dur; gate PASS olmadan Production yapılamaz.

---

# Sprint 7 — Legacy/Gemma özel yapıları emekli et

Eski model+task birleşik kimlikleri yalnız migration alias'ı olur:

```text
gemma4-ocr-q8
speaking_transcript_cleanup_12b
speaking_rubric_evaluation_12b
```

Canonical model:

```text
gemma4-12b
```

Görevler bağımsız TaskProfile'lara taşınır.

`model_profiles.json` migration:

```text
backup → parse → model/runtime/task/binding üret → validate → atomic new config
```

Başarısız migration eski dosyayı korur. Yeni yazımlar yalnız yeni config schema'ya gider; legacy read compatibility bir geçiş dönemi korunur.

Domain servislerinde `gemma` string veya profile-ID branching kalmamalıdır.

---

# Sprint 8 — Final regression ve production gate

Kod işi bittikten sonra ilk kez toplu test/regression çalıştırılır.

Doğrulanacak paketler:

### Gateway
health/completion/vision/JSON/schema/reasoning-only/empty/invalid/timeout/size/redirect/strict-local.

### Runtime
process identity, port ownership, start-stop, lease/drain, shared runtime, fingerprints, crash recovery.

### OCR
Golden corpus, CER/WER, critical-token, leakage, structured answer, review policy.

### Scoring
canonical criterion/level, exact evidence, direct-score rejection, invalid-schema fail-closed, deterministic Rust-only types.

### Speaking
text-only, no-mmproj, shared runtime, evidence, deterministic ceilings.

### Rollback

```text
Gemma Production
→ Model X Experimental
→ task binding Model X
→ failure
→ explicit binding rollback
→ Gemma Production
```

Rollback model dosyasını yeniden seçtirmemeli; provenance/cache model kimliğini doğru ayırmalıdır.

Production gate:

- [ ] Legacy config migration
- [ ] Gemma baseline regression
- [ ] Model registry
- [ ] Capability negotiation
- [ ] Task router
- [ ] Runtime adapter
- [ ] Strict-local privacy
- [ ] Golden OCR benchmark
- [ ] Golden scoring benchmark
- [ ] Speaking regression
- [ ] Rollback
- [ ] CI

---

## Config ve provenance hedefi

Yeni config kavramsal olarak:

```text
model_platform.json
models[]
runtimes[]
task_profiles[]
bindings[]
capability_manifests[]
benchmark_results[]
```

Cache key en az:

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

Provenance en az:

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

## Typed error hedefleri

- `MODEL_REGISTRY_ENTRY_NOT_FOUND`
- `MODEL_CAPABILITY_MISMATCH`
- `MODEL_CAPABILITY_UNVERIFIED`
- `MODEL_BINDING_NOT_FOUND`
- `MODEL_BINDING_UNAVAILABLE`
- `MODEL_NOT_PRODUCTION_APPROVED`
- `MODEL_PROBE_FAILED`
- `MODEL_BENCHMARK_REQUIRED`
- `MODEL_BENCHMARK_FAILED`
- `MODEL_CONFIG_MIGRATION_FAILED`
- `MODEL_RUNTIME_ADAPTER_UNSUPPORTED`

Teknik kodlar developer diagnostics'te kalır; teacher-facing yüzey recovery action gösterir.

---

## Non-goals

- llama.cpp production runtime'ını kaldırmak
- Cloud LLM'i default yapmak
- scoring kararını modele devretmek
- modelin sayısal skorunu doğrudan persist etmek
- validation'ı modele bırakmak
- automatic ensemble
- silent fallback
- gerçek öğrenci verisini benchmark corpus'u yapmak

## Başarı Tanımı

Yeni model ekleme akışı:

```text
GGUF seç
→ gerekirse mmproj seç
→ capability probe
→ golden benchmark
→ Experimental
→ task binding
→ karşılaştırma
→ promotion gate PASS
→ Production
```

Yeni model için Rust'a model-ailesi özel enum, hard-coded profile ID veya yeni `build_*_args()` dalı eklemek gerekmemelidir.
