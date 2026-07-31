# Generation ve Rollback Güvenliği

Bu belge Phase 3'teki OCR yeniden çalıştırma, PDF preview üretimi ve submission silme güvenlik sözleşmesini tanımlar. Ana kural şudur:

```text
aktif veri korunur
→ yeni iş candidate/staging alanda çalışır
→ sonuç ve source precondition doğrulanır
→ ProjectStore transaction commit eder
→ aktif pointer ancak bundan sonra değişir
```

## OCR generation yaşam döngüsü

`Project.student_answer_ocr_generations` versioned geçmişi taşır. Her generation `generation_id`, `submission_id`, source fingerprint/document, storage revision, job provenance, model/prompt bilgisi, sonuç, diagnostics ve öğretmen review durumunu içerir.

Durumlar: `candidate`, `ready_for_review`, `active`, `rejected`, `failed`, `stale`, `interrupted`, `superseded`.

Eski projelerdeki flat `student_answer_ocr_records` alanı load sırasında korunur ve aktif read projection olarak kullanılır. Sırf açılış nedeniyle rewrite yapılmaz. İlk gerçek OCR mutation'ı generation metadata'sını oluşturur.

## OCR rerun, approval ve scoring invalidation

`force_rerun` mevcut flat OCR sonuçlarını temizlemez. Kısa `mutate` işlemi yalnız candidate generation ve submission running durumunu yazar. Model/crop/parse işlemi lock dışında yürür. Uzun sonuç `commit_job` ile şu precondition'larla commit edilir:

- submission ve source document hâlâ mevcut,
- source fingerprint aynı,
- generation/job eşleşmesi aynı,
- bütün soru kapsamı ve kayıt doğrulaması geçerli,
- stale veya silinmiş entity yok.

Başarılı sonuç, mevcut OCR veya scoring'e bağlı OCR varsa `ready_for_review` olur. Öğretmen `accept_student_answer_ocr_generation` çağırınca active pointer transaction içinde değiştirilir, eski generation `superseded` olarak kalır ve ilgili scoring kayıtları `invalidated` yapılır. `reject_student_answer_ocr_generation` eski active projection'a dokunmaz. Model start/timeout/parse/validation/interruption/source değişikliği başarısızlıklarında active sonuç korunur.

## Immutable PDF preview

Yeni preview dosyaları şu yapıda tutulur:

```text
outputs/previews/<document_id>/
  generations/<generation_id>/page-001.png
  generations/<generation_id>/manifest.json
  .staging/<generation_id>/...
```

Render active generation'ı silmeden staging'e yazılır. Bütün sayfalar, regular-file/symlink, boyut, page count, containment ve manifest generation bilgisi doğrulanır. Staging immutable generation klasörüne rename edilir; `ProjectStore::commit_job` yalnız `PdfPreviewState.active_generation_id` ve owned metadata'yı günceller. Commit başarısız veya source stale ise yeni generation temizlenir ve eski pointer olduğu gibi kalır. Legacy `cache/page_previews/<document_id>` okuması geriye dönük korunur.

## Recovery

ProjectStore açılışında canlı job sahibi olmayan `candidate` OCR generation'ları `interrupted` olarak işaretlenir; flat active OCR değişmez. Preview staging klasörleri doctor tarafından orphan sayılır; metadata tarafından active gösterilmeyen staging aktif preview'ı etkilemez ve cleanup adayıdır. Active generation klasörü eksikse okuyucu `PreviewActiveGenerationMissing` typed durumuna düşer; başka bir generation sessizce seçilmez, öğretmene rerender önerilir. Recovery tekrar çalıştırıldığında aynı sonuç korunur.

## Retention ve GC

GC politikası active, pending/review, scoring/frozen/audit tarafından referanslı ve en az bir önceki başarılı generation'ı korur. Failed/stale/interrupted generation veya orphan staging yalnız referans taramasından sonra cleanup adayıdır. GC hatası domain metadata'sını geri almaz ve aktif dosyayı silmez; orphan artifact diagnostic olarak raporlanır. Bu fazda settings UI yoktur; politika servis/doctor seviyesindedir.

## Submission dependency politikası

`delete_student_submission` önce transactional dependency scan yapar. Aktif/geçmiş OCR generation, flat OCR/review, scoring/teacher approval, frozen input, crop/page/artifact ilişkisi, persisted running job ve ilgili audit/analysis referansları silmeyi bloklar. Teacher-facing hata `StudentSubmissionInUse` mesajıdır; teknik sayaçlar diagnostics alanında kalır. Varsayılan hard cascade yoktur. Batch silme aynı scan'i `commit_job` closure içinde tekrarlar; concurrent dependency oluşursa mutation uygulanmaz.

## Artifact silme sırası

Metadata commit edilmeden tek kopya artifact silinmez. Başarılı metadata transaction sonrasında dosyanın hâlâ referanslı olup olmadığı kontrol edilir; cleanup best-effort/deferred'dir. Cleanup başarısızlığı orphan diagnostic üretir, aktif domain verisi geri alınmaz veya sessizce yok edilmez.

## ProjectStore entegrasyonu

- kısa pointer/status mutation: `ProjectStore::mutate`,
- OCR/preview uzun iş sonucu: `ProjectStore::commit_job`,
- source fingerprint/storage revision/CAS precondition: job commit closure,
- narrow owned fields: OCR generation sonucu veya preview pointer; eski full Project snapshot'ı geri yazılmaz.

## Teacher-facing durumlar

Rerun sırasında mevcut OCR görünür kalır ve “Mevcut onaylı sonuç korunuyor” mesajı gösterilir. Candidate hazır olduğunda mevcut/yeni karşılaştırması ve kabul/reddet aksiyonları görünür. Preview render sırasında eski sayfalar kullanılabilir. Yeni iş başarısız olursa kullanıcıya “Mevcut sonuç/önizleme korundu” açıklaması verilir; generation UUID, hash, staging path veya raw exception gösterilmez.
