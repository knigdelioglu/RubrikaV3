# Model Runtime Ownership — Faz 4

Bu belge RubrikaV3 model runtime yaşam döngüsünün tek sahibi olan
`ModelProcessManager` için process ownership, recovery ve lease sözleşmesini
tanımlar. `ModelRuntimeService` bu coordinator için typed facade'dır; OCR,
scoring, rubric, speaking ve analysis servisleri process başlatmaz veya
durdurmaz.

## Lifecycle owner ve state machine

```text
Stopped → Starting → Healthy
   ↑         ↓         ↓
   └──── Stopping ← Draining

Healthy → Failed / Unverified
```

`Healthy`, runtime'ın teacher-facing karşılığı olan `Hazır` durumudur.
Coordinator startup'ı `startup_lock` ile single-flight yapar. `Ready` olmayan
veya bounded `/health` ve process identity doğrulamasından geçmeyen bir process
lease alamaz; completion probe bu hot path'in parçası değildir.

## Managed process identity

Persisted state PID-only değildir. `ManagedProcessIdentity` PID, owner UID,
native process start time, canonical executable path/fingerprint, argv
fingerprint, beklenen port, runtime profile fingerprint, launch instance ID ve
launch zamanını taşır.

macOS'ta `ProcessInspector` UID, start time ve executable path için `libproc`
kullanır. `lsof` ile port sahipliği yalnızca ek bir sinyaldir; tek başına kill
yetkisi vermez. `PID=0`, negatif PID veya kimliği doğrulanamayan süreç üzerinde
SIGTERM, SIGKILL veya process-group kill yapılmaz.

## Child ownership ve stop sırası

Mevcut uygulama oturumunda `tokio::process::Child` authoritative handle'dır.
Unix'te child kendi process group'unda (PGID = child PID) başlatılır; böylece
doğrulanmış graceful/force sinyali sahip olunan gruba gider. Child
`kill_on_drop` ile korunur, stdout/stderr model loguna aktarılır ve exit
watcher runtime state'i günceller. Normal explicit stop sırası:

```text
Child handle
→ live identity revalidation
→ doğrulanmış process-group SIGTERM
→ bounded wait
→ identity revalidation
→ doğrulanmış process-group SIGKILL (yalnız eşleşme sürüyorsa)
→ child.wait / reap
→ persisted state cleanup
```

Startup başarısızlığı cleanup'ı yalnız mevcut Child handle üzerinden yapılır;
henüz port açmamış process için çıplak PID kullanılmaz.

## Persisted state ve restart recovery

State uygulama config/log alanındaki `managed_model_process.json` dosyasında,
schema version ile atomik yazılır; proje klasörüne yazılmaz. Uygulama yeniden
açıldığında kayıt önce `unverified` kabul edilir:

1. PID yoksa kayıt temizlenir; sinyal gönderilmez.
2. PID varsa tüm identity sinyalleri ve beklenen port birlikte doğrulanır.
3. Eşleşme yok ve süreç portu kullanıyorsa kayıt `Unverified` olur; süreç
   korunur ve yeni duplicate runtime başlatılmaz.
4. Eşleşme yok ve beklenen port kullanılmıyorsa stale kayıt temizlenir; canlı
   sürece dokunulmaz.
5. Tam eşleşme ve health başarılıysa süreç güvenli biçimde kullanılabilir;
   stop öncesi identity tekrar doğrulanır.

## Lease/ref-count

Her model tüketimi `acquire_ready_runtime_lease(profile, consumer, operation, correlation_id)` ile
lease alır; health/readiness ve ModelGateway çağrıları bu lease kapsamındadır.
Sonuç dar ProjectStore commit edildikten sonra `lease.release()` yapılır.

Lease; lease ID, verified runtime instance ID, profile fingerprint, correlation
ID, consumer/job ve operation kind taşır. Readiness yalnız bounded `/health`
kontrolü ve process identity ile doğrulanır; completion probe hot path'in
parçası değildir. Release idempotenttir; yanlış runtime instance veya
ikinci release typed hata üretir. Bir lease'in release edilmesi yalnız kendi
registry kaydını siler.

Registry aktif lease sayısı, en eski lease yaşı ve operation kind'ları
diagnostics için tutar. Teacher-facing normal model ekranı yalnız aktif işlem
sayısını gösterir; UUID, PID, UID, path ve fingerprint diagnostics alanında
kalır.

## Startup single-flight ve profile compatibility

İlk uyumlu acquire startup'ı başlatır; eşzamanlı talepler aynı startup lock ve
aynı runtime instance'a bağlanır. Startup lock altında deadline ve bounded
backoff ile health beklenir; worker yeniden preflight yapmaz. Runtime profile fingerprint binary/model,
host/port ve runtime preset'ten deterministik üretilir.

- Aynı fingerprint yeni process başlatmaz.
- Uyumlu profil aktif lease'ler varken restart yapmaz.
- Uyumsuz profil aktif lease varken `MODEL_RUNTIME_PROFILE_BUSY` döner.
- Lease yokken geçiş serialize edilir: graceful stop → identity check → yeni
  profile start → readiness.

## Idle shutdown ve draining

Son lease release edildiğinde varsayılan 30 saniyelik idle timer başlar. Timer
runtime instance ID ve generation ile bağlıdır; yeni acquire eski timer'ı
mantıksal olarak geçersiz kılar. Timer yalnız sıfır lease ve aynı instance için
verified stop yapar.

Settings → Modeller `Durdur` eylemi aktif lease varken process'i doğrudan
öldürmez. Coordinator `Draining` durumuna geçer, yeni lease'leri reddeder ve
mevcut lease'lerin release edilmesini bekler; son lease sonrası stop yapılır.

## Unexpected exit

Child beklenmedik biçimde çıkarsa exit watcher runtime metadata'sını kaldırır,
aktif lease'leri terminal kabul eder, persisted state'i temizler ve runtime'ın
`Healthy` görünmesini engeller. Bekleyen çağrılar fake başarı veya normal zero
sonucu almaz; typed runtime-exited/startup error yolu kullanılır. Otomatik
sınırsız restart loop yoktur.

## Service integration

Production lifecycle çağrısı yalnız coordinator'dadır. `StudentAnswerOcrService`,
`ScoringService`, `RubricExtractionService`, `QuestionTextService`,
`SpeakingExamService` ve `AnalysisService` model çağrılarının etrafında lease
kullanır. `LlamaServerGateway` yalnız verified endpoint'e HTTP request, timeout
ve safe parse yapar; Child, PID veya restart state sahibi değildir.

## Diagnostics ve teacher-facing ayrımı

Normal UI `Hazır`, `İşlemler bitince durdur`, `Müdahale gerekli` gibi durumları ve
sonraki aksiyonu gösterir. Teknik PID, UID, executable path, argv/profile
fingerprint, identity mismatch ayrıntısı ve lease UUID'leri diagnostics/doctor
katmanına aittir. Teacher-facing mapper teknik enum adlarını göstermez.

## Test kanıtları

`model_process_manager` testleri identity mismatch'inde unrelated child'a
signal gönderilmediğini ve production consumer'larda global stop olmadığını
kontrol eder. Ağsız 50 eşzamanlı lease-release stress testi negatif ref-count ve
duplicate lease davranışını doğrular. Loopback tabanlı gerçek acquire/health
isolation fixture'ı açıkça ignored'dır; loopback erişimi olmayan sandboxlarda
çalıştırma sonucu environment tarafından doğrulanamaz ve `NOT VERIFIED` olarak
raporlanmalıdır.
