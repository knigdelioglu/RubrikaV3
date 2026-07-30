# Speaking Scoring Policy

Canonical sürüm: `speaking_scoring_policy_v2`  
Prompt sürümü: `speaking_rubric_evidence_tr_v4`

Model puan veya toplam üretmez. Model her frozen alt gösterge için düzey, olumlu kanıt segmentleri, karşı kanıt segmentleri, eksik zorunlu koşullar ve gerekçe üretir. Backend gerçek segment ID’lerini doğrular, deterministic ceiling uygular ve tam sayı criterion puanını hesaplar.

`strong`, “olumlu bir özellik görüldü” anlamına gelmez. Frozen policy içindeki bütün `mandatoryRequirements` açık ve doğrulanabilir kanıtla karşılanmalı, belirgin eksik veya karşı kanıt bulunmamalıdır. İki komşu düzey arasında üst düzey yalnız bu koşulla seçilir. Yardımcı ayrıntı eksikliği tek başına düşüş değildir; ASR belirsizliği, konuşma dili ve yazı diline göre küçük kusurlar öğrenci aleyhine kullanılmaz. Kanıt yoksa cömertlik adına başarı varsayılmaz ve aynı kusur ilgisiz iki göstergede çift cezalandırılmaz.

AI kümeleri:

- `content_main_idea`: `task_relevance`, `main_idea`, `supporting_ideas`, `examples_reasons`, `content_depth`; her biri 0–4.
- `speech_structure`: `opening`, `idea_order`, `transitions`, `coherence`, `conclusion`; her biri 0–3.
- `turkish_language`: `sentence_clarity`, `vocabulary_range`, `contextual_word_use`, `connectors`, `repetition_control`; her biri 0–3.

Deterministic tavanlar model seçimini silmez. `selectedLevelId` orijinal seçimi, `appliedLevelId` backend sonucunu; `ceilingReasonCode` ve `ceilingExplanation` görünen nedeni taşır.

- Geliştirilmemiş fikir listesi: `supporting_ideas <= adequate`.
- Somut örnek/açık gerekçe yok: `examples_reasons <= adequate`.
- İki ayrı geliştirme yönü yok: `content_depth <= adequate`.
- Planlı giriş yok: `opening <= adequate`.
- İki farklı işlevsel geçiş yok: `transitions <= adequate`.
- Açık toparlayan kapanış yok: `conclusion <= limited`.
- Kısa veya tekrarlı örnek: `vocabulary_range <= adequate`.
- Bağlaç çeşitliliği yetersiz: `connectors <= adequate`.
- Aynı temel ifade kısa metinde en az üç kez tekrarlı: `repetition_control <= limited`.

Pozitif düzey geçerli canonical evidence segmenti olmadan uygulanmaz. Karşı kanıt ID’leri de doğrulanır. Aynı kanıt kümesinin bir criterion içindeki bütün ilgisiz alt göstergelere körlemesine kopyalanması sonucu bloke eder. Eksik zorunlu alt gösterge final criterion ve toplam oluşmasını engeller.

Akıcılık ve süre deterministik backend ölçümleridir. Minimum sürenin `%60`’ından kısa kayıt `low` measurement confidence üretir; otomatik puanı sıfırlamaz fakat öğretmen onayı gerektirir.

## 3 Yıldızlı Öğretmen Yıldız Değerlendirmesi (`TeacherStarRating`)

Gözlemsel öğretmen ölçütlerinde Gemma kullanılmaz. Öğretmen erişilebilir 3 yıldızlı (`★ ★ ☆`) kontrol üzerinden gözlemini işaretler:

- 1 Yıldız: Geliştirilebilir (`developing` / `star_1`)
- 2 Yıldız: İyi (`good` / `star_2`)
- 3 Yıldız: Çok iyi (`very_good` / `star_3`)

Puan dönüşümü (Backend Frozen Policy):
- 5 puanlık ölçüt (Öz değerlendirme ve gelişim hedefi): 1 Yıldız → 2/5 | 2 Yıldız → 4/5 | 3 Yıldız → 5/5
- 10 puanlık ölçüt (Ses/Diksiyon, Beden Dili): 1 Yıldız → 4/10 | 2 Yıldız → 7/10 | 3 Yıldız → 10/10
- 15 puanlık ölçüt (Hazırlık/Materyal): 1 Yıldız → 6/15 | 2 Yıldız → 11/15 | 3 Yıldız → 15/15

Açık Değerlendirilmedi (`null`) ve Performans gösterilmedi (`0` puan) ayrımı korunur. Yıldız seçilmemesi `null` sayılır ve `Geçici toplam` üretir.

## Deterministik Süre ve Clipping Politikası

Süreyi Yönetme Puanı (5 Puan):
- `min <= actual <= max` → 5/5
- Alt veya üst sınırdan sapma `%10` veya daha az → 4/5
- Sapma `%10`'dan fazla, `%25` veya daha az → 3/5
- Sapma `%25`'ten fazla, `%40` veya daha az → 2/5
- Sapma `%40`'tan fazla → 1/5

Clipping Politikası:
- Mikrofon ses taşması (clipping) teknik bir kayıt kalitesi uyarısıdır (`recording_quality_warning`).
- Clipping otomatik olarak öğrencinin ses, diksiyon veya akıcılık puanını düşürmez.

Repeatability hash; scoring transcriptini, segmentleri, metrikleri, cleanup işaretlerini, frozen rubric/policy’yi, cleanup ve evaluation prompt sürümlerini, model hash’ini ve runtime fingerprint’i kapsar. Aynı hash + eksiksiz canonical sonuç production’da model çağrısını atlar.
