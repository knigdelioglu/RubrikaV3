# RubrikaV3 — Local Model Platform Migration Baseline

Bu belge, `LOCAL_MODEL_PLATFORM_MIGRATION_PLAN.md` uygulanırken korunması gereken mevcut production davranışını ve regresyon referanslarını sabitler. Migration'ın amacı model bağımlılıklarını gevşetmek; OCR, scoring, speaking ve privacy davranışlarını yeniden tanımlamak değildir.

## Production baseline

Migration başlangıcındaki doğrulanmış production modeli Gemma 4 12B'dir. Mevcut model gateway, managed/external runtime, strict-local sınırı, typed prompt/schema/policy contract, model/runtime fingerprint ve golden corpus yeni platformun karşılaştırma baseline'ı kabul edilir.

## Model kullanım envanteri

Aşağıdaki model çağrıları migration kapsamındaki task yüzeyleridir:

- `question_text_extraction`
- `rubric_extraction`
- `student_answer_ocr`
- `student_answer_ocr_issue_correction`
- `scoring`
- `speaking_transcript_cleanup`
- `speaking_evaluation`
- `general_text` / analysis çağrıları

Her task yeni platformda bağımsız `TaskProfile` ve `TaskModelBinding` ile temsil edilecektir. Domain servisleri model ailesi veya GGUF adına göre dallanmayacaktır.

## Değiştirilemez güvenlik invariant'ları

1. React/UI model endpoint'ine doğrudan çağrı yapmaz. Tüm inference Rubrika backend gateway/runtime sınırından geçer.
2. Öğrenci verisi taşıyan çağrılar `StrictLocal` politikasını korur; loopback dışı endpoint sessizce kabul edilmez.
3. Semantic scoring sırasında modelin sayısal skor alanı karar kaynağı değildir. Model canonical criterion/level ve evidence önerir; puan Rust tarafından canonical frozen rubric üzerinden hesaplanır.
4. Bilinmeyen, eksik veya duplicate criterion ID; geçersiz canonical level; eksik exact evidence; invalid JSON/schema normal skor gibi uygulanmaz. Akış fail-closed ve teacher-review davranışını korur.
5. Deterministik soru tipleri mümkün olan yerlerde model çağrısı yapmadan Rust ile puanlanmaya devam eder.
6. OCR confidence/review gate versioned backend policy tarafından uygulanır; modelin `needsReview` iddiası tek başına kaynak değildir.
7. Prompt policy ile user data sınırı korunur. Öğrenci cevabı, soru metni, rubrik, transcript veya metrikler system policy metnine karıştırılmaz.
8. Her model çağrısı prompt version, schema version, policy version, model fingerprint, runtime fingerprint, sampling parameters ve response format provenance'ını taşır.
9. Managed process ownership yalnız Rubrika'nın başlattığı ve kimliği doğrulanmış process için stop/replace yetkisi verir. PID tek başına process kimliği değildir.
10. Model/server hataları application crash'e veya sessiz fallback'e dönüşmez; typed error + recovery action üretir.
11. Speaking cleanup ve speaking evaluation mevcut shared-runtime davranışını korur; sırf task kimlikleri farklı diye ikinci model process'i açılmaz.
12. Speaking text-only task'ları `mmproj` yüklemez. Vision task'larının projector/capability gereksinimi task contract üzerinden doğrulanır.
13. Cache/provenance model ve runtime fingerprint değişimini ayırt eder. Bir modelin sonucu başka model sonucu gibi reuse edilmez.
14. Harici model kullanımı yalnız explicit consent ile açılır; legacy veya public endpoint student-data çağrısında otomatik güvenli kabul edilmez.

## Regression referansları

Migration kapanışında en az aşağıdaki mevcut referanslar tekrar doğrulanmalıdır:

- `docs/MODEL_GATEWAY.md`
- `docs/MODEL_RUNTIME_OWNERSHIP.md`
- `docs/MODEL_GATEWAY_LIMITS_AND_CONFIGURATION.md`
- `docs/GOLDEN_OCR_SCORING_BENCHMARK.md`
- `testdata/golden/tymm_tde_001/`
- model gateway structured-output / privacy / timeout / redirect regressions
- model process identity / lease / drain regressions
- golden OCR metrics and corpus regressions
- scoring canonical ID/level/exact-evidence/direct-score rejection regressions
- speaking text-only/no-mmproj/shared-runtime regressions

## Migration compatibility sınırı

Legacy `ModelProfile`, `ModelRuntimePreset` ve eski profile ID'leri migration'ın ilk safhalarında yalnız read/compatibility kaynağı olarak kalabilir. Yeni platformun production kararları `ModelDefinition`, `RuntimeDefinition`, `TaskProfile`, registry capability manifest ve task binding üzerinden verilmelidir.

Eski config migration tamamlanmadan legacy serializer veya kullanıcı config'i silinmez. Config conversion başarısızsa eski dosya korunur ve fail-safe recovery uygulanır.

## Baseline başarı şartı

Yeni platform tamamlandığında Gemma 4 12B özel-case olmadan registry'deki doğrulanmış production model olarak aynı güvenlik sınırlarıyla çalışmalı; yeni bir model eklemek için Rust'a model-ailesi özel enum, profile ID veya `build_*_args()` dalı eklemek gerekmemelidir.
