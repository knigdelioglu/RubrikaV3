# ProjectStore concurrency and persistence contract

Bu belge Faz 2 persistence sözleşmesidir. Faz 1'deki `TrustedProjectRoot`,
`ManagedProjectPath`, containment/symlink kontrolleri ve atomic replacement
korunur. Referans proje bu fazda değiştirilmez.

## Canonical flow

```text
Command / service
  -> ProjectStore::mutate or ProjectStore::commit_job
  -> canonical trusted-root project lock
  -> current project.json read under the lock
  -> precondition and entity validation
  -> synchronous mutation closure
  -> invariants/workflow validation
  -> storage_revision + 1
  -> trusted-root temp write, flush/sync, atomic replace
  -> updated ProjectSnapshot
```

Mutation closure synchronous'tur. Lock tutulurken model, HTTP, OCR, PDF
render, büyük kopyalama veya kullanıcı etkileşimi yapılamaz.

## Snapshot and revision

`Project.storage_revision` backend tarafından yönetilir. Alanı olmayan legacy
dosyalar `0` olarak yüklenir; salt açılış revision artırmaz. Başarılı her
canonical mutation tam bir kez artırır. Mutation closure, serialization,
atomic write veya rename başarısız olursa eski revision ve eski JSON korunur.

Backend servisleri için `ProjectSnapshot` şu bilgiyi taşır:

- canonical `project`
- `revision`
- raw serialized `content_fingerprint`
- session-bound `trusted_root`

Fingerprint FNV-1a ile deterministik olarak üretilen, güvenlik hash'i olmayan
bir değişiklik algılama değeridir. UI'ya teknik hash gösterilmez.

## Lock registry

Kilit anahtarı canonical trusted project root'tur. Registry weak references
kullanır ve yalnız canlı lock'ları tutar; tek global mutation mutex yoktur.
Aynı root'taki transaction'lar sıralanır, farklı root'lar paralel ilerler.

## Short mutation

Kısa domain değişiklikleri `ProjectStore::mutate` içindeki closure'da güncel
entity'yi ID ile bulur. Closure başarısızsa disk yazılmaz. `expected_revision`
verilmişse revision uyuşmazlığı `PROJECT_REVISION_CONFLICT` döner.

## External modification

Session'ın bildiği fingerprint ile lock altında okunan fingerprint farklıysa
veya caller'ın beklediği fingerprint uyuşmuyorsa `PROJECT_EXTERNALLY_MODIFIED`
döner. Eski session hiçbir zaman dış değişikliğin üzerine yazmaz.

Teacher-facing mesaj:

> Proje siz çalışırken başka bir işlem tarafından güncellendi. Son durumu
> yenileyip işlemi yeniden deneyin.

Teknik revision/hash/path yalnız diagnostics içindedir.

## Long job snapshot / narrow commit

Uzun iş lock'u işi boyunca tutmaz:

1. Job en güncel snapshot ve source generation/hash'i alır.
2. Model/OCR/render işi lock dışında çalışır.
3. Candidate sonuç üretir.
4. `commit_job` transaction açar, güncel project'i yeniden okur.
5. Entity/source generation closure içinde yeniden doğrulanır.
6. Yalnız job'ın sahibi olduğu alan uygulanır.

`ProjectStore::commit_snapshot_cas`, henüz closure API'sine taşınmamış eski
servislerin aday snapshot'ını kabul eder; revision/workflow/root gibi sistem
alanlarını taşımadan JSON seviyesinde değişen entity/alanları güncel project'e
merge eder. Aynı entity'nin iki farklı değişikliği `PROJECT_MUTATION_CONFLICT`
olur. Yeni job kodu doğrudan `commit_job` kullanmalıdır.

Job commit sonuçları:

- `Applied`: candidate yazıldı
- `Stale`: source/entity generation artık uyumsuz; success/applied değildir
- `Conflict`: revision veya dış değişiklik çakışması
- `EntityMissing`: hedef entity artık yok
- `Rejected`: validation veya persistence hatası

## Ownership

