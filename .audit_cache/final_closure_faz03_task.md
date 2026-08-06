# Final Technical Debt Closure — FAZ 3 (TD-01 AssessmentActivity Scope Migration)

## Kapsam

Proje kökü: `/Users/kadir/Desktop/RubriKa/RubrikaV3`

Bu görev, "**Final Technical Debt Closure**" kampanyasının üçüncü uygulama aşamasıdır. FAZ 0+1 ve FAZ 2 tamamlanmıştır (`docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` — tek otorite matris; bu dosyayı oku, FAZ 3 sonucuyla güncelle). Bu faz yalnız iş emrinin 3. bölümünü (TD-01) kapsar.

**Yetki ve yasaklar:**
- Production kodunda değişiklik yapmaya ve **versioned migration kodu geliştirmeye açık onay verilmiştir**.
- Hiçbir gerçek kullanıcı projesinde migration, repair, cleanup veya write çalıştırma. Migration yalnız tempdir ve committed test fixture'larında doğrulanır.
- Git commit oluşturma. Kullanıcıya ait değişiklikleri silme, stash yapma, geri alma, `git reset/clean/checkout --/restore` kullanma.
- Mevcut WIP (`stash@{0}` tur0+tur1, 25+ değişik dosya, FAZ 1/2 değişiklikleri) korunur; üzerinde çalış.
- Dosya silme yok; eski alanlar serde `default`/`skip_serializing_if` ile korunur.

## Bağlam

`docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` **TD-01 (P0)**: Yazılı sınav verileri (`questions`, `student_submissions`, `student_answer_ocr_records`, `scoring_records`, `exam_package_freeze`, `student_answer_ocr_generations` ve active projection, `QuestionText` generation, `ScoringRun/ScoringRecord`, rubrik/rubric version/frozen QEP) `Project` düzeyinde tek koleksiyonlardır. Aynı projede ikinci yazılı sınav oluşturulursa veriler karışır. `AssessmentActivity` (domain/assessment.rs) yalnız metadata taşır.

Mimari sözleşmeler: `docs/PROJECTSTORE_CONCURRENCY.md` (mutate/commit_job, storage revision), `docs/FILE_OWNERSHIP_MAP.md` (Faz 2/3 ownership), `docs/API_CONTRACTS.md`, `docs/FEATURE_FLOW_MAP.md`. Migration kapısı: `MigrateWithVerifiedBackup` (project_store.rs) korunur.

KURAL: Explore/keşif ajanları kullanma, dosyaları doğrudan oku. Plan sunma, onay isteme, soru sorma — doğrudan kod uygulamasına geç. Görev sonunda STATUS formatında rapor ver.

---

## 3. TD-01 — Yazılı sınav verilerini gerçek AssessmentActivity kapsamına taşı

Bu görevde yalnız geçici "tek yazılı sınav guard" ile yetinme. Versioned, backup-gated activity-scope migration kodunu tamamla.

### 3.1 Canonical scope

Aşağıdaki entity'ler açık `assessment_activity_id` taşımaya veya activity-owned koleksiyon altında saklanmaya başlamalı:

- Question
- StudentSubmission
- QuestionText generation
- Rubric/rubric version/frozen QEP
- StudentAnswerOcrGeneration ve active projection
- ScoringRun/ScoringRecord
- ExamPackageFreeze
- İlgili analysis/export referansları

Her kayıt ayrıca gerekli yerde `class_application_id` ve öğrenci/submission scope'unu korumalı.

