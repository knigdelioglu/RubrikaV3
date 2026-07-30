# Speaking Exam Engine

Canonical akış:

```text
Whisper ham segmentleri
→ Gemma 4 12B segment-preserving cleanup
→ deterministic cleanup gate
→ transcript_for_scoring
→ aynı merkezi Gemma 4 12B runtime ile v4 level/evidence değerlendirmesi
→ evidence ID validation
→ frozen v2 deterministic ceiling/reconciliation
→ backend integer puan
→ öğretmen incelemesi ve onayı
```

Cleanup `raw_transcript` alanını değiştirmez. Aynı `segment_id` ve sıra korunur; segment eksikliği, kesilmiş finish reason, semantic change/review işareti veya `%85–120` dışı kelime kapsamı accepted sonucu engeller.

Evaluation’a düz metin değil canonical segment JSON’u, görev, konuşma türü, frozen alt göstergeler/descriptors, cleanup güveni ve deterministic metrikler verilir. Öğrenci kimliği, geçmiş notları ve teacher-only gözlemler verilmez. Model olumlu/karşı kanıt ve eksik koşul üretir; serbest puan alanları kullanılmaz.

Pipeline ancak cleanup accepted/öğretmen onaylı, bütün zorunlu alt göstergeler parse edilmiş, ID ve kanıtlar doğrulanmış, ceiling tamamlanmış ve integer criterion puanları oluşmuşsa tamamlanır. Eksik reconciliation final toplam üretmez. Ceiling uygulanmış all-strong aday öğretmen incelemesine görünür nedenle gider.

Akıcılık metriği kayıt süresi, aktif konuşma süresi, kelime/dakika, konuşma oranı, uzun duraklama, dolgu, tekrar sayaçları ve clipping metriklerini (clipping_ratio, clipping_event_count) taşır. Minimum sürenin anlamlı bölümü karşılanmıyorsa measurement confidence düşüktür; bu sıfır puan değildir. Clipping öğrenciye otomatik puan cezası vermez, yalnız kayıt kalitesi uyarısı üretir.

Sınav kurulurken öğretmen sınav adı, konuşma türü (hazırlıklı/hazırlıksız), ortak görev metni, dk/sn cinsinden süre sınırlarını (`min_duration_seconds`, `target_duration_seconds`, `max_duration_seconds`) ve sınava atanacak sınıfları (`assigned_class_ids: Vec<String>`) belirler. Projede kayıtlı legacy `class_id` alanı yükleme anında otomatik olarak `assigned_class_ids = [class_id]` şeklinde dondurulur ve normalize edilir.

Öğretmen gözlemi ölçütleri (`Ses/Diksiyon`, `Hazırlık/Materyal`, `Beden Dili`, `Öz Değerlendirme`) 3 yıldızlı (`★ ★ ☆`) erişilebilir `TeacherStarRating` kontrolü ile değerlendirilir. Yıldız seçilmeyen kartlar `null` sayılır ve `Geçici toplam` gösterilir.

Production repeatability canonical `evaluationInputHash` ve kayıtlı sonuçla sağlanır. Prompt/policy/model/runtime/transcript değişimi cache’i stale yapar. Legacy decimal kayıtlar okunabilir; yeni model ve öğretmen puanları tam sayıdır.