| Job/servis | Owned fields | Precondition |
| --- | --- | --- |
| StudentAnswerOcrService | ilgili OCR record, OCR job bağlantısı, OCR status/generation | submission/document/question source generation |
| ScoringService | ilgili scoring record, review/reconciliation, scoring generation | frozen package + scoring input hash |
| PdfPreviewService | ilgili document preview metadata | document id + source fingerprint |
| SpeakingExamService | ilgili attempt/transcript/evaluation fields | attempt id + transcript generation |
| RubricExtractionService | ilgili question rubric suggestion/status | rubric document/source generation |
| ExamPackageBuildService | package summary/freeze candidate fields | exam source + rubric/question snapshot |
| SchoolClassService | class, roster, scan-batch and assignment entities | target entity id |
| AssessmentOrganizationService | activity and class application entities | activity/application id |
| DocumentService | document metadata | document id + stored source |
| AnalysisService | analysis/readiness result fields | assessment id + source revision |

Bir job sahibi olmadığı alanları eski snapshot'tan geri yazamaz. Candidate
adapter'ı bu nedenle top-level ve ID'li array değişikliklerini değişiklik
tabanlı merge eder; workflow ve storage alanlarını ProjectStore yeniden üretir.

## Migration and legacy

Revision alanı olmayan legacy project salt açılışta `0` olarak kalır. Sadece
mevcut class/assessment migration'ı gerekiyorsa eski backup davranışı korunur;
bu migration revision artırmaz. İlk gerçek mutation revision `1` yazar.
Migration ikinci açılışta tekrar çalışmaz ve yeni root/path güvenliği geçerlidir.

## Atomic failure

Serialization/write/rename hatasında `project.json` eski geçerli dosya olarak
kalır. Temp dosyası temizlenir; mümkünse parent directory sync edilir. Faz 1
trusted-root resolver'ı temp, target ve parent için geçerlidir.

## Conflict UI

Conflict AppError olarak UI'ya gelir; sahte başarı veya otomatik stale retry
yoktur. Teacher form state'i korunur ve UI `Son durumu yenile` eylemi sunar.

## Production writer inventory

| Servis | Entity | Mutation türü | Precondition | Owned fields | Full snapshot write kaldı mı? |
| --- | --- | --- | --- | --- | --- |
| ProjectStore | Project | create / `mutate` / `commit_job` | revision + fingerprint | canonical project | Hayır |
| DocumentService | Document | short import/remove metadata | document id/source | document fields | Hayır; transactional candidate merge |
| StudentScanService | batch/submission | short mutation | entity id | scan fields | Hayır; transactional candidate merge |
| StudentAnswerOcrService | OCR record | long job + review mutation | source generation | OCR-owned fields | Hayır; transactional candidate merge |
| PdfPreviewService | Document.preview | long job commit | source fingerprint | preview fields | Hayır; transactional candidate merge |
| RubricExtractionService | Question.rubric | long job commit | rubric source | rubric fields | Hayır; transactional candidate merge |
| ExamPackageBuildService | package metadata | long job commit | source/package snapshot | package fields | Hayır; transactional candidate merge |
| ScoringService | ScoringRecord | long job / teacher override | frozen package + input hash | scoring fields | Hayır; transactional candidate merge |
| SchoolClassService | class/roster/assignment | short mutation | entity id | class-owned fields | Hayır; transactional candidate merge |
| AssessmentOrganizationService | activity/application | short mutation | entity id | assessment-owned fields | Hayır; transactional candidate merge |
| SpeakingExamService | exam/attempt | short + long job commit | attempt/transcript generation | speaking-owned fields | Hayır; transactional candidate merge |
| AnalysisService | analysis output | short result commit | assessment source | analysis fields | Hayır; transactional candidate merge |
| Workflow/commands | read model | read only / command dispatch | none | none | Hayır |

`save_project(Project)` yalnız `#[cfg(test)]` fixture compatibility yoludur.
Üretim kaynaklarında blind full-project save çağrısı bulunmamalıdır.

## Verification evidence

`project_store` unit suite includes:

- same initial snapshot, concurrent SchoolClass + AssessmentActivity update;
- two stale candidates from that same initial snapshot committed concurrently;
- expected revision conflict;
- external file fingerprint conflict;
- different-root lock parallelism;
- long-job narrow commit while an unrelated class changes;
- stale source result rejection;
- legacy revision 0 -> first mutation revision 1;
- 50 concurrent class/activity/document mutations with valid JSON and revision
  count validation.