Uygulama yaklaşımı (mevcut domain'e en az yıkıcı): mevcut `Project` koleksiyonlarına serde-default'lu `assessment_activity_id: Option<String>` ekle (veya eşdeğer additive alan), tüm writer/reader call-path'lerini activity-scoped yap; `list/get/readiness/workflow` okumaları aktive activity ile filtrelesin. Performans tarafı zaten activity-scope'ludur — yazılı (ve listening) family'yi kapsa.

### 3.2 Migration

- Yeni schema sürümü ekle (mevcut `storage_revision`/normalize akışına uygun, additive).
- Migration başlamadan verified backup sözleşmesi korunmalı (`MigrateWithVerifiedBackup` kapısı; gerçek projede ÇALIŞTIRMA).
- Eski flat proje tek bir written activity içeriyorsa deterministic olarak ona bağlanmalı.
- Hiç written activity yok ama legacy written data varsa sentetik/göç activity'si yalnız açık, belgeli policy ile oluşturulabilir (bu policy'yi kodda yorum satırı + docs'a yaz).
- Birden çok written activity varken flat legacy verinin hangi activity'ye ait olduğu belirsizse otomatik tahmin yapma; typed `MigrationAmbiguousAssessmentScope` blocker üret.
- İkinci migration no-op olmalı (idempotent).
- Unknown alanlar kaybolmamalı (additive; serde deny_unknown_fields YOK).
- Gerçek kullanıcı projesinde migration çalıştırma.

### 3.3 Bütün servisleri activity-scoped yap

Question/rubric/import/OCR/scoring/freeze/workflow/export command'leri `assessment_activity_id` olmadan çalışamamalı veya mevcut route/context'ten backend-authoritative biçimde çözmelidir.

Frontend ham proje-level listelerden assessment verisi türetmemeli (mevcut DTO'lara activity filtresi taşınır; frontend'de domain kararı üretilmez).

### 3.4 Zorunlu fixtures (Rust testleri, tempdir'de)

- Eski tek written sınav projesi → migration → semantik equality.
- Aynı projede iki yeni written activity → soru/OCR/scoring izolasyonu (A verisi B'ye sızmaz).
- Aynı öğrenci numarası farklı sınıflarda → karışma yok.
- Bir activity silme isteği dependency nedeniyle bloklanır.
- Activity A'nın frozen QEP'i Activity B scoring'inde kullanılamaz.

Testler kırmızı→yeşil kanıtıyla: önce izolasyonu ihlal eden fixture'ın FAIL ettiğini göster (mümkünse), sonra düzeltme sonrası PASS.

## ÇALIŞMA SÖZLEŞMESİ

- Önce mevcut projeyi ve ilgili dosyaları incele (domain/project.rs, domain/assessment.rs, services: question_text, rubric, student_scan, student_answer_ocr, scoring, exam_package_build, project_store normalize, workflow_engine, commands).
- Görev kapsamı dışındaki dosyaları değiştirme.
- `git reset`, `git clean`, `git checkout --`, `git restore`, force push, rebase veya geçmiş değiştiren Git komutlarını kullanma.
- Hiçbir koşulda Git commit, branch, tag veya pull request oluşturma.
- Gizli anahtarları, tokenleri, kullanıcı verilerini veya proje içeriğini dış servislere gönderme.
- Gereksiz biçimlendirme ve kapsam dışı refactor yapma (cargo fmt yalnız değiştirdiğin dosyalar için).
- Çalıştırılan testler başarısız olursa saklama; hata mesajlarını kısa ve doğru biçimde raporla.
- Çalışma sonunda yalnızca aşağıdaki formatta sonuç ver:

```text
STATUS: COMPLETED | BLOCKED | APPROVAL_REQUIRED | FAILED
SUMMARY: En fazla 10 satırlık sonuç özeti
CHANGED_FILES: Değiştirilen dosya yolları
VALIDATION: Çalıştırılan testler ve sonuçları (exit code + passed/failed + süre)
RISKS: Kalan riskler veya "none"
NEXT_ACTION: Gerekli sonraki işlem veya "none"
```

Onay gerektiren, geri döndürülemez, kapsamı genişleten ya da güvenlik açısından riskli bir işlemle karşılaşırsan işlemi gerçekleştirme; `STATUS: APPROVAL_REQUIRED` formatında çıkış yap (APPROVAL_REQUEST, REASON, IMPACT, ALTERNATIVES).

## Doğrulama (bu fazın kapsamı — AGENTS.md seviye E, storage/migration değişikliği)

- `cargo fmt --manifest-path src-tauri/Cargo.toml --check` (bozuksa yalnız değiştirdiğin dosyaları formatla)
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` (TAM lib suite — migration/serialization değişikliği tüm modülleri etkileyebilir; süre ~30-60s beklenir)
- Hedefli testler: `cargo test --manifest-path src-tauri/Cargo.toml --lib project_store` (migration + idempotence + ambiguity), `question_text`, `student_scan`, `student_answer_ocr`, `scoring`, `rubric`, `exam_package`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
- Frontend değişikliği varsa: `npm run typecheck`, `npm run lint`, `npm test -- --run` (yalnız etkilenen testler)
- `git diff --check`

Tam suite (check:all, smoke, tauri:build, full cargo test + integration) **Faz 11'e** aittir — bu fazda çalıştırma. Çalıştırdığın her komutun süresini raporla (komut, exit code, passed/failed/ignored, elapsed).

## Kabul kriterleri (bu faz için)

- TD-01 matriste FAZ 3 ile CONFIRMED→kapanmış; matrise migration kararları (sentetik activity policy, ambiguity blocker) işlendi.
- Yeni entity alanları additive; eski proje JSON'ları kayıpsız açılıyor (fixture testi).
- Migration idempotent (ikinci çalıştırma no-op); ambiguity'de typed blocker; tek-written-activity'de deterministic attach.
- Çoklu written activity izolasyon fixture'ları yeşil (soru/OCR/scoring/QEP).
- Activity silme dependency scan'i test edildi.
- Yeni git commit yok; kullanıcı değişikliklerine dokunulmamış; gerçek projede migration ÇALIŞTIRILMAMIŞ.
