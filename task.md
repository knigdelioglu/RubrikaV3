# Görev kaydı

## Ortak sınıf servisi ve çoklu sınav organizasyonu

- Merkezi sınıf okuması mevcut `SchoolClassService` üzerinde tutuldu.
- Aktif ders–sınıf görevlendirmeleri `TeachingAssignment` olarak eklendi; sınav sınıf seçimleri bu kaynaktan filtreleniyor.
- `AssessmentActivity` ortak sınav kaydı ve `AssessmentClassApplication` sınıf uygulaması olarak ayrıldı.
- Yazılı, dinleme ve konuşma türleri ile `written`/`speaking` workflow family eşlemesi eklendi.

## AssessmentActivity/ClassApplication canonical speaking kaydı

- Konuşma setup ve execution akışı `AssessmentActivity.classApplications` kaynağına bağlandı; yeni speaking write yolunda bağımsız `assignedClassIds` kullanılmıyor.
- `SpeakingAttempt` activity/application/school-class referansları ve config snapshot’ı taşıyor; `ProjectStore` canonical persistence ve runtime compatibility projection’ını yönetiyor.
- Legacy `assignedClassIds` unambiguous migration, unresolved warning, idempotence ve ProjectStore reload testleri eklendi.
- Organization command sözleşmeleri, doctor canonical integrity sayaçları ve ilgili akış haritaları güncellendi.
- Ortak ve sınıfa özel belge ID bağlama seviyeleri eklendi.
- JSON ProjectStore açılış migration’ı eski sınıf alanlarını backfill ediyor; belirsiz eski konuşma sınavları unresolved bırakılıyor.
- UI organizasyon sayfası ve Kurulum → Sınıflar bağlantısı eklendi.

Doğrulama sonucu ve gerçek UI smoke sonucu teslim raporunda güncellenecektir.

## Faz 6 — final security audit kapanışı

- Model gateway: streaming 32 MiB response / 128 MiB request limitleri,
  connect/first-byte/idle timeout'ları, content-type doğrulaması,
  `ModelResponseTooLarge` typed hatası; raw body loglanmaz.
- Public error sözleşmesi: `PublicErrorDto`; `AppError` Tauri sınırında
  `technical_details` içermez; frontend `safeMessage`/`recoveryAction`/
  `retryable`/`detailsAvailable` kullanır; workflow hata yolu
  `WorkflowUnavailable` yerine typed hata + UI'da [Yeniden dene]/[Tanılama].
- Hard-coded `/Users/kadir` ve `llm/models` production kaynaklarından
  temizlendi (proof_22); model seçimi config'ten gelir, eksik path typed
  `ModelServerPathMissing`.
- Speaking "Sınavı Başlat" backend `startSpeakingExam` komutuna bağlandı;
  duplicate session koruması ve revision kanıtı
  (`speaking_backend_persistence.rs`).
- OS project lock: `ProjectWriteLease` (flock) + process-içi paylaşım;
  gerçek ikinci-process fixture testi; app single-instance flock lease.
- Portable asset serving: `managed-asset://` protokolü, relative managed
  path, traversal/symlink reddi, 32 MiB bound; asset scope genişletilmedi.
- Audit: append-only sha256 zinciri (`AuditService`), tamper/silme tespiti,
  kritik kararlarda fake success yasağı.
- Backup/restore: sınırlı `.rbackup` formatı, staging + atomic activation,
  traversal/symlink/checksum korumaları; JobManager job'ları.
- Generation GC: dry-run plan, protected set, bounded cleanup, orphan
  staging; `run_generation_gc` komutu + Ayarlar butonu.
- Doctor `SecurityDoctorSummary` sayaçları; proof_18–proof_31 testleri.
- Gates: frontend build/typecheck/lint/129 test PASS; fmt/clippy PASS;
  lib 362 PASS / 6 environment-blocked (loopback TCP, sandbox);
  integration proof'ları PASS; `.app` üretildi; DMG hdiutil sandbox'ta
  engellendi (environment-blocked).
