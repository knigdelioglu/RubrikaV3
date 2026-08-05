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

## TYMM Performans Değerlendirme — Faz A (Rust backend)

- `AssessmentType::Performance` / `WorkflowFamily::Performance` eklendi;
  `AssessmentActivity.performanceDetails` ve
  `ClassApplication.performanceAssessments` serde default ile açıldı.
- `domain/performance.rs`: `PerformanceDetails` (rubrik sürüm geçmişi dahil),
  `PerformanceRubric`, `PerformanceCriterion`, `PerformanceLevel`,
  `CriterionRating`, `PerformanceAssessment`, `PerformanceAssessmentStatus`.
- `PerformanceService`: görev CRUD (teklik anahtarı korunur), rubrik
  doğrulama (3-6 ölçüt, 3/5 düzey, gözlenebilir tanım, azalan/benzersiz
  puan), yayın = yeni sürüm, onaylı rubrik kilidi, geçici toplam hesabı
  (servis), onay kuralları, `Missing`/`NotPerformed` (sıfır puan yazılmaz).
- ProjectStore migration: `performance` workflow family türetimi +
  `performanceDetails`/`performanceAssessments` idempotent default backfill.
- 10 Tauri komutu `performance_commands.rs` + `AppState.performance_service`.
- Frontend: `performance` türüne tip uyumu + Performance DTO'ları + client
  invoke sarmalayıcıları; UI sayfaları Faz B kapsamında yazılmadı.

Doğrulama: fmt/clippy PASS; cargo test 494 lib + integration PASS;
`npm run build` + `npm run typecheck` PASS. UI smoke Faz B sonunda.

## TYMM Performans Değerlendirme — Faz B (frontend iş akışı)

- Tür yönlendirmesi: `examWorkspace.ts`'e `PERFORMANCE_EXAM_STEPS`
  (task / assessment / results) + `derivePerformanceStepStatuses`; `assessmentMode.ts`
  ve `AssessmentModeSelector`'a `performance` modu.
- `PerformanceOrganizationPage.tsx` (yeni): görev listesi (ders/dönem/sınıf
  filtresi), `/performance/new` oluşturma akışı (ders/sınıf/sıra + PerformanceDetails
  formu + rubrik taslağı), `/performance/:id` düzenleme akışı (görev bilgileri
  kaydı + rubrik düzenleyici + yayın akışı + sürüm geçmişi + onaylı rubrik kilidi).
- `PerformanceScoringPage.tsx` (yeni): sınıf öğrenci listesi + durum rozetleri,
  ölçüt bazında düzey seçimi, geçici toplam (yalnız seçili düzeyler), eksik ölçüt /
  eksik öğrenci uyarıları, `Missing`/`NotPerformed` işaretleme (sıfırdan ayrı
  görsel, puan kolonu boş), onay akışı (eksik ölçütte kapalı, onay sonrası kilit).
  `PerformanceResultsView` sonuç özeti (PDF/Excel Faz C).
- `performanceOrganizationUi.ts` (yeni): beceri alanı / çalışma biçimi / durum
  etiketleri, rubrik doğrulama (3-6 ölçüt, 3/5 düzey, gözlenebilir tanım), geçici
  toplam ve eksik ölçüt yardımcıları.
- Entegrasyon: `CanonicalExamWorkspacePage` performans adımlarını yönlendiriyor;
  `AssessmentOrganizationPage` tür filtresine `performance`, header'a giriş
  butonu ve performans kartı meta satırı eklendi; `App.tsx`'te
  `/performance`, `/performance/new`, `/performance/:id` rotaları; `projectRoutes`
  performans alanı eşlemesi; `index.css` performans stilleri.

Kullanılan komutlar: `create_performance_task`, `update_performance_task`,
`list_performance_tasks`, `get_performance_task`, `publish_performance_rubric`,
`get_performance_rubric_history`, `save_performance_assessment`,
`approve_performance_assessment`, `set_performance_assessment_status`,
`list_performance_assessments`.

Doğrulama: `npm run typecheck` PASS (~7.0s); `npm run build` PASS (~7.1s).
Kural gereği test/lint tam suite koşulmadı; UI smoke kullanıcı tarafından yapılacak.
