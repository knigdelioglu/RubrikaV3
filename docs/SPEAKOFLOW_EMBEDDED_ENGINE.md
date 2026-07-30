# Embedded Speakoflow engine

RubrikaV3 konuşma sınavı artık Speakoflow masaüstü uygulamasını, launcher’ı, history veritabanını, global shortcut’ı veya ayrı bir process’i kullanmaz. Motor aynı Tauri process’i içinde Rust workspace crate’leri olarak çalışır.

## Crate sınırları

- `speakoflow-types`: Tauri/UI bağımsız state, segment, metric ve typed error tipleri.
- `speakoflow-audio`: CPAL input device, bounded callback kuyruğu, mono downmix, 16 kHz resampling ve WAV yazımı.
- `speakoflow-vad`: deterministic konuşma/sessizlik ve filler/repetition ölçümleri.
- `speakoflow-stt`: tek lazy/sıcak Whisper instance’ı. Güncel GGUF Whisper modelleri transcribe.cpp ile, eski `.bin` modelleri whisper.cpp ile yüklenir. Öncelik `RUBRIKA_WHISPER_MODEL_PATH` ve `RUBRIKA_V3_WHISPER_MODEL_PATH` değerlerindedir; bunlar yoksa mevcut SpeakoFlow kullanıcı model klasöründeki indirilen Whisper modeli otomatik keşfedilir.
- `speakoflow-engine`: tek aktif session ve `Starting → Recording ↔ Paused → Stopping → Transcribing → Completed` lifecycle’ı; cancel/failure yolları da capture worker’ı kapatır.

## Rubrika adapter akışı

`start_speaking_exam` yalnızca konuşma sınavı tanımını kaydeder. `start_speaking_exam_attempt`/`toggle_speaking_capture(start)` engine session başlatır ve session kimliğini attempt’e yazar. Pause/resume/cancel komutları aynı capture worker’a gider. Stop komutu capture worker’dan WAV örneklerini alır; transkripsiyon bloklayıcı worker’da tamamlanır. `sync_speaking_attempt` yalnızca Rubrika ProjectStore’daki canonical attempt’i ve değerlendirme job’unu okur.

Attempt metin ve tanılama artifact’leri proje içinde tutulur. WAV yalnızca öğretmen puanı
kesinleşene kadar geçici olarak bulunur:

```text
artifacts/speaking-exams/<attempt-id>/
  audio-original.wav  # geçici; nihai puan kaydından hemen sonra kalıcı silinir
  transcript-raw.json
  transcript-cleanup.json
  segments.json
  metrics.json
  diagnostics.json
```

Konuşma değerlendirme zinciri backend içinde çalışır; ayrı bir SpeakoFlow uygulaması veya launcher açılmaz:

`Ses kaydı → Whisper ham transkript → raw_transcript kalıcı kayıt → Gemma 4 12B segment cleanup → cleanup doğrulaması → transcript_for_scoring → aynı Gemma 4 12B runtime ile rubrik değerlendirmesi → deterministik ölçüt reconciliation → öğretmen incelemesi → nihai onay`

Raw transcript hiçbir cleanup sonucu ile overwrite edilmez. Cleanup çıktısı boş, bozuk veya doğrulanamazsa `transcript_for_scoring` üretilmez ve Gemma 4 12B scoring başlamaz; attempt öğretmen incelemesine bırakılır. Model veya Whisper hatası sıfır puan üretmez; hata tanısı ProjectStore alanında korunur. Öğretmen nihai puanı kaydettikten sonra önce ProjectStore atomik kaydı tamamlanır, sonra WAV kalıcı silinir ve `audioPath` temizlenir. Uygulama iki işlem arasında kapanırsa `get_speaking_exam` onaylı attempt’lerdeki artık WAV’ı açılışta temizler.

Gemma 4 12B cleanup ve rubrik değerlendirmesi aynı text-only llama.cpp runtime’ını kullanır: projector yüklenmez, `turbo3` KV cache ile çalışır, thinking kapalı istek alır. Cleanup isteği, segment-preserving JSON çıktısının kesilmemesi için transkript uzunluğu ve segment sayısından türetilen `256..4096` token bütçesi ve yerel 12B modelinin ilk yanıt gecikmesini karşılamak için 300 saniyelik üst timeout kullanır. Whisper transkripsiyonu tamamlandıktan sonra ortak runtime hazırlanır; cleanup ile rubrik arasında süreç kapatılmaz ve aynı model iki kez yüklenmez. Rubrik işi tamamlandıktan sonra runtime kapatılır.

Gemma 4 12B yalnızca transkriptten gözlenebilen içerik, yapı, tutarlılık ve Türkçe
kullanımı ölçütlerine öneri verir. Tek bir beklenen cevap varsaymaz. Beden dili, mekân,
göz teması, ses, hazırlık ve iletişim ölçütleri `teacher_only` kalır; öğretmen bunları
`Çok iyi / İyi / Orta / Geliştirilebilir` düzeyleriyle işaretler ve backend düzeyi
deterministik puana çevirir.

## Runtime ayarı

Whisper modeli için öncelikli ayar `RUBRIKA_WHISPER_MODEL_PATH`’tir. Mevcut SpeakoFlow kurulumu bulunan makinelerde `whisper-medium-Q8_0.gguf` otomatik bulunur; model yoksa kayıt düğmesi devre dışı kalır ve sahte transcript üretilmez.

## Command yüzeyi

- `start_speaking_exam`
- `list_speaking_exam_microphones`
- `select_speaking_exam_microphone`
- `get_speaking_exam_runtime_status`
- `toggle_speaking_capture`
- `start_speaking_exam_attempt`
- `pause_speaking_exam_attempt`
- `resume_speaking_exam_attempt`
- `stop_speaking_exam_attempt`
- `cancel_speaking_exam_attempt`
- `sync_speaking_attempt`
- `get_speaking_exam`
- `select_speaking_exam_class`
- `select_speaking_exam_student`
- `update_speaking_criterion_score`
- `update_speaking_criterion_level`
- `update_speaking_attempt_note`
- `approve_speaking_attempt`
