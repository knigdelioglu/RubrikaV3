# RubrikaV3 — Güncel Teknik Borç ve Mimari Risk Denetim Raporu

Denetim tarihi: 2026-08-05
Denetim türü: Yalnız denetim ve raporlama (salt okunur)
Rapor dosyası: `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md`
Önceki rapor (hipotez kaynağı): `.hermes/desktop-attachments/Teknik Borç Denetim Raporu.md`

---

## 1. Denetim kapsamı

Bu denetim, `performans_degerlendirme` dalındaki mevcut çalışma ağacını esas alır ve TYMM uyumlu performans değerlendirme özelliğini (`AssessmentType::Performance`, `PerformanceDetails`, sürümlü rubrik, öğrenci değerlendirmesi, onay, Missing/NotPerformed, CSV/PDF raporu) uçtan uca inceler. Kapsam:

- Backend: `src-tauri/src/domain/` (tümü), `src-tauri/src/services/` (tümü), `src-tauri/src/commands/` (tümü), `src-tauri/src/jobs/` (tümü), `src-tauri/src/platform/` (tümü), `src-tauri/src/diagnostics.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/main.rs`, `src-tauri/src/bin/`, `src-tauri/tests/`, `src-tauri/Cargo.toml`.
- Frontend: `src/api/`, `src/app/`, `src/components/`, `src/pages/`, `src/state/`, `src/utils/`, `src/` kök dosyaları, `package.json`, `tsconfig.app.json`.
- Dokümanlar: `AGENTS.md`, `ARCHITECTURE.md` (gerektiğinde), `docs/` altındaki plan/denetim raporları, `.hermes/desktop-attachments/Teknik Borç Denetim Raporu.md` (yalnız hipotez).

Çalışma sözleşmesi gereği hiçbir production kodu, test, migration, yapılandırma veya kullanıcı verisi değiştirilmemiştir. Tek oluşturulan dosya bu rapordur. Git commit/amend/push yapılmamış, çalışma ağacına dokunulmamıştır.

---

## 2. Snapshot ve çalışma ağacı durumu

Denetim başında alınan salt-okunur snapshot:

```
branch:         performans_degerlendirme
HEAD:           26a41ba10d9bd234925b98fd8ff16e4002bc83d5  ("Performans değerlendirme ölçeği eklendi")
git status:     tek untracked giriş: ?? .hermes/  (denetim öncesinde de mevcuttu)
tracked değişiklik: 0 (git diff --stat boş, git diff --name-status boş)
untracked dosya: 1 (yalnız .hermes/ dizini)
git diff --check: temiz
```

Denetim başındaki ve sonundaki `git status --short` aynıdır; çalışma ağacı denetim sırasında değişmemiştir. `.hermes/` dizini (`desktop-attachments/Teknik Borç Denetim Raporu.md`) kullanıcının uygulama eklenti verisidir ve kod değişikliği değildir.

**Yeni TYMM performans özelliğine ait dosyalar** (HEAD'e son commit'te eklenen; tam listesi `git log`/diff ile doğrulandı):

- Backend: `src-tauri/src/domain/performance.rs` (129 satır), `src-tauri/src/services/performance_service.rs` (1862 satır), `src-tauri/src/commands/performance_commands.rs` (267 satır).
- `domain/assessment.rs`: `AssessmentType::Performance`, `WorkflowFamily::Performance`, `AssessmentActivity.performance_details`, `ClassApplication.performance_assessments`.
- Frontend: `src/pages/PerformanceOrganizationPage.tsx`, `src/pages/PerformanceScoringPage.tsx`, `src/pages/performanceOrganizationUi.ts` (344 satır, TDE şablon kataloğu dahil), `src/pages/performanceReportUi.ts` (92 satır).
- `src/api/commands.ts` (11 performance wrapper), `src/api/types.ts` (Performance tipleri), `src/app/examWorkspace.ts` (`PERFORMANCE_EXAM_STEPS`, `derivePerformanceStepStatuses`), `src/app/App.tsx` (rota kayıtları), `src/app/projectRoutes.ts`, `src/app/assessmentMode.ts`, `src/app/projectSwitcher.ts`.
- `src-tauri/src/lib.rs`: `AppState.performance_service` + `invoke_handler` kaydı.
- Migration: `project_store.rs::normalize_assessment_organization` (performance alanları).

Denetim sırasında başka bir süreç tarafından aynı dosyaların değiştirildiğine dair gözlem yoktur (başlangıç ve bitiş `git status` aynı).

**Önemli not:** HEAD `26a41ba` "Performans değerlendirme ölçeği eklendi" commit'idir ve çalışma ağacı temizdir; yani raporun bulguları tam olarak bu commit'in içeriğine aittir.

---

## 3. Yönetici özeti

RubrikaV3'ün `performans_degerlendirme` dalı, önceki teknik borç raporunun yazıldığı snapshot'a göre **çok ileri durumdadır**:

- Rust test kapıları tamamen yeşil (önceden 46 derleme hatası vardı): `cargo test` lib 494 + entegrasyonlar tamamı geçti, `npm run check:all` exit 0.
- Yazılı scoring doğruluk katmanı ciddi biçimde güçlendirilmiş: `decision_state`/provisional/final ayrımı, `needs_review` final toplamdan çıkarılmış, kriter eşleşmesi canonical ID'ye geçmiş, deterministik scoring + fingerprint cache eklenmiş, `structuredAnswer` typed union'a dönüşmüş.
- Model pipeline: completion probe hot path'ten çıkarılmış, tek bir `acquire_ready_runtime_lease` akışı var, JPEG cache content-keyed, prompt veri izolasyonu (system vs user) tüm use-case'lere uygulanmış, schema/prompt versioning tam, StrictLocal varsayılan + redirect/proxy engeli, `critical_term_hint` model request'inden çıkarılmış.
- Speaking text-only profil doğrulanmış; analysis yapısı structured (metrics/claims + metricRefs).

Yeni TYMM performans özelliği, **kapsam (scope) açısından doğru tasarlanmıştır**: görev + rubrik `AssessmentActivity` altında, değerlendirmeler `ClassApplication.performance_assessments` altında, rubrik sürümü kayıtta sabitlenir, Missing/NotPerformed ≠ sıfır tüm katmanlarda korunur, onay tüm ölçütleri zorunlu kılar ve onaylanmış kayıt düzenlenemez. Bu, yazılı sınav tarafındaki "veriler proje seviyesinde" sorunundan farklı olarak **doğru bir activity-scope modelidir**.

Buna rağmen denetim **iki P0** ve çok sayıda P1 bulgusu ortaya çıkarmıştır:

1. **P0 — Yazılı sınav verileri hâlâ proje seviyesinde:** `questions`, `student_submissions`, `student_answer_ocr_records`, `scoring_records`, `exam_package_freeze` `Project` düzeyinde tek koleksiyonlardır. Aynı projede ikinci bir yazılı sınav oluşturulursa soru/OCR/puan verileri birinciyle karışır. Performans tarafı bundan muaftır.
2. **P0 — `set_performance_assessment_status` onaylı kaydı silebiliyor:** `assessment_id` verilmediğinde backend, onaylı kaydı öğrenci ID'siyle bulup puanlarını/notunu silip durumu `Missing`/`NotPerformed` yapabiliyor. "Onaylanmış değerlendirmenin durumu değiştirilemez" invariant'ı yalnız `assessment_id` verilen dalda doğrulanıyor.

Diğer başlıca P1'ler: performans adım hazırlığının frontend'de türetilmesi (backend snapshot yok), taslak kaydın her kayıtta en yeni rubrik sürümüne sessizce yeniden sabitlenmesi, yanlış `assessment_id` ile duplicate değerlendirme oluşması, sınıf uygulaması silmenin performans değerlendirmelerini dependency scan'siz silmesi, performans raporunda geçici (InProgress) toplamların final toplamla ayrılmadan gösterilmesi, CSV formül enjeksiyonu, frontend eşzamanlı mutation yarışı, count-only scoring/OCR readiness, job rehydration hatasının yutulması, birden fazla workflow otoritesi, teknik veri sızıntıları, job manager'daki production `unwrap()`'ları.

**Denetim kararı:** `RELEASE_BLOCKED` — P0'lar çözülmeden performans özelliği genel kullanıma açılmamalıdır; pilot (kısıtlı, tek ders/sınıf) kullanım P0-2'nin hızlı backend düzeltmesiyle mümkündür.

---

## 4. Mevcut test ve release durumu

Bu denetimde çalıştırılan kapılar (kod değişmeden, nihai snapshot üzerinde):

| Komut | Exit | Sonuç | Süre | Hata sınıfı |
|---|---|---|---|---|
| `npm run typecheck` | 0 | PASS | ~20s | — |
| `npm run lint` | 0 | PASS (0 hata, 4 warning — tümü `PerformanceScoringPage.tsx` exhaustive-deps) | ~10s | — |
| `npm test` | 0 | PASS 147/147 (0 fail, 0 skip) | ~2.3s | — |
| `npm run build` | 0 | PASS (tsc + vite) | ~2s | — |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | 0 | PASS | ~1s | — |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | 0 | PASS | ~91s | — |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | 101 | **FAIL** — 5 clippy hatası, tamamı `#[cfg(test)]` test kodunda: `deterministic_scoring_service.rs:888` (`field_reassign_with_default`), `scoring_anchor_service.rs:626/636/647` (aynı lint), `scoring_cache_service.rs:392` (`cloned_ref_to_slice_refs`) | ~40s | test-code lint (production derleme hatası değil) |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` | 0 | PASS 494 passed, 0 failed, 4 ignored | 21s | — |
| `cargo test --manifest-path src-tauri/Cargo.toml` (tüm hedefler) | 0 | PASS: lib 494 (4 ignored); `final_data_loss_proofs` 11 (1 ignored); `final_security_proofs` 8; `project_creation_regression` 1; `project_lock_process_fixture` 2; `speaking_backend_persistence` 1 | ~186s (lib) | — |
| `npm run check:all` | 0 | PASS (build+typecheck+lint+test+cargo:fmt+cargo:clippy+cargo:test) | ~8dk | — |
| `npm run tauri:dev -- --smoke` | n/a | **BLOCKED** — `Port 5173 is already in use` (vite `strictPort: true`, vite.config.ts:10). Portta iki canlı vite süreci var (PID 88309 = bu repo, PID 71207 = farklı proje `PayCraft`). Bu süreçler kullanıcıya ait olduğundan sonlandırılmadı. | — | environment (port çakışması) |
| `git diff --check` | 0 | PASS | — | — |

Notlar:

- Önceki raporun P0 #3'ü ("cargo test 46 derleme hatası") **artık geçerli değildir** — testler derleniyor ve tamamı geçiyor.
- Önceki raporlarda belgelenen "7 model-başlatma testi hang ettiği" sorunu bu ortamda görülmedi: `start_job_returns_model_mmproj_missing` ve `start_job_returns_model_start_failed_when_binary_exits` dahil model-sunucu testleri geçti (ortamda model ikilisi mevcut olmalı veya testler hang-etmeyecek şekilde düzeltilmiş).
- `cargo clippy --all-targets --all-features -- -D warnings` yalnız `npm run check:all`'ın **kapsamı dışındadır** (check:all `cargo:clippy` = `-- -D warnings`, `--all-targets` olmadan → exit 0). Bu 5 hata test kodundadır; production derleme ve test çalıştırmasını engellemez ancak "tüm hedeflerde -D warnings" temiz değildir.
- Smoke testi bu ortamda port çakışması nedeniyle koşulamadı; prior audit kayıtları smoke'un port boşken PASS olduğunu gösteriyor. Smoke, `lib.rs:112`'de `RUBRIKA_SMOKE` ile app sürecinin exit(0) yapmasını sağlar; kod derlenmiş ve app başlatma yolu etkilenmemiştir.

---

## 5. P0 bulgular

### P0-1 — Yazılı sınav verileri proje seviyesinde tek koleksiyonda (AssessmentActivity yalnız metadata)

```text
ID: TD-01
Öncelik: P0
Başlık: Yazılı sınav soru/OCR/scoring verileri hâlâ proje seviyesinde; ikinci yazılı sınav ilkiyle karışır
Durum: CONFIRMED (yazılı taraf; performans tarafı muaftır)
Kaynak: src-tauri/src/domain/project.rs:44-82; src-tauri/src/domain/assessment.rs:97-161
Etkilenen akış: Yazılı sınav A + Yazılı sınav B (aynı proje); scoring/QEP/OCR tümü
Somut kod kanıtı:
  - Project struct (project.rs:44-82):
      pub students: Vec<Student>,
      pub questions: Vec<Question>,                       // proje seviyesi
      pub student_submissions: Vec<StudentSubmission>,    // proje seviyesi
      pub student_answer_ocr_records: Vec<StudentAnswerOcrRecord>, // proje seviyesi
      pub scoring_records: Vec<ScoringRecord>,            // proje seviyesi
      pub exam_package_freeze: Option<ExamPackageFreeze>, // proje seviyesi
  - AssessmentActivity (assessment.rs:133-161) yalnız metadata + class_applications + performance_details/speaking/listening taşır; yazılı taraf için soru/rubrik/OCR/scoring referansı yoktur.
  - scoring_readiness (src-tauri/src/domain/scoring.rs:648-751) `project.student_submissions * project.questions` üzerinden hesap yapar; activity ID kullanmaz.
Call/data flow:
  create_assessment_activity(Written) → AssessmentActivity (id=X)
  → start_question_text_extraction / import_rubric → project.questions'ı doldurur (activity bağlantısı yok)
  → start_student_answer_ocr → project.student_answer_ocr_records (activity bağlantısı yok)
  → start_scoring_job → project.scoring_records (activity bağlantısı yok)
  → İkinci yazılı sınav (id=Y) aynı koleksiyonlara yazar.
Kullanıcıya etkisi: Aynı projede iki yazılı sınav yönetilmeye çalışılırsa ikinci sınavın soru/rubrik listesi birinciyi görür veya ezebilir; OCR/scoring kartesyeni iki sınavın öğrenci-soru çarpımıyla karışır; QEP freeze tek paketle sınırlı olduğundan iki sınav tek paket gibi dondurulur.
Veri doğruluğu etkisi: Farklı sınavların verilerinin karışması → yanlış öğrencinin yanlış soruya ait puanı, yanlış toplam, yanlış sınav raporu.
Neden teknik borç: Domain modeli "tek proje = tek yazılı sınav" varsayar; organizasyon katmanı çoklu sınavı desteklerken veri katmanı desteklemiyor.
Hangi koşulda ortaya çıkar: Aynı projede ikinci bir yazılı (veya dinleme — written family) sınavı oluşturulup kullanıldığında. İlk sınavda normal kullanımda görünmez.
Mevcut koruma: Frontend'de sınav listesinden giriş, adım durumları; `AssessmentActivity.is_same_key` teklik anahtarı; ancak bunlar veri scope'u sağlamaz.
Korumanın yetersiz kaldığı nokta: Backend hiçbir mutation'da "bu soru/rubrik/OCR/scoring kaydı hangi assessment activity'e ait" bilgisini doğrulamaz çünkü bu bilgi kaydedilmez.
Önerilen çözüm: Yazılı (ve listening) verilerini activity altına taşımak için versioned migration + DTO değişikliği; `Question`, `StudentSubmission`, OCR, scoring kayıtlarına `assessment_activity_id` eklemek ve tüm komut akışlarını activity-scoped yapmak. Alternatif olarak proje seviyesinde "tek aktif yazılı sınav" kısıtı uygulamak (P0'e ertelenebilir kısa kapatma).
En küçük güvenli çözüm: P0 kapsamında projede ikinci yazılı sınav oluşturmayı backend'de engellemek (`ASSESSMENT_ACTIVITY_IN_USE` benzeri); gerçek çözüm Tur 3'te.
İdeal uzun vadeli çözüm: Activity-scoped domain (assessment id tüm entity'lerde) + migration + e2e çoklu sınav testleri.
Migration gerektiriyor mu: Gerçek çözüm için evet (versioned, tek yönlü, backup-gated).
Geriye uyumluluk riski: Yüksek — eski projelerin mevcut flat kayıtları "hangi sınavın" olduğu bilinmeden scope'lanamaz; varsayılan olarak ilk/tek sınavı temsil etmelidir.
Test ihtiyacı: İki yazılı sınavlı proje fixture; sınav A puanının sınav B'ye sızmasını kanıtlayan regresyon; migration sonrası eski tek sınav projesinin aynı kalması.
Tahmini kapsam: XL (gerçek çözüm) / S (kapatma kısıtı)
Bağımlı olduğu diğer borçlar: TD-11 (workflow tek otorite), TD-12 (count-only readiness) — bunlar da activity scope'suzdur.
Önerilen uygulama sırası: Tur 3 (kapsam migration); kısa süreli kapatma Tur 1'e sığar.
```

### P0-2 — `set_performance_assessment_status` onaylı öğrenci kararını silebiliyor

```text
ID: TD-02
Öncelik: P0
Başlık: assessment_id boşken onaylı değerlendirmenin durumu Missing/NotPerformed'a çekilip puanları silinebiliyor
Durum: CONFIRMED (backend invariant açığı; mevcut UI çoğunlukla ID'yi gönderir, ama stale-state yarışı tetikleyebilir)
Kaynak: src-tauri/src/services/performance_service.rs:778-843
Etkilenen akış: set_performance_assessment_status (Missing/NotPerformed işaretleme)
Somut kod kanıtı:
  performance_service.rs:778-793:
      if let Some(assessment_id) = input.assessment_id.as_deref() {
          if ...find(assessment_id).is_some_and(status == Approved) { return Err(...); }
      }
      // input.assessment_id None ise onay kontrolü YAPILMAZ
  performance_service.rs:808-843:
      } else {
          match application.performance_assessments.iter_mut().find(|a| a.student_id == input.student_id) {
              Some(assessment) => assessment,   // onaylı olsa bile alınır
              None => { ... yeni kayıt ... }
          }
      };
      assessment.ratings = vec![];
      assessment.provisional_total = 0;
      assessment.assessed_at = None;
      assessment.status = input.status;   // Approved → Missing/NotPerformed, puanlar silindi
Call/data flow: frontend statusMutation (assessmentId: selectedAssessment?.id) → set_performance_assessment_status
  → None dalında student_id ile arama → onaylı kayıt üzerine yazma.
  Alternatif tetikleyici: assessments listesi eskiyken öğretmen "Eksik" derse selectedAssessment undefined kalır → assessment_id None → backend onaylı kaydı bulur ve siler.
Kullanıcıya etkisi: Onaylanmış (final) bir öğrenci kararı sessizce "Eksik"e döner; tüm puanlar/geri bildirim kaybolur; audit "durum güncellendi" kaydeder (yanlış olay). Rapor geçmişi olmadığı için geri döndürülemez.
Veri doğruluğu etkisi: Öğretmen onayıyla verilmiş final kararın silinmesi = veri kaybı + "öğretmen onayı olmadan final karar değişimi".
Neden teknik borç: Onay koruması yalnız bir kod dalında var; komut kontratı `assessment_id: Option` izin verir.
Hangi koşulda ortaya çıkar: assessment_id None ile çağrıldığında ve öğrencinin onaylı kaydı varsa (doğrudan API çağrısı veya frontend'in stale listeyle buton göstermesi).
Mevcut koruma: Frontend ApprovedAssessmentView'da durum butonlarını göstermez; UI onaylı kayda ID ile işaret eder. Bu bir backend koruması değildir.
Korumanın yetersiz kaldığı nokta: Backend, "onaylı kayıt status değiştirilemez" kuralını None dalında zorlamıyor.
Önerilen çözüm: `set_performance_assessment_status` içinde, assessment_id None iken student_id ile bulunan kayıt Approved ise reddet; None dalında yeni kayıt oluşturmadan önce mevcut kaydın durumunu kontrol et.
En küçük güvenli çözüm: function başında, bulunan kayıt için `if status == Approved { return Err(AssessmentActivityInUse) }` kuralını None dalına da genişlet; + regresyon testi.
İdeal uzun vadeli çözüm: Değerlendirme mutasyonlarını ortak bir state machine üzerinden yürütmek; Approved state'ten yalnız typed "yeni değerlendirme aç" akışı çıkabilsin.
Migration gerektiriyor mu: Hayır.
Geriye uyumluluk riski: Düşük (yalnızca kurallı kullanımı sıkılaştırır).
Test ihtiyacı: onaylı kayıt + assessment_id=None set_status reddi; onaylı kayıt + doğru ID ile reddi; onaysız kayıt ile None akışının hâlâ çalışması; stale-frontend senaryosu.
Tahmini kapsam: S
Bağımlı olduğu diğer borçlar: TD-09 (stale overwrite) ile birlikte değerlendir.
Önerilen uygulama sırası: Tur 1 (veri/puan doğruluğu) ilk madde.
```

---

## 6. P1 bulgular

### P1-1 — Performans adım hazırlığı yalnız frontend'de türetiliyor (backend authoritative snapshot yok)

```text
ID: TD-03
Öncelik: P1
Başlık: Performance task/assessment/results adım durumları backend WorkflowSnapshot'sız, frontend helper'ında hesaplanıyor
Durum: CONFIRMED
Kaynak: src/app/examWorkspace.ts:396-470 (derivePerformanceStepStatuses); src-tauri/src/services/workflow_engine.rs (performance kolu yok — grep "performance" hiç eşleşmedi)
Etkilenen akış: /project/:projectId/activities/:activityId/task|assessment|results
Somut kod kanıtı: derivePerformanceStepStatuses, activity.performanceDetails/rubricVersions/classApplications.performanceAssessments/studentScopeIds üzerinden "blocked (rubrik yayınlanmadan…)", "completed (approvedCount >= totalStudents)", "ready (results approvedCount > 0)" kararlarını frontend'de üretir. Backend WorkflowSnapshot yalnız yazılı/speaking için türetilir (workflow_engine.rs:66-126).
Kullanıcıya etkisi: Rubrik publish durumu, onay tamamlanma durumu gibi hazırlık sinyalleri arka plan değişikliğinde bayat kalabilir; backend reddi (RUBRIC_MISSING gibi) ancak buton tıklanınca görünür.
Veri doğruluğu etkisi: Doğrudan puan hatası değil; "blocked" gösterilip yine de geçilebilen tek engel backend'in kendisidir (publish rubrik zorunlu kılan save gate korur).
Neden teknik borç: AGENTS.md kuralı "UI domain readiness üretmez" ihlali; yazılı tarafta düzeltilen desen performans tarafında yeniden üretilmiş.
Önerilen çözüm: Backend'e performance için authoritative step/readiness snapshot eklemek (ör. get_performance_workflow_snapshot: published rubric varlığı, approved/eksik/değerlendirilen sayaçları) ve frontend'in bunu render etmesi.
En küçük güvenli çözüm: `get_performance_report` benzeri salt-okunur bir `get_performance_status` DTO'su; derivePerformanceStepStatuses yalnız bu DTO'yu tüketir.
İdeal uzun vadeli çözüm: WorkflowSnapshot'a performance family kolu eklemek.
Migration: Hayır. Test: frontend + backend kontrat testi. Kapsam: M. Sıra: Tur 2.
```

### P1-2 — Taslak değerlendirme her kayıtta en yeni rubrik sürümüne sessizce yeniden sabitleniyor

```text
ID: TD-04
Öncelik: P1
Başlık: save_performance_assessment, InProgress kaydın rubric_id/rubric_version değerini en yeni yayınlanmış sürümle ezberi yeniden sabitliyor (K8 "eski puanlar yeniden hesaplanmaz" ihlali — taslak kapsamı)
Durum: CONFIRMED
Kaynak: src-tauri/src/services/performance_service.rs:584-588 (existing dalında) ve :552-553 (validate + provisional_total en yeni rubriğe göre)
Somut kod kanıtı:
  let (school_class_id, latest_rubric) = { ... latest_published_rubric(details) ... };   // satır 524-530
  validate_ratings(&input.ratings, &latest_rubric)?;                                      // satır 552
  let provisional_total = compute_provisional_total(&input.ratings, &latest_rubric);      // satır 553
  existing.rubric_id = latest_rubric.id.clone();
  existing.rubric_version = latest_rubric.version;                                        // satır 587-588
Call/data flow: publish v1 → taslakları kaydet (InProgress, v1 sabit) → publish v2 (onaylı yoksa serbest) → aynı taslağı yeniden kaydet → puanlar v2'nin düzey puanlarıyla yeniden hesaplanır ve sürüm v2'ye taşınır.
Kullanıcıya etkisi: Düzey puanları sürümler arasında değiştiyse öğretmenin önceki taslağının toplamı sessizce değişir; rubrik sürüm geçmişinde "öğrenci v2 ile değerlendirildi" görünür.
Veri doğruluğu etkisi: Tarihsel puanın hangi rubriğe göre verildiği değişir; rapor öğrenci satırını "kayıttaki sürüm"e göre çözdüğü için görünüm tutarlı kalır ama sürüm sabitleme değişmezliği bozulur.
Neden teknik borç: K8 invariant'ı yalnız publish kilidinde (onaylı var → yeni sürüm yayınlanamaz) korunuyor; kayıt düzeyinde "kullandığı sürümü sabit tutma" yok.
Hangi koşulda ortaya çıkar: v1 taslaklar varken v2 yayınlanıp taslak yeniden kaydedildiğinde; ya da öğrenci farklı sürümde başlanmışsa.
Mevcut koruma: Onaylı değerlendirme varsa yeni sürüm yayınlanamaz; onaylı kayıt düzenlenemez.
Korumanın yetersiz kaldığı nokta: InProgress taslaklar korunmuyor.
Önerilen çözüm: Save'i kayıt mevcutsa kaydın mevcut rubrik sürümüne göre validate/hesapla; yalnız yeni kayıtta en yeni sürümü sabitle; veya sürüm değişimini öğretmene açık "yeni rubrik sürümüyle yeniden değerlendir" onayına bağla.
En küçük güvenli çözüm: existing kayıt için rubric_id/version'ı dokunmadan bırak; validate_ratings/compute_total'ı kaydın sabitlediği sürümle yap.
İdeal uzun vadeli çözüm: Sürüm sabitleme + görüntüleme rubriği ayrımını domain'de netleştir; sürüm değişiminde taslaklar için invalidation durumu.
Migration: Hayır. Geriye uyum riski: Düşük. Test: v1 kayıt + v2 yayın + yeniden kayıt → toplam/sürüm değişmez. Kapsam: S-M. Sıra: Tur 1.
```

### P1-3 — Yanlış/başka uygulamaya ait assessment_id ile duplicate değerlendirme oluşuyor

```text
ID: TD-05
Öncelik: P1
Başlık: save_performance_assessment, assessment_id'yi application/student kapsamında doğrulamıyor; yabancı ID'de sessizce ikinci kayıt oluşturuyor
Durum: CONFIRMED
Kaynak: src-tauri/src/services/performance_service.rs:568-575
Somut kod kanıtı:
  let existing = application.performance_assessments.iter_mut().find(|assessment| {
      Some(&assessment.id) == input.assessment_id.as_ref()
          || (input.assessment_id.is_none() && assessment.student_id == input.student_id)
  });
  // assessment_id Some ama bu uygulamada değilse → None döner → else dalı YENİ kayıt yaratır.
Kullanıcıya etkisi: Aynı öğrenci için aynı sınıf uygulamasında iki InProgress kaydı oluşabilir; ikisi de onaylanırsa rapor .find ile ilkini gösterir, ikinci karar görünmez.
Veri doğruluğu etkisi: Öğretmen kararının hangi kaydın üzerinde olduğu belirsizleşir; "aynı öğrenci için iki final kaydı" riski.
Neden teknik borç: Komut kontratı kimlik çapraz doğrulaması içermiyor; backend "ID'yi bulamazsan hata ver" yerine "yeni kayıt" davranışı seçiyor.
Önerilen çözüm: assessment_id Some ise kaydın bu application içinde VE student_id ile eşleştiğini doğrula; eşleşmiyorsa typed hata döndür.
En küçük güvenli çözüm: find koşuluna application-üyelik + student eşleşmesi eklemek; yabancı ID'de AssessmentClassApplicationNotFound/AssessmentInvalidInput.
Migration: Hayır. Test: başka uygulamaya ait ID → red; duplicate kayıt oluşamama. Kapsam: S. Sıra: Tur 1.
```

### P1-4 — Sınıf uygulaması silme, performans değerlendirmelerini dependency scan'siz siliyor

```text
ID: TD-06
Öncelik: P1
Başlık: remove_class_application, performance_assessments (onaylı dahil) varlığını kontrol etmiyor
Durum: CONFIRMED
Kaynak: src-tauri/src/services/assessment_organization_service.rs:650-663
Somut kod kanıtı:
  let has_attempts = !application.speaking_attempts.is_empty()
      || project.speaking_exams.iter().any(...);   // YALNIZ speaking kontrolü
  // performance_assessments kontrol edilmez; kayıtlar application içinde gömülü olduğu için silinir.
Call/data flow: remove_assessment_class_application → remove_class_application → application.remove(index) → kayıtlar kaybolur.
Kullanıcıya etkisi: Öğretmen yanlışlıkla sınıf uygulamasını kaldırırsa tüm değerlendirme kararları (onaylı dahil) geri döndürülemez biçimde silinir.
Veri doğruluğu etkisi: Veri kaybı; domain geçmişi (puan, onay tarihi, rubrik sürümü) korunmaz.
Neden teknik borç: Delete-dependency taraması yalnız speaking için güncellenmiş; performans yeni eklendiğinde scan'e eklenmemiş.
Önerilen çözüm: has_attempts kontrolüne `!application.performance_assessments.is_empty()` ekle; silme yerine typed `AssessmentClassApplicationInUse`.
En küçük güvenli çözüm: performance_assessments boş değilse reddet (speaking ile aynı desen).
Migration: Hayır. Test: onaylı değerlendirmesi olan uygulama silinemez; boş uygulama silinebilir. Kapsam: S. Sıra: Tur 1.
```

### P1-5 — Performans raporu geçici (InProgress) toplamları final toplamla ayrım yapmadan gösteriyor

```text
ID: TD-07
Öncelik: P1
Başlık: get_performance_report, onaylanmamış InProgress değerlendirmelerin geçici toplamını rapor satırında "total" olarak basıyor; provisional/final ayrımı yok
Durum: CONFIRMED
Kaynak: src-tauri/src/services/performance_service.rs:1055-1063, 989
Somut kod kanıtı:
  total: assessment.filter(|a| matches!(a.status, Approved | InProgress)).map(|a| a.provisional_total),
  // InProgress kaydın geçici toplamı rapor toplamı olarak gösterilir
  summary.assessed_count += 1  (InProgress dahil, satır 989)
Kullanıcıya etkisi: Öğretmen raporu basarken onaylanmamış öğrencinin geçici puanı final puan gibi görünür; CSV/PDF'te ayrım etiketi yok.
Veri doğruluğu etkisi: "Onaysız öğrenci rapora girebiliyor" sorusunun cevabı evet; rapor dışa aktarılırsa finalleşmemiş karar yayılır.
Neden teknik borç: DTO'da provisionalTotal/finalTotal ayrımı yok; plan §2 "geçici/final ayrımı" eksik.
Önerilen çözüm: DTO'ya `isApproved`/`provisional` alanı veya ayrı `provisional_total` ve `final_total` ekle; CSV/PDF'ta onaysız satırı açıkça işaretle; opsiyonel olarak raporu yalnız onaylı satırlarla üretme modu ekle.
En küçük güvenli çözüm: Satır `status` alanını rapor tablosunda görünür kıl (halihazırda status var ama tabloda onaysız "değerlendirildi" gibi gösteriliyor); toplam sütununda yalnız Approved satırlarında değer.
Migration: Hayır. Test: InProgress öğrenci rapor satırı ayrım testi. Kapsam: S-M. Sıra: Tur 1.
```

### P1-6 — Performans CSV raporunda formül enjeksiyonu koruması yok

```text
ID: TD-08
Öncelik: P1
Başlık: CSV export'unda öğrenci adı/no gibi kullanıcı kontrollü hücreler "=", "+", "-", "@" ile başlıyorsa formül olarak yazılıyor
Durum: CONFIRMED
Kaynak: src/pages/performanceReportUi.ts:38-44 (escapeCell), :68-69 (studentName/studentNumber)
Somut kod kanıtı: escapeCell yalnız [";\n\r] içeren hücreleri tırnaklar; "=HYPERLINK(...)" gibi hücreler tırnaksız yazılır. Excel'de formül olarak yorumlanır.
Kullanıcıya etkisi: Kötü niyetli veya tesadüfi bir öğrenci adı raporu açan öğretmenin Excel'inde istenmeyen formül çalıştırabilir (veri sızıntısı/pivot saldırısı riski).
Veri doğruluğu etkisi: Rapordaki öğrenci adı bozulabilir; bilgi ifşa riski.
Neden teknik borç: Export sözleşmesinde CSV injection kuralları yok.
Önerilen çözüm: `escapeCell`'e `=`, `+`, `-`, `@`, `\t`, `\r` ile başlayan hücreleri `'` önekiyle veya tırnakla yazma kuralı ekle; ayrıca XLSX varsa hücreyi string tipinde işaretle.
En küçük güvenli çözüm: önek/kaçış kurallı escapeCell + birim test.
Migration: Hayır. Test: "=HYPERLINK..." adlı öğrenci CSV'si. Kapsam: S. Sıra: Tur 1.
```

### P1-7 — Performans değerlendirme ekranında eşzamanlı mutation yarışı ve draft kaybı

```text
ID: TD-09
Öncelik: P1
Başlık: PerformanceScoringPage'de save in-flight iken approve/status butonları aktif; kayıt esnasındaki düzenlemeler refetch ile sessizce siliniyor
Durum: CONFIRMED
Kaynak: src/pages/PerformanceScoringPage.tsx:282-288 (canApprove), 612-659 (butonlar), 149-162 (refetch-reset effect)
Somut kod kanıtı: canApprove ve status butonları saveMutation.isPending'i kontrol etmiyor; kayıt bittiğinde refreshAssessments() → assessmentByStudent yeniden kurulur → draft state (ratingDrafts/feedback) sunucu snapshot'ıyla sıfırlanır.
Kullanıcıya etkisi: "Kaydet" düğmesine basıp hemen "Onayla"ya basan öğretmen iki komutu aynı anda gönderir; kayıt bitmeden yazılan düzey/geri bildirim kaybolabilir.
Veri doğruluğu etkisi: Eşzamanlı komut sırası → öğretmenin yeni kararı daha eski taslak tarafından overwrite edilebilir (lost update).
Neden teknik borç: Mutation'lar arası koordinasyon yok; AGENTS "her eylem disabled reason/loading" kuralı kısmen.
Önerilen çözüm: save in-flight iken approve/status/revert butonlarını devre dışı bırak; save onSuccess'inde yalnız ilgili assessment'ı tazele; draft state'i refetch yerine mutation sonucuyla güncelle.
En küçük güvenli çözüm: butonlara `!saveMutation.isPending` guard + effect'te "pending yokken sıfırla" koşulu.
Migration: Hayır. Test: component testi (save pending → approve disabled; draft korunur). Kapsam: S-M. Sıra: Tur 1 veya Tur 2.
```

### P1-8 — Frontend domain readiness'i yeniden üretiyor (workflow tek otorite yok)

```text
ID: TD-10
Öncelik: P1
Başlık: Persisted project.workflow + canlı evaluate_workflow + workflow_engine kısa devre + frontend helper'ları birden fazla otorite
Durum: CONFIRMED (önceki rapor #4/#5 ile tutarlı; performans için TD-03 ayrıca)
Kaynak: src-tauri/src/domain/project.rs:82 (persisted workflow), src-tauri/src/services/project_store.rs:845 (her mutation'da yeniden yazılır), src-tauri/src/services/workflow_engine.rs:384-400 (persisted snapshot kısa devresi), src/app/examWorkspace.ts:91/396 (frontend türetimi)
Somut kod kanıtı: workflow_engine.rs:384-400, `!exam_package_frozen && current_stage in [ExamPackageBuildReady|Running|ReviewNeeded|...]` ise `project.workflow` (persisted) olduğu gibi döner — canlı hesaplama yapılmaz. Ayrıca student_scan_service.rs:1407-1547, exam_package_build_service.rs:142-464 elle WorkflowSnapshot yazıyor.
Kullanıcıya etkisi: Ekran/backend/persisted stage uyuşmazlığı olasılığı; öğretmen yanlış "hazır" sinyali görebilir.
Veri doğruluğu etkisi: Dolaylı — scoring gate backend'de hâlâ frozen QEP ister.
Neden teknik borç: Tek otorite ilkesi ihlali; frontend "hazır" kararını backend'in yerine üretiyor.
Önerilen çözüm: WorkflowSnapshot'ı salt hesaplanan (computed) yapıp persisted snapshot'ı yalnız cache olarak tutmak; kısa devreyi kaldırmak; frontend türetimini backend snapshot'a indirgemek.
En küçük güvenli çözüm: workflow_engine kısa devresini kaldırıp her zaman canlı hesaplama döndürmek; derivePerformanceStepStatuses için TD-03 çözümü.
Migration: Hayır. Test: workflow birim testleri + performans adım testleri. Kapsam: M. Sıra: Tur 2.
```

### P1-9 — Scoring/OCR readiness yalnız kayıt sayısına dayanıyor (duplicate false-ready riski)

```text
ID: TD-11
Öncelik: P1
Başlık: scoring_readiness OCR hazırlığını count eşitliğiyle ölçüyor; (submission_id, question_id) tekliliği doğrulanmıyor
Durum: CONFIRMED (önceki rapor #7 ile aynı)
Kaynak: src-tauri/src/domain/scoring.rs:649, 698-703
Somut kod kanıtı:
  let expected_records = project.student_submissions.len() * project.questions.len();
  ocr_ready = expected_records > 0
      && project.student_answer_ocr_records.len() == expected_records   // count-only
      && ...all(TeacherApproved && !needs_review);
Call/data flow: Aynı (submission,question) ikilisi için duplicate flat kayıt sayıyı şişirirse + başka bir ikili eksikse count eşit görünür → scoring gate açılır.
Kullanıcıya etkisi: Eksik OCR'li bir öğrenci/soru varken "hazır" görünüp scoring başlatılabilir; model puanı eksik kayıt için üretilmez.
Veri doğruluğu etkisi: Kartesyan kapsam garantisi yok → yanlış "tamamlandı" ve eksik puan.
Neden teknik borç: Readiness, teklilik/kapsam yerine cardinality üzerine kurulu.
Önerilen çözüm: Beklenen ikili kümesini hesaplayıp gerçek kayıtların bu kümeyi örtüp örtmediğini ve duplicate olmadığını doğrula; kapsam eksikliği için structured blocker.
En küçük güvenli çözüm: set-based coverage kontrolü (duplicate red + missing pair listesi).
Migration: Hayır. Test: duplicate fixture false-ready'yi engeller. Kapsam: M. Sıra: Tur 1/Tur 2.
```

### P1-10 — Legacy scoring default'u hâlâ `scoring_applied=true`

```text
ID: TD-12
Öncelik: P1
Başlık: ScoringRecord.scoring_applied serde default'u true; legacy kayıtlar için risk migration'da azaldı ama doğrudan default tehlikeli kaldı
Durum: PARTIAL (önceki rapor #8; load-time normalize azaltıyor ama serde default değişmedi)
Kaynak: src-tauri/src/domain/scoring.rs:281-284, 341-343; src-tauri/src/services/project_store.rs:2663-2742 (normalize_scoring_records)
Somut kod kanıtı: `#[serde(default = "default_scoring_applied")] pub scoring_applied: bool;` → true. decision_state default = Provisional (scoring.rs:120-124). normalize_scoring_records yalnız legacy JSON yüklenirken devreye girer; in-process oluşturulan veya normalize edilmeyen kayıtlarda default true kalır. scoring_record_is_final TeacherApproved ister → legacy final olmaz; ama scoring_record_is_accepted, migrated auto_accepted legacy kayıtları accepted toplamına sokar.
Kullanıcıya etkisi: Eski kayıtlar final toplamda görünmez; accepted/provisional toplamlarda görünebilir.
Veri doğruluğu etkisi: Sessiz default true → yeni doğrulama akışından geçmemiş bir kayıt "uygulanmış" sayılabilir.
Neden teknik borç: Serde default ile "yok" bilgisi "doğru" olarak temsil ediliyor; fail-closed olmaktan uzak.
Önerilen çözüm: default'u `false` yap ve yokluğu eksik bilgi olarak ele al; load-time normalization'da eksik alanı explicit state'e bağla.
En küçük güvenli çözüm: `default_scoring_applied` → false + normalize kontrolünde eski true davranışını koru.
Migration: Evet (semantic migration, veri yapısı değil). Test: alanı olmayan kayıt final/ready değildir. Kapsam: S-M. Sıra: Tur 1.
```

### P1-11 — Frontend error boundary runtime-safe değil

```text
ID: TD-13
Öncelik: P1
Başlık: unknown değerler doğrudan AppError olarak cast ediliyor; command/event payload'ları runtime schema ile doğrulanmıyor
Durum: CONFIRMED (önceki rapor #9 ile aynı)
Kaynak: src/api/commands.ts:115-117, src/api/tauriClient.ts, src/pages/PerformanceScoringPage.tsx:345-350/804
Somut kod kanıtı: `if (typeof e === 'object' && e !== null && 'code' in e) { throw e as AppError; }` — yalnız "code" anahtarı var diye kabul edilir; safeMessage/recoveryAction eksikse ErrorBanner çökebilir.
Kullanıcıya etkisi: Backend dışı bir hata nesnesi UI'da ham olarak işlenebilir; teknik alanlar görünebilir.
Veri doğruluğu etkisi: Yok (yalnız görüntü).
Önerilen çözüm: Minimal runtime validator (code string + safeMessage string + recoveryAction?) ile normalize; uymayanı UNKNOWN_ERROR'a düşür.
En küçük güvenli çözüm: `isAppError` guard fonksiyonu + test.
Migration: Hayır. Kapsam: S. Sıra: Tur 4.
```

### P1-12 — Job rehydration hatası sessizce yutuluyor

```text
ID: TD-14
Öncelik: P1
Başlık: rehydrate_jobs sonucu `let _ =` ile yok sayılıyor; startup rehydration yok
Durum: CONFIRMED (önceki rapor #10 ile aynı)
Kaynak: src-tauri/src/commands/job_commands.rs:52; src-tauri/src/jobs/job_manager.rs:593
Somut kod kanıtı: `let _ = state.job_manager.rehydrate_jobs(...)` — hata durumunda diskteki job'lar yüklenmez; list_jobs boş döner; önceki process'in Running job'ları Interrupted'a alınmadan "kaybolmuş" görünür. Startup'ta ayrı rehydrate çağrısı yok.
Kullanıcıya etkisi: Yeniden başlatmada iptal edilemeyen veya durumu belirsiz job görünebilir.
Veri doğruluğu etkisi: Job/progress yanlış gösterimi.
Önerilen çözüm: rehydrate hata yolunu typed AppError olarak yay veya görünür warning'e çevir; startup'ta tek rehydrate noktası tanımla.
En küçük güvenli çözüm: hata durumunda diagnostic + teacher-safe mesaj.
Migration: Hayır. Kapsam: S. Sıra: Tur 4.
```

### P1-13 — Production unwrap() ve yutulan commit hataları

```text
ID: TD-15
Öncelik: P1
Başlık: job_manager Mutex unwrap'ları ve speaking_exam_service'te yutulan commit_snapshot_cas hataları
Durum: PARTIAL (önceki rapor #13; sayı azaldı ama kritik noktalar kaldı)
Kaynak: src-tauri/src/jobs/job_manager.rs:104,139,146,291 (Mutex::lock().unwrap()); src-tauri/src/services/speaking_exam_service.rs:1156,1941,2467 (let _ = commit_snapshot_cas)
Somut kod kanıtı: job_manager satırları lock poison'da panic riski taşır; speaking_exam_service satırları hatasız başarısız commit sonrası memory'de değişmiş kaydı bırakır (kalıcılık kaybı sessiz).
Kullanıcıya etkisi: Nadir panic → job sistemi çöker; commit hatası → öğretmen değişikliği kaybolur.
Veri doğruluğu etkisi: Commit hatası durumunda sessiz kayıp.
Neden teknik borç: AGENTS "typed errors; no unwrap" kuralı ihlali.
Önerilen çözüm: lock'ları map_err ile typed hata; commit sonuçlarını en azından loglayıp audit'e işle, hayati yollarda yay.
En küçük güvenli çözüm: job_manager lock'larına `unwrap_or_else(|p| p.into_inner())` yerine map_err; commit'lere error logging.
Migration: Hayır. Test: lock-poison birim testi + commit-fail regresyonu. Kapsam: S-M. Sıra: Tur 4.
```

### P1-14 — OCR duplicate canonical/read model (flat list canonical; helper ölü kod)

```text
ID: TD-16
Öncelik: P1
Başlık: student_answer_ocr_records (flat) canonical, student_answer_ocr_generations history; resolved_active_ocr_records hiç kullanılmıyor
Durum: CONFIRMED (önceki rapor #6)
Kaynak: src-tauri/src/domain/project.rs:48-52, 139-151; src-tauri/src/services/student_answer_ocr_service.rs:1314-1319, 1719-1727; src-tauri/src/services/scoring_service.rs:519; src-tauri/src/services/workflow_engine.rs:106-119
Somut kod kanıtı: flat kayıtlar hem writer hem reader tarafında kullanılıyor; `resolved_active_ocr_records()` tanımlı ama hiç çağrılmıyor (dead code).
Kullanıcıya etkisi: Şu an tutarlı; ancak iki kaynak bakım yükü ve gelecekte divergans riski.
Veri doğruluğu etkisi: Aktif projection ile generation history arasında senkron işlemlerde (accept/reject) tutarlılık riski.
Neden teknik borç: İki modelin ikisi de persistence'te; tek canonical owner yok.
Önerilen çözüm: flat listeyi salt read projection olarak işaretle ve resolved_active_ocr_records kullan; yazımları tek yere topla.
En küçük güvenli çözüm: okuyucuları resolved_active_ocr_records üzerinden geçirmek.
Migration: Hayır. Kapsam: M. Sıra: Tur 4.
```

### P1-15 — Teknik veri öğretmen arayüzüne sızıyor

```text
ID: TD-17
Öncelik: P1
Başlık: Raw workflow kodu/JSON/UUID/internal enum öğretmen ekranlarında görünebiliyor
Durum: CONFIRMED (önceki rapor #11; performans tarafında iki örnek ek)
Kaynak: src/components/workflow/BlockingReasons.tsx:11 (`blockingReasonLabels[r] || r` — etiket yoksa raw kod); src/pages/PerformanceScoringPage.tsx:492-493 (Missing/NotPerformed İngilizce enum adları parantez içinde), 966-968 (raw report.teacherId UUID)
Somut kod kanıtı: `{blockingReasonLabels[r] || r}` ham engel kodunu gösterir; `{report.teacherId ? report.teacherId : 'Belirtilmedi'}` UUID gösterir.
Kullanıcıya etkisi: Teknik jargon/UUID öğretmende kafa karışıklığı; iç yapı ifşası.
Veri doğruluğu etkisi: Yok (görsel).
Önerilen çözüm: etiket düşmeyen her kod için fallback etiket; teacherId yerine öğretmen adı/anonim etiket; enum adları yerine Türkçe etiket.
En küçük güvenli çözüm: BlockingReasons fallback'i Türkçe genel mesaja çevir; teacherId'yi gizle.
Migration: Hayır. Kapsam: S. Sıra: Tur 1/Tur 2.
```

### P1-16 — Frontend job polling/event çoğalması

```text
ID: TD-18
Öncelik: P1
Başlık: Aynı job bilgisi için birden çok polling/listener; merkezi job store kullanımı kısmen
Durum: PARTIAL (önceki rapor #12; AppLayout global işlem merkezi var, sayfa bazlı polling hâlâ)
Kaynak: src/app/AppLayout.tsx:192 (refetchInterval), src/pages/ScoringPage.tsx:89 (refetchInterval)
Somut kod kanıtı: iki ayrı TanStack Query refetchInterval'ı job snapshot'ları için çalışıyor.
Kullanıcıya etkisi: Gereksiz IPC/event trafiği; performans (puanlama sırasında) ve pil maliyeti.
Veri doğruluğu etkisi: Yok.
Önerilen çözüm: Job snapshot'ları tek merkezi store/query'e toplamak; sayfalar event abonesi olsun.
En küçük güvenli çözüm: sayfa bazlı poller'ı global job query'ye bağlamak.
Migration: Hayır. Kapsam: M. Sıra: Tur 4.
```

### P1-17 — Extraction her hedef soru için tüm sayfa görsellerini yeniden gönderiyor

```text
ID: TD-19
Öncelik: P1
Başlık: question_text_service ve rubric_extraction_service her target soru isteğinde tüm hazırlanmış sayfa inputlarını clone'layıp gönderiyor
Durum: CONFIRMED (önceki rapor #21)
Kaynak: src-tauri/src/services/question_text_service.rs:752; src-tauri/src/services/rubric_extraction_service.rs:606; llama_server_gateway.rs:1781-1822
Somut kod kanıtı: `model_input_images: all_prepared_inputs.clone()` — O(soru × tüm sayfalar) maliyet; gateway tüm görselleri base64 "high" detail ile user mesajına koyar.
Kullanıcıya etkisi: Extraction yavaşlığı ve bellek; uzun sınavlarda time-out riski.
Veri doğruluğu etkisi: Yok.
Neden teknik borç: Sayfa hedefleme/pencereleme yok (UYGULAMA_PLANI Faz 2.10 öngörüyordu).
Önerilen çözüm: question→page eşlemesi ve sınırlı pencere; fallback genişlemesi.
En küçük güvenli çözüm: Tek sayfa hedefi + düşük confidence'ta ±1 pencere.
Migration: Hayır. Kapsam: M. Sıra: Tur 5.
```

### P1-18 — Rubrik parse retry'i tam multimodal isteği yeniden gönderiyor

```text
ID: TD-20
Öncelik: P1
Başlık: Rubrik extraction retry'i görseller dahil tüm isteği yeniden gönderiyor; deterministic/text-only repair yok
Durum: CONFIRMED (önceki rapor #28)
Kaynak: src-tauri/src/services/rubric_extraction_service.rs:1002-1039
Somut kod kanıtı: `retry_request = request.clone()` (image'lar dahil), yalnız `strictJsonOnly=true`, `attempt=2`.
Kullanıcıya etkisi: Parse hatasında 2x görsel maliyet ve gecikme.
Veri doğruluğu etkisi: Yok.
Önerilen çözüm: grammar/schema-only retry veya text-only salvage; aynı görsellerle ikinci çağrıyı yalnızca bilinçli kararla.
En küçük güvenli çözüm: ilk yanıtın kısmi JSON'ını deterministik olarak kurtarma (mevcut parse_partial) ile sınırlı ikinci çağrı.
Migration: Hayır. Kapsam: M. Sıra: Tur 5.
```

### P1-19 — OCR görüntü zincirinde deskew/perspective/registration ve OCR özel yüksek DPI yok

```text
ID: TD-21
Öncelik: P1
Başlık: OCR crop'ları UI preview artifact'ından (~144-150 DPI) düz dikdörtgen crop; registration transformu yok
Durum: CONFIRMED (önceki rapor #22 ve UYGULAMA_PLANI Faz 2.1/2.2)
Kaynak: src-tauri/src/services/student_answer_crop_service.rs:490-501; src-tauri/src/services/pdf_service.rs:67-79 (scale 2.0 ≈144 DPI), 230-238 (pdftoppm varsayılan 150); model_input_image_service.rs:734-736 (alignment_transform = identity_v1)
Somut kod kanıtı: crop_preview_image `image.crop_imm(x,y,w,h)` doğrudan; render DPI provenance'ı `render_dpi: None / render_dpi_unknown_for_existing_preview_artifact` (student_answer_ocr_service.rs:2562,2583).
Kullanıcıya etkisi: Eğik/basık taramalarda OCR hata oranı yüksek; kalite ancak benchmark ile ölçülebilir.
Veri doğruluğu etkisi: Dolaylı (OCR doğruluğu → puan).
Neden teknik borç: Pipeline "render→crop" ekseni düzleştirilmemiş.
Önerilen çözüm: OCR'a özel yüksek DPI render + page registration/deskew + provenance; benchmark kapısı ile default seç.
En küçük güvenli çözüm: Önce golden set + baseline ölçümü (UYGULAMA_PLANI Faz 2 kuralı).
Migration: Hayır. Kapsam: L. Sıra: Tur 6.
```

### P1-20 — Beş preprocess varyantı cache hit'te dahi üretiliyor

```text
ID: TD-22
Öncelik: P1
Başlık: OCR her kaynak görsel için 5 preprocess varyantının tamamını üretiyor; adaptive/selective yok
Durum: CONFIRMED (önceki rapor #17, UYGULAMA_PLANI 2.5)
Kaynak: src-tauri/src/services/student_answer_ocr_service.rs:45-51, 2632-2656; ocr_image_preprocess_service.rs:101-136
Somut kod kanıtı: `for variant in PREPROCESS_VARIANTS { ... }` — hepsi eager üretilir, sonra biri seçilir.
Kullanıcıya etkisi: İlk çalıştırmada 5x görsel işleme maliyeti; uzun sınavlarda zaman.
Veri doğruluğu etkisi: Yok.
Önerilen çözüm: Image quality profili → tek başlangıç varyantı; ikinci-pass trigerlarında ek varyant.
En küçük güvenli çözüm: Policy ile tek varyant seçimi; alternatifler yalnız ihtiyaç halinde.
Migration: Hayır. Kapsam: M. Sıra: Tur 6.
```

### P1-21 — Count-only performance sayfaları için frontend test kapsamı sıfır

```text
ID: TD-23
Öncelik: P1
Başlık: Performans özelliğinin saf helper'ları ve save/approve/publish akışları için hiçbir otomatik test yok
Durum: CONFIRMED
Kaynak: glob `**/*.test.ts*` (27 dosya) içinde performance testi yok; performanceOrganizationUi.ts (344 satır) ve performanceReportUi.ts (92 satır) test dışı; examWorkspace.test.ts/assessmentMode.test.ts performance kapsamı yok
Kullanıcıya etkisi: Rubrik doğrulama semantiği, CSV escape, toplam hesabı, onay akışı regresyonlarında yalnız manuel güven.
Veri doğruluğu etkisi: Puan/onay akışında kod değişikliği sonrası sessiz hata riski artar.
Neden teknik borç: Faz B/C'de "test dayatması yok" politikası; bu özellik puan üreten bir akış olduğu için test zorunlu olmalı.
Önerilen çözüm: performans helper'ları + component + komut kontrat testleri (node --test deseni).
En küçük güvenli çözüm: performanceOrganizationUi/performanceReportUi saf fonksiyon testleri + backend kontrat testleri.
Migration: Hayır. Kapsam: M. Sıra: Tur 0 (test tabanı).
```

---

## 7. P2 bulgular

### P2-1 — Monolitik DTO ve ham Project snapshot

```text
ID: TD-24
Öncelik: P2
Başlık: get_project_snapshot ham Project döndürüyor; src/api/types.ts 2180 satır monolit
Durum: CONFIRMED (önceki rapor #15/#16; types.ts 2180 satıra büyümüş)
Kaynak: src-tauri/src/commands/project_commands.rs:129-135 (Result<Project,...>); src/api/types.ts (2180 satır)
Etki: Frontend persisted domain'e bağımlı; Rust-TS contract otomatik üretilmiyor; her domain değişikliği elle TS eşleme gerektirir → drift riski. Performans tipleri bu dosyada ~150 satır eklemiştir.
Önerilen çözüm: Read-model DTO + otomatik schema (örn. ts-rs) veya manuel kontrat testi. Kapsam: L. Sıra: Tur 8.
```

### P2-2 — Correlation ID zinciri kırık

```text
ID: TD-25
Öncelik: P2
Başlık: command→job→model→mutation→audit zincirinde tek correlation id yok; her katmanda yeniden üretiliyor
Durum: CONFIRMED (önceki rapor #17; kısmen iyileşmiş — scoring run_id lease'e geçiyor)
Kaynak: src-tauri/src/domain/model.rs:284-296 (ModelInvocationContract'te correlation_id YOK), src-tauri/src/services/project_store.rs:90-99 (MutationOptions.correlation_id her çağrıda yeni), src-tauri/src/services/audit_service.rs:853-886 (AuditEntryInput yeni UUID), scoring_service.rs:377-424 (audit'e correlation chain yok)
Etki: Hata izleme/provenance zayıf; performans publish/evaluate/approve/export işlemlerinde audit olayını command'le ilişkilendirmek zor.
Önerilen çözüm: MutationOptions + AuditEntryInput + ModelInvocationContract'a correlation alanı; komut katmanından tek id akıtma. Kapsam: M. Sıra: Tur 4.
```

### P2-3 — Büyük servisler ve AppState bağımlılıkları

```text
ID: TD-26
Öncelik: P2
Başlık: diagnostics.rs 5831, llama_server_gateway.rs 5447, project_store.rs 5140, speaking_exam_service.rs 5139, performance_service.rs 1862 satır; AppState 29 alan (23 Arc)
Durum: CONFIRMED (önceki rapor #14; sayılar değişmiş)
Kaynak: wc -l sonuçları; src-tauri/src/lib.rs:41-71
Etki: Sorumluluk yoğunluğu, test izolasyonu zorluğu, değişiklik yayılımı. performance_service tek dosyada ~10 sorumluluk (CRUD, sürümleme, değerlendirme, onay, rapor) barındırıyor.
Önerilen çözüm: Modül ayrımı (rubric_versioning, assessment_evaluation, report), AppState'te facade. Kapsam: L. Sıra: Tur 8.
```

### P2-4 — Legacy prompt fallback hâlâ mevcut (dormant)

```text
ID: TD-27
Öncelik: P2
Başlık: gateway, PromptContract yoksa legacy_prompt_contract_with_data fallback'ine düşüyor
Durum: PARTIAL (önceki rapor #19; tüm üretim çağrıları Some geçiyor — dormant)
Kaynak: src-tauri/src/services/llama_server_gateway.rs:64-81; src-tauri/src/services/prompt_contract.rs:65-92
Etki: Yeni çağrı sitesi `None` geçerse versionless/schema'sız legacy prompt kullanılabilir; fail-closed değil.
Önerilen çözüm: `request_contract`'te None'ı typed hata olarak reddetmek. Kapsam: S. Sıra: Tur 4.
```

### P2-5 — Scoring fingerprint calibration/anchor "none" placeholder

```text
ID: TD-28
Öncelik: P2
Başlık: ScoringFingerprint kalibrasyon ve anchor sürümlerini sabit "none" olarak yazıyor
Durum: CONFIRMED (önceki rapor #18/23 ile uyumlu; temel hash tam)
Kaynak: src-tauri/src/services/scoring_service.rs:1335 (default_sampling 2048), 1339 (calibration="none", anchor="none")
Etki: Aynı paket/prompt/model ile farklı kalibrasyon/anchor politikası cache/fingerprint'i değiştirmez → cache geçersiz kullanılabilir.
Önerilen çözüm: calibration/anchor gerçek sürümü hash'e dahil et. Kapsam: S. Sıra: Tur 7.
```

### P2-6 — Job polling/event çoğaltması ve merkezi job store (P1-16'nın yapısal uzantısı)

```text
ID: TD-29
Öncelik: P2
Başlık: Job snapshot'larda merkezi veri kaynağı kısmen; sayfa ve global poller ayrık
Durum: PARTIAL (önceki rapor #12)
Kaynak: src/app/AppLayout.tsx:192, src/pages/ScoringPage.tsx:89
Önerilen çözüm: tek job query/queryClient; event tabanlı güncelleme. Kapsam: M. Sıra: Tur 4.
```

### P2-7 — Analysis artık yapılandırılmış (önceki bulgu düzeltilmiş) — performans analysis'e bağlı değil

```text
ID: TD-30
Öncelik: P2
Başlık: Analysis metrics[]/claims[] + metricRefs yapılandırılmış; fakat performans sonuçları analysis servisine bağlı değil (kendi raporu var)
Durum: ALREADY_FIXED (structured); PARTIAL (performans entegrasyonu yok)
Kaynak: src-tauri/src/domain/analysis.rs:129-159; analysis_service.rs:609, 743-753; assessment_kind yalnız Written|Speaking (analysis.rs:5-8)
Etki: Önceki "serbest metin analysis" bulgusu çözülmüş. Performans verisi istatistiksel analiz/raporlama ortak hattında yer almıyor — bilinçli bir karar mı yoksa eksik mi netleştirilmeli.
Önerilen çözüm: Performans aggregation'unun ayrı ama aynı desende olması; ileride ortak metric registry'ye ekleme. Kapsam: M. Sıra: Tur 8 (isteğe bağlı).
```

### P2-8 — Rubrik şablonları yalnız frontend'de; backend kontratına publish anında uyar

```text
ID: TD-31
Öncelik: P2
Başlık: TDE şablon kataloğu frontend'de salt-okunur; backend'de şablon yok
Durum: CONFIRMED
Kaynak: src/pages/performanceOrganizationUi.ts:68-173 (4 şablon × 4 ölçüt), :175-199 (template→rubric kopyası, yeni ID'ler, 5 düzey)
Etki: Şablon kopyası bağımsızdır (güvenli); backend doğrulaması publish'te çalışır. Şablon değişikliği frontend dağıtımı ister; zümre düzenlemesi rubrik taslağı üzerinden yapılır — mimari olarak kabul edilebilir.
Önerilen çözüm: İleride şablonları backend servis kataloğuna taşımak. Kapsam: S. Sıra: Ertelenebilir.
```

### P2-9 — Golden set / benchmark altyapısı yok

```text
ID: TD-32
Öncelik: P2
Başlık: OCR/scoring/model için committed golden set, CER/WER, latency, RAM, DPI, token budget benchmark kapısı yok
Durum: CONFIRMED (önceki rapor #25)
Kaynak: repoda benchmark manifesti/fixture bulunamadı (UYGULAMA_PLANI Faz 2 zorunlu kılar)
Etki: Kalite regresyonu ölçülemez; P3 iyileştirmeleri (KV cache, thread, MTP, HTR) savunulamaz.
Önerilen çözüm: anonim golden set + baseline manifesti + CER/WER/kritik terim/latency/RAM kapısı. Kapsam: XL. Sıra: Tur 0/6 (benchmark zorunlu fazların ön koşulu).
```

### P2-10 — Frontend integration testleri sınırlı

```text
ID: TD-33
Öncelik: P2
Başlık: Testler ağırlıkla helper/view-model; gerçek page render, command mock kontratı, backend DTO parse, failed save, stale response kapsamı dar
Durum: CONFIRMED (önceki rapor #24)
Kaynak: npm test 147 test dosya envanteri; performans sayfaları hiç kapsanmıyor (TD-23)
Önerilen çözüm: component + kontrat testleri; performans için Tur 0'da taban. Kapsam: L. Sıra: Tur 0.
```

---

## 8. P3 bulgular (performans, kalite, ileri geliştirme)

```text
ID: TD-34 — P3 — Eager 5 preprocess varyantı maliyeti: cache hit'te dahi tüm varyantların disk/CPU işlemi yapılıyor (cache hit dosya kontrolüyle sınırlı olsa da üretim eager). En küçük çözüm: varyant seçimini policy'ye almak (TD-22 ile bağlantılı).
ID: TD-35 — P3 — Model/runtime tuning kanıtı yok: KV cache q8_0, thread/batch, MTP, speculative decoding, HTR karşılaştırmaları benchmark'sız yapılamaz (UYGULAMA_PLANI 33-36 "doğrulanamadı"). Golden set (TD-32) tamamlanmadan başlanmaz.
ID: TD-36 — P3 — Speaking score retry backoff: speaking_exam_service.rs:2276-2281 sabit 2s sleep — küçük ama parametrik olmalı.
ID: TD-37 — P3 — deterministik scoring kapsamı: deterministic_scoring_service 8 answer type destekliyor; SentenceAnnotation/GrammarAnalysis/Essay model yolunda — ileride deterministik kaplama genişletilebilir (Tur 7).
ID: TD-38 — P3 — Rapor üretimi her çağrıda tüm roaster'ı tarar (O(students × criteria)); sınıf büyüdükçe gecikme; cache/özelleştirme gerekebilir.
ID: TD-39 — P3 — performans raporu PDF'i window.print(); sunucu tarafı PDF (pdf_service) altyapısı mevcut ama kullanılmıyor — tek format ve pagination tutarlılığı için ileride.
```

---

## 9. TYMM performans özelliği genel değerlendirmesi

Performans özelliği, plan (`docs/TYMM_PERFORMANCE_PLAN.md`) ile kod arasında **yüksek uyum** gösterir:

- **Kapsam:** Görev `AssessmentActivity` (`performance_details` + `rubric_versions`), değerlendirmeler `ClassApplication.performance_assessments`, sınıf üyeliği `SchoolClassService` roster'ından doğrulanıyor. Yazılı `ScoringRecord`/QEP/OCR akışına dokunmuyor. `AssessmentType::Performance` için tüm Rust match'leri exhaustive (derleme geçiyor).
- **Rubrik sürümleme (K8):** publish = yeni sürüm; onaylı değerlendirmesi olan rubrik yeni sürüm bile yayınlanamıyor; kayıt `rubric_id + rubric_version` sabitliyor; rapor kayıt sürümüyle çözüyor; silme komutu yok (TD-06 silme koruması sınıf uygulaması silmede eksik).
- **Eksik semantiği (K9):** Missing/NotPerformed ≠ 0 tüm katmanlarda (Rust enum, TS union, form, CSV/PDF). Sıfır puan gerçek 0 düzey seçimiyle mümkün; Missing'te puan/not silinir, toplam None.
- **Öğretmen onayı:** onay tüm ölçütleri zorunlu kılar, onaylanmış kayıt düzenlenemez, `approved_at` + rubrik sürümü sabitlenir. AI puan üretmiyor (hiçbir model çağrısı yok).
- **Command katmanı:** 11 komut, typed input/output, backend doğrulama, `AppError`; 6'sı kritik olayda audit yazıyor (create/update/publish/approve/status). `save` (taslak) ve `get_report` audit'siz — kabul edilebilir.
- **Migration:** `normalize_assessment_organization` additive/idempotent; `MigrateWithVerifiedBackup` backup-gated; `InspectReadOnly` hiç yazmıyor.

**Ana zayıflıklar:** P0-2 (onaylı kayıt silinebilir), P1-2 (taslak re-pin), P1-3 (duplicate kayıt), P1-5 (rapor geçici toplam), P1-7 (frontend yarışı), TD-03 (frontend-derived readiness), TD-23 (test yok).

---

## 10. Performance assessment scope

Kanıtlanan model:

| Veri | Saklandığı yer | Assessment ID | ClassApplication ID | Project-level alan | Karışma riski |
|---|---|---|---|---|---|
| Görev metadata | `AssessmentActivity` (assessment.rs:133-161) | activity.id (kendi) | — | — | Yok |
| Rubrik sürümleri | `AssessmentActivity.performance_details.rubric_versions` | activity.id | — | — | Yok |
| Öğrenci değerlendirmesi | `ClassApplication.performance_assessments` (assessment.rs:123) | application.activity_id | application.id | — | Yok (TD-05 duplicate riski hariç) |
| Student | `Project.students` (roster) | — | — | evet (ortak) | Yok (ortak kimlik) |

**Aynı proje içinde iki performans görevi:** teklik anahtarı `academicYearId + courseId + gradeLevel + term + assessmentType + sequenceNumber` (`is_same_key`, assessment.rs:181-188) ayrı kayıtlar üretir; `list_performance_tasks` yalnız `Performance` filtreler; değerlendirmeler application altında. Veriler karışmıyor. ✓

**Yazılı + performans birlikte:** Ayrı `assessment_type`; yazılı veriler proje seviyesinde, performans activity seviyesinde. Performans kendi koleksiyonlarında tutulduğundan yazılı ile karışmıyor; yazılı-yazılı karışması TD-01'dir. ✓ (performans açısından)

**Birden fazla sınıf uygulaması aynı görevi kullanabiliyor:** `create_performance_task` her seçilen sınıf için ayrı `ClassApplication` oluşturur; her application kendi `performance_assessments`'ini taşır. ✓

**Scope kimlikleri:** `assessment_activity_id` (implicit — activity_id), `class_application_id`, `performance_task_id` (= activity_id), `student_id`, `rubric_version` — kayıtta açıkça var (performance.rs:110-128). ✓

**Fixture doğrulaması (zihinsel + test):**
- aynı proje → yazılı A → yazılı B: TD-01 nedeniyle sorunlu (soru/OCR/puan proje seviyesinde karışır). ❌
- → performans A → performans B: ayrı activity; B'nin create'inde teklik anahtarı farklıysa geçer; değerlendirmeler ayrı. ✓
- iki farklı sınıf → aynı öğrenci numarası: öğrenci ID'si ile scope'lu; student_id tektir; aynı numaralı farklı sınıf öğrencileri farklı student_id'ye sahiptir. ✓ (student identity UUID bazlı; numara çakışması ID çakışması değildir)

Önceki P0 "AssessmentActivity yalnız metadata; sorular/rubrik/OCR/scoring proje seviyesinde":
- **Yazılı taraf:** hâlâ geçerli → **TD-01 (P0)**.
- **Performans tarafı:** geçerli değil — rubrik/değerlendirme activity+application scope'lu. ✓
- **Ortak proje domain'i (student, school_class, teaching_assignment):** ortak olması tasarımdır; sorun değil.

---

## 11. Performance rubrik sürümleme değişmezleri

| İnvariant | Durum | Kanıt |
|---|---|---|
| Taslak rubrik değerlendirmede kullanılamaz | ✓ CONFIRMED | save `latest_published_rubric` ister (performance_service.rs:524-530); sürüm 0 reddedilir |
| Yayımlanmış rubrik immutable | ✓ (kayıt kopyası) | her publish yeni `PerformanceRubric` push'lar; var olan değişmez (satır 461-469) |
| Yayımlanmış rubrik düzenlenecekse yeni sürüm | ✓ | publish max+1 (satır 454-460) |
| Eski değerlendirme tam olarak kullandığı rubrik sürümünü taşır | ⚠️ PARTIAL | ilk kayıtta sabitlenir; fakat sonraki save'de en yeni sürüme re-pin edilir (TD-04) |
| Yeni rubrik sürümü eski değerlendirmeyi sessizce değiştirmez | ⚠️ PARTIAL | onaylı kayıtlar korunur (publish kilidi); InProgress taslaklar re-pin ile değişebilir (TD-04) |
| Onaylanmış karar sonradan yeniden hesaplanmaz | ✓ | onaylı kayıt düzenlenemez; publish kilitli |
| Kullanılan rubrik sürümü silinemez | ✓ | rubrik silme komutu yok |
| Rubrik sürümü yalnız frontend kilidiyle korunmuyor | ✓ | backend publish kilidi var (satır 438-453) |
| Aynı anda iki publish duplicate üretmiyor | ✓ | publish `mutate` içinde max+1; proje lock seri hale getirir; ancak çift tıklama iki ayrı sürüm üretir (idempotency yok — P1-22'ye bak) |
| Draft 0 / published >=1 migration+reload sonrası korunuyor | ✓ | normalize additive; version korunur (serialization_roundtrip testi) |
| Ölçüt/düzey ID'leri sürümler arasında güvenli | ⚠️ | ID'ler frontend'de `${template.id}-c{n}` / `level-{n}`; sürümler arası stabil ancak düzenleme ID'yi değiştirebilir → kayıt re-pin'inde validate_ratings eski ID'leri reddeder (güvenli davranır, kayıp değil) |
| Şablondan oluşturulan rubrik bağımsız kopya | ✓ | `performanceTemplateToRubric` yeni ID'lerle derin kopya (performanceOrganizationUi.ts:175-199) |

**Ek bulgular:**
- Publish, `input.rubric.version` alanını yok sayar (satır 461-468) — güvenli.
- Publish sonrası draft (sürüm 0) history'de kalır; yeni sürümlerle aynı listede. ✓
- İki publish duplicate sürüm üretmez ama **çift submit iki ayrı sürüm** üretir (P1-22, idempotency yok).

---

## 12. Missing / NotPerformed / zero semantiği

Katman bazında tarama:

| Katman | Davranış | Kanıt |
|---|---|---|
| Rust enum | `InProgress/Approved/NotPerformed/Missing` ayrı (performance.rs:86-94); serde default `InProgress` | performance.rs:88-90 |
| serde default | `ratings=[]`, `provisional_total=0` (performance.rs:115-118) — durum ayrı tutulur | performance.rs:115-122 |
| Migration | performans alanları normalize additive; Missing kayıt korunur | project_store.rs:1796-1801 |
| Command DTO | status yalnız `missing|not_performed` (SetPerformanceAssessmentStatusInput) | performance_service.rs:108-118; :728-737 reddi |
| TS tipleri | `'in_progress'|'approved'|'not_performed'|'missing'` | types.ts:1217 |
| Form state | Missing/NotPerformed butonları ayrı; `—` gösterimi; sıfır puan gerçek 0 düzey ile | PerformanceScoringPage.tsx:434-441, 478-493 |
| save/reload | Missing kayıt re-save ile InProgress'e dönebilir (bilinçli revert); onaylı ise red | performance_service.rs:568-592, 577 |
| Geçici toplam | `provisional_total` yalnız servis hesaplar (satır 553) | performance_service.rs:1157-1168 |
| Final toplam | Onaylı kayıt `approved_at` ile final; rapor InProgress toplamını da basar (TD-07) | performance_service.rs:1055-1063 |
| Öğrenci durumu | Rapor status alanı; Missing/NotPerformed `total: null` | performance_service.rs:1053-1063 |
| CSV | boş hücre (TD-08 escape riski hariç) | performanceReportUi.ts:61-64 |
| PDF | `—` gösterimi | PerformanceScoringPage.tsx:1008-1012 |
| Backup/restore | JSON bazlı — durum değerleri korunur | ProjectStore atomic write |
| Eski proje açılışı | performans yoksa boş koleksiyon; `assessmentType` değerleri değişmez | project_store normalize |

Soru bazında özet:

- **Missing toplamın paydasını nasıl etkiliyor?** Rapor toplamları öğrenci başına satır bazında `total: None` — paydaya puan katmaz; `student_count` bütün öğrencileri sayar, `missing_count` ayrı. Toplam dışına çıkar (payda katılmaz).
- **NotPerformed aynı:** `total: None`, `not_performed_count` ayrı.
- **Gerçek 0 puan:** Düzey puanı 0 seçilirse `total: Some(0)` — Missing'ten farklı görünür (rapor `0` gösterir). UI "En az bir düzey seçin" yalnız hiç seçim yoksa bloklar; yalnız 0 puanlı düzey seçimi geçer (PerformanceScoringPage.tsx:617).
- **Geçici/final ayrımı:** Kayıtta yalnız `provisional_total` var; `status=Approved` final olarak yorumlanır. DTO'da ayrı `provisionalTotal`/`finalTotal` yok → TD-07.
- **Eksik öğrenci finalleştirilebilir mi?** `approve_performance_assessment` yalnız tüm ölçütler değerlendirilmiş kaydı onaylar; Missing/NotPerformed kayıtların ratings'ı boş olduğundan onay reddedilir (pinned.criteria ⊄ rated). ✓
- **NotPerformed öğretmen kararı mı?** Öğretmenin tıklamasıyla `set_performance_assessment_status('not_performed')` yazılır; sistem varsayılanı değildir. ✓

**Yasak desen taraması** (`unwrap_or_default`, `unwrap_or(0)`, `Option<number>→0`, `null→0`, `undefined→0`, boş string→0, Missing→0, NotPerformed→0, NaN→0):
- Backend'de performans toplamı için bu desenlerden hiçbiri kullanılmıyor; `provisional_total` servis tarafından rating toplamı (u32) ile hesaplanır; Missing'te `0` yazılır ama `status` ayırt edicidir ve rapor `total: None` üretir.
- Frontend `performanceProvisionalTotal` (performanceOrganizationUi.ts:328-336) `level ? sum + points : sum` — bilinmeyen level yoksayılır (0 değil); gösterimde `—`/null kullanılır.
- Sıfırla karışma: rapor/tablo/CSV/PDF'te ayrı gösterim kanıtlandı. ✓

---

## 13. Öğretmen kararı ve finalite

| Soru | Durum | Kanıt |
|---|---|---|
| AI/model doğrudan puan verebiliyor mu? | Hayır | Performans akışında hiçbir model çağrısı yok; puan yalnız öğretmenin düzey seçimi + servis toplamı |
| Öğretmen onayı final karar için backend'de zorunlu mu? | Evet (final raporu açısından kısmen) | onay `approved_at` + durum Approved; onay sonrası düzenleme reddi. Ancak rapor InProgress toplamlarını da gösterir (TD-07) |
| Frontend onay görünümü canonical state'e dayanıyor mu? | Evet | `isApproved` `assessment.status==='approved'`'den; backend state kaynağı |
| Geçici/final ayrımı? | Kısmen | kayıtta `status`; DTO'da tek `total` (TD-07) |
| Onaylanan karar düzenlenirse invalidation/reapproval? | Yok | Onaylanmış kayıt düzenlenemez; "yeni değerlendirme aç" akışı **yok** (planın vaat ettiği "yeni değerlendirme açılabilir" implemente edilmemiş) — onaylı karar değiştirilemez; düzeltme ancak durum sıfırlamasıyla (P0-2 riski) |
| Final kararın hangi rubrik sürümüne göre verildiği kayıtlı mı? | Evet | `rubric_id + rubric_version` onay anında kayıtta; rapor bu sürümle çözer |
| Onaylayan öğretmen/actor bilgisi? | Hayır | `approved_at` var; actor yok. Audit zinciri app seviyesinde ama öğretmen kimliği içermiyor; `teacher_id` yalnız report'ta teaching_assignment'tan türetilir |
| Aynı öğrenci için iki final kaydı oluşabilir mi? | Evet (kenar) | TD-05: yabancı assessment_id ile duplicate kayıt; ikisi de onaylanabilir; rapor `.find` ilki alır |
| Concurrent save lost update? | Olası | TD-09: frontend eşzamanlı mutation; backend entity CAS yok |
| Stale frontend yeni kararı overwrite edebilir mi? | Olası | TD-09 + P0-2: stale listeyle Missing işaretleme onaylı kaydı silebilir |

**Kanıt:** approval kuralı performance_service.rs:673-709; onay sonrası save reddi :577-583; publish kilidi :438-453; rapor sürüm çözümü :995-1021.

---

## 14. Performance workflow otoritesi

Karar noktaları ve otorite:

| Karar | Otorite | Kanıt |
|---|---|---|
| Görev hazır mı (rubrik yayınlandı mı)? | Frontend `derivePerformanceStepStatuses` (hasPublishedRubric) | examWorkspace.ts:396-470 |
| Değerlendirme başlanabilir mi? | Frontend "blocked (rubrik yayınlanmadan)" + backend save gate | examWorkspace.ts:437-443; performance_service.rs:524-530 |
| Öğrenci tamamlandı mı? | Frontend sayaçları (approvedCount vs totalStudents) | examWorkspace.ts:413-422, 445-450 |
| Öğretmen onayı tamamlandı mı? | Backend (onay durumu) | performance_service.rs:673-715 |
| Rapor alınabilir mi? | Frontend (approvedCount > 0) | examWorkspace.ts:454-464 |

**Bulgu (TD-03):** performans ailesi için backend authoritative WorkflowSnapshot yok; adım durumları yalnız frontend helper'ında. Backend komutları (save/publish/approve/report) kendi korumalarını yapar, dolayısıyla "veri güvenliği" backend'dedir; fakat "hazırlık/ilerleme" gösterimi frontend'in yeniden ürettiği domain kararıdır (AGENTS.md kuralı ihlali, yazılı taraf için de TD-10).

---

## 15. Performance command ve concurrency analizi

| Command | Input | Scope IDs | Backend validation | Revision/CAS | Audit | Typed error | Idempotent | Duplicate risk |
|---|---|---|---|---|---|---|---|---|
| create_performance_task | CreatePerformanceTaskInput | project, classes | teklik anahtarı, sınıf uygunluğu, seviye | mutate (yok) | evet | evet | hayır (ayrı UUID) | teklik anahtarı çift gönderimde ikincisi reddedilir ✓ |
| update_performance_task | activity | activity | tür kontrolü | mutate | evet | evet | evet (aynı değer) | düşük |
| list_performance_tasks | filtreler | project | — | — | hayır | evet | evet | — |
| get_performance_task | activity | project+activity | tür kontrolü | — | hayır | evet | evet | — |
| publish_performance_rubric | rubric | activity | validate_rubric + onaylı kilidi | mutate | evet | evet | **hayır** | çift tıklama iki sürüm (TD-22) |
| get_performance_rubric_history | activity | — | tür kontrolü | — | hayır | evet | evet | — |
| save_performance_assessment | ratings | activity+application+student | roster, published rubric, rating ID'leri | mutate | hayır | evet | kısmen | TD-05 (yabancı ID duplicate) |
| approve_performance_assessment | assessment | activity+application | tüm ölçütler, sürüm varlığı | mutate | evet | evet | evet (ikinci onay red) | düşük |
| set_performance_assessment_status | status | activity+application+student | yalnız missing/not_performed; onay kontrolü Some dalında | mutate | evet | evet | kısmen | **P0-2** (None dalı onaylı kaydı siler) |
| list_performance_assessments | filtre | activity+application | tür kontrolü | — | hayır | evet | evet | — |
| get_performance_report | activity+application | — | tür, rubrik, sınıf | — | hayır | evet | evet | — |

Reddedilen senaryolar (kanıt):
- Yanlış assessment ID: TD-05 → duplicate (kabul edilmiyor). ❌
- Yanlış class application ID (başka activity'nin): save/approve `application` bulamaz → hata ✓; ama assessment_id çapraz doğrulanmıyor (TD-05).
- Yanlış student ID (sınıf dışı): `StudentNotFound` ✓ (save :544-551). `set_status` student'ı roster'da doğrulamıyor — öğrenci sınıftan çıkarılmışsa kayıt yine oluşur (küçük açık).
- Başka görevin rubric version ID'si: save `latest` rubriğe zorlar; validation bu rubriğe göre → yabancı level ID reddedilir ✓.
- Stale revision: entity CAS yok; ProjectStore `mutate` güncel dosyayı okur → komut düzeyinde "beklenen sürüm" yok (TD-09'la birlikte).
- Duplicate submit: create/publish için idempotency yok; publish iki sürüm üretir (P1-22).
- Onaylanmış kaydı değiştirme: save/status Some dalında red ✓; status None dalında red değil (P0-2).
- Taslak rubrikle scoring: red ✓ (latest published zorunlu).
- Silinmiş/arşivlenmiş entity: arşivli application'da save durum kontrolü yok (activity.status Archived ise performans komutları çalışmaya devam eder — küçük açık).

**P1-22 (yeni): publish idempotency yok — çift submit iki sürüm üretir.** Frontend LoadingButton ilk tick riskini sınırlıyor; backend idempotency key yok.

---

## 16. Performance migration analizi

- Eski proje schema: normalize + serde default ile açılıyor (project_store.rs:1432-1520). `assessmentActivities` performans tipi yoksa dokunulmaz. ✓
- Default alanlar veri anlamını değiştirir mi: `rubric_versions` boş → `latest_published_rubric` None → save/report `RUBRIC_MISSING` (fail-closed). ✓
- İdempotent: normalize yalnız eksik alanı ekler (changed flag ile); ikinci çalıştırma state değiştirmez. ✓
- Migration save sırasında sessiz yazma: `open_project_with_mode` MigrateWithVerifiedBackup'ta backup + atomic persist; `open_project_with_warnings`'ta migration_changed ise önce backup sonra persist (project_store.rs:422-434, 520-527). Salt açılış rewriti yok (InspectReadOnly hiç yazmaz). ✓
- Backup zorunlu: MigrateWithVerifiedBackup backup zorunlu kılar. ✓
- Bilinmeyen enum: `assessment_type` bilinmeyen değer → normalize workflow_family "written" varsayar; serde AssessmentType bilinmeyen string'de **load fail** olur (fallback yok). Bu "performans" dışındaki bilinmeyen türlerde eski projeyi açılmaz yapar (bilinçli fail-closed; risk düşük).
- Eski projeler otomatik boş performance koleksiyonuyla güvenli açılıyor: evet (assessmentActivities'ta performans yoksa hiçbir şey eklenmez; activity sayısı artmaz). ✓
- Draft/published kuralı legacy state'te: performans activity'si olmayan eski proje etkilenmez. ✓
- Rollback/failure: persist_migrated_project atomik; hata typed AppError. Partially migrated proje: normalize atomik yazım sonrası ya tam ya hiç. ✓

---

## 17. Performance delete/dependency analizi

- Performans görevi silinebilir mi? **Silme komutu yok** — görev kalıcı (bilinçli; "delete performance task" yok).
- Yayımlanmış rubrik silinebilir mi? Hayır (silme yok).
- Değerlendirmede kullanılan rubrik sürümü silinebilir mi? Hayır.
- Öğretmen onaylı değerlendirme silinebilir mi? Doğrudan silme komutu yok; fakat **P0-2** ve **TD-06** üzerinden silinebilir.
- Class application silinince performance kayıtları? **TD-06**: `remove_class_application` dependency scan'siz kayıtları gömülü olarak siler (onaylı dahil).
- Student silinince? `SchoolClassService.remove_student` performans kayıtlarını taramıyor; kayıtlar application'da kalır, rapor roster'dan türetildiği için görünmez (orphan referans; veri kaybı değil).
- Rapor geçmişi/audit dependency: rapor geçmişi yok; audit kayıtları app seviyesinde; performans silme audit'i yok (silme olmadığı için).
- Delete dependency scan backend + transaction içinde mi? Yazılı/speaking için evet; performans için **hayır** (TD-06).
- Fiziksel artefact yok ama domain geçmişi: domain geçmişi JSON içinde; P0-2/TD-06 bunu yok edebilir.

---

## 18. Performance CSV/XLSX/PDF analizi

| Kontrol | Durum | Kanıt |
|---|---|---|
| Aynı canonical veri | Evet | hepsi `get_performance_report` DTO'sundan |
| Aynı öğrenci/ölçüt sırası | Evet | roster sırası; display_rubric.criteria sırası |
| Missing/NotPerformed/0 ayrı | Evet | CSV boş hücre; PDF `—`; 0 gerçek 0 |
| Geçici sonuç final rapora girebiliyor | **Evet (sorun)** | TD-07: InProgress `total` dahil |
| Onaysız öğrenci rapora girebiliyor | Evet (satır dahil; total dahil) | TD-07 |
| Rubrik sürümü raporda | Evet | `rubricId/rubricName/rubricVersion` (DTO:198-200) + satır bazlı sürüm çözümü |
| Türkçe karakterler | CSV UTF-8 BOM; PDF print view | performanceReportUi.ts:79 |
| Formül enjeksiyonu koruması | **Hayır** | TD-08 |
| Öğrenci adı/dosya adı injection | CSV hücreleri riskli; dosya adı print (güvenli) | TD-08 |
| PDF/XLSX toplamları canonical hesapla eşleşiyor | Evet (kayıttaki provisional_total) — ancak TD-07 geçici/final ayrımı yok | — |
| Rapor yeniden üretildiğinde eski onaylı state değişmiyor | Evet | salt-okunur DTO; mutation yok |

---

## 19. Performance frontend state analizi

| Kontrol | Durum | Kanıt |
|---|---|---|
| Failed save draft'ı siliyor mu? | Hayır — draft korunur; yalnız hata gösterilir | PerformanceScoringPage.tsx:217, 205-208 |
| Backend commit öncesi success toast? | Hayır — onSuccess'te gösterilir | :209-216 |
| Duplicate click duplicate command? | Kısmen korumalı (LoadingButton isPending) | :613-658; TD-09 eşzamanlı farklı mutation'lar korumasız |
| Stale response yeni state'i overwrite? | Olası (refetch-reset effect) | TD-09 |
| Route değişiminde kaydedilmemiş veri kaybı? | Evet, uyarısız | dirty guard/useBlocker yok; AppErrorBoundary key remount (App.tsx:45) |
| Rubrik publish çift tıklama? | LoadingButton koruması + publishDisabledReason | PerformanceOrganizationPage.tsx:1522-1530, 1359-1363 |
| Öğrenci değişiminde önceki form state sızıyor mu? | Hayır (effect reset) | PerformanceScoringPage.tsx:149-162 |
| Missing/NotPerformed/0 yanlış eşleniyor mu? | Hayır | :434-441, 478-493, 892-898 |
| Error payload runtime validation? | Hayır | TD-13 (`as AppError`) |
| Raw backend kodları öğretmene? | Kısmen | TD-17 (Missing/NotPerformed İngilizce, teacherId UUID) |
| Read-only/final state yalnız UI kilidi mi? | Backend de reddeder (approve/save); UI ek kilit | performance_service.rs:577-583; PerformanceScoringPage.tsx:455-471 |
| Backend reddi doğru gösteriliyor? | Evet ErrorBanner safeMessage | ErrorBanner.tsx:16-17 |

---

## 20. Yazılı–performance–speaking izolasyonu

- `AssessmentType` varyantları exhaustive; `WorkflowFamily::Performance` ayrı (assessment.rs:16-32). `display_title` performans için ayrı etiket.
- `workflow_engine.rs`'te performans kolu yok — performans adımları backend snapshot'sız (TD-03); yazılı workflow'unu etkilemez (grep "performance" workflow_engine'de eşleşmedi).
- `finish_assessment` yalnız `written|speaking` (analysis.rs:5-8); performans kendi raporunu üretir. ✓
- `ScoringRecord`/`StudentAnswerOcrRecord`/`SpeakingAttempt` performans kayıtlarından ayrı; performans `ClassApplication.performance_assessments` içinde. ✓
- Backup/restore/preflight: JSON tabanlı; performans alanları proje dosyasında olduğundan backup/restore'a otomatik girer; preflight referans doğrulaması JSON içeriğiyle sınırlı (fiziksel artefact yok). ✓
- Audit: performans create/update/publish/approve/status audit ediliyor; save (taslak) ve report/export edilmiyor (kabul edilebilir).
- Cross-feature regresyon: `AssessmentType::Performance` eklenmesi eksik match bırakmadı (tüm cargo test + frontend build yeşil). Yazılı/speaking akışları etkilenmemiş görünüyor (mevcut testler geçiyor).

---

## 21. AssessmentActivity ve ClassApplication kapsam matrisi

| Veri türü | Canonical owner | Assessment ID | ClassApplication ID | Project-level compatibility | Karışma riski |
|---|---|---|---|---|---|
| Written soru/rubrik | `Project.questions` | hayır | hayır | evet (düz) | **Yüksek (TD-01)** |
| Written OCR | `Project.student_answer_ocr_records` (+ generations) | hayır | hayır | evet | **Yüksek (TD-01)** |
| Written scoring | `Project.scoring_records` | hayır | hayır | evet | **Yüksek (TD-01)** |
| QEP freeze | `Project.exam_package_freeze` | hayır | hayır | evet | Yüksek (tek paket) |
| Speaking attempt | `ClassApplication.speaking_attempts` | application.activity_id | application.id | evet (SpeakingExam runtime projection) | Düşük |
| Performance task/rubric | `AssessmentActivity.performance_details.rubric_versions` | activity.id | — | hayır | Yok |
| Performance assessment | `ClassApplication.performance_assessments` | application.activity_id | application.id | hayır | Düşük (TD-05) |
| Student / SchoolClass | `Project.students` / `Project.school_classes` | — | — | ortak | Ortak tasarım |

---

## 22. Workflow tek otorite analizi

Otorite kaynakları:
1. Persisted `project.workflow` (project.rs:82) — her mutation'da yeniden yazılır (project_store.rs:845).
2. Canlı `evaluate_workflow`/`evaluate_workflow_inner` (workflow_engine.rs:16-31).
3. `workflow_engine.rs:384-400` kısa devresi — `!exam_package_frozen` ve belirli stage'lerde persisted snapshot'ı aynen döndürür (canlı hesap YAPMAZ).
4. Elle `WorkflowSnapshot` yazan servisler: student_scan_service.rs:1407-1547, exam_package_build_service.rs:142-464, student_answer_crop_service.rs:852.
5. Frontend: `deriveExamStepStatuses`/`derivePerformanceStepStatuses`/`resolveNextExamStep`/`NextActions` — examWorkspace.ts:91, 396-470; NextActions.tsx.

**Sonuç:** Tek otorite yok (TD-10). Scoring gate backend'de QEP Frozen olduğundan puan doğruluğu kısmen korunur; fakat gösterim otoriteleri dağınık. Performans tarafı için TD-03.

---

## 23. OCR canonical model analizi

- Flat `student_answer_ocr_records` = yazılı/scoring/workflow için canonical (scoring_service.rs:519, workflow_engine.rs:106-119, scoring.rs:698-703).
- `student_answer_ocr_generations` = versioned history; flat'a senkron yazılır (student_answer_ocr_service.rs:1314-1319, 1719-1727).
- `resolved_active_ocr_records` (project.rs:139-151) hiç kullanılmıyor (dead code).
- Teacher accept/reject ve active pointer generation üzerinden; migration'da legacy flat'ten devam.
- **Bulgu (TD-16):** tek canonical owner yok; okuma projeksiyonu helper'ı ölü.

---

## 24. Scoring doğruluk analizi

- Final toplam: `scoring_record_is_final` (scoring.rs:757-770) TeacherApproved + !needs_review + skor + review Approved/Edited ister → `needs_review` final toplamdan **çıkarılmış** (önceki P1 çözüldü). ✓
- Provisional/accepted/final ayrımı `ScoringSummaryDto` ile (scoring.rs:358-386).
- Readiness: TD-11 (count-only) kısmi açık; `expected_records > 0` boş-list koruması var; duplicate false-ready mümkün.
- Kriter eşleşmesi canonical ID (scoring_service.rs:1524-1558; semantic_scoring_service.rs:52-66) — başlık fallback'i kaldırılmış. ✓ (önceki P1 çözüldü)
- Deterministik scoring (8 tür) + fingerprint cache (`scoring_cache_service`) model çağrısını atlayabiliyor; cached candidate final olamaz (review kalır). ✓
- Fingerprint kalibrasyon/anchor "none" placeholder (TD-28).
- Legacy default `scoring_applied=true` (TD-12) riski azalmış (load-time normalize) ama fail-closed değil.

---

## 25. Model runtime ve prompt/schema analizi

- Completion probe: hot path'ten çıkarılmış; yalnız manual `probe_model_server`/doctor/benchmark (model_process_manager.rs:164-169, 598-604; llama_server_gateway.rs:502-509). ✓ (önceki P0 çözüldü)
- Readiness/lease: tek `acquire_ready_runtime_lease`; fixed sleep yok (exponential backoff, model_process_manager.rs:769-772). ✓ (kısmen çözüldü)
- Prompt data isolation: tüm use-case'ler system/user ayrımı (prompt_contract.rs:1-6; user_data_message :94-102). ✓
- Versioning tablosu: question v2, rubric v2+schema contract, OCR v4, issue correction v2 observed-only, identity v2, scoring v4, speaking cleanup v4, evaluation v5, analysis v2 — tamamı schema+backend validation'lı. ✓
- `critical_term_hint` model request'inden çıkarılmış; deterministic post-OCR analyzer (student_answer_ocr_service.rs:2008-2040). ✓
- Local-only: `PrivacyMode::StrictLocal` default; redirect `Policy::none()`; `.no_proxy()`; loopback-only URL gate; external explicit opt-in. ✓
- Text-only speaking profili (no mmproj). ✓
- **Kalan:** rubric retry full-resend (TD-20), extraction tüm sayfa (TD-19), eager 5 preprocess (TD-22), 144-150 DPI crop/deskew yok (TD-21), KV/thread/MTP tuning benchmark'sız (TD-35).

---

## 26. Job ve error handling analizi

- Job rehydration: TD-14 (`let _ =` job_commands.rs:52; startup rehydrate yok).
- Job manager production unwrap: TD-15 (job_manager.rs:104,139,146,291).
- Yutulan commit hataları: TD-15 (speaking_exam_service.rs:1156,1941,2467).
- Cancellation: JobTaskGuard, 7 state makinesi, cancellation checkpoint'leri mevcut; correlation zinciri eksik (TD-25).
- Frontend job polling: TD-18 (AppLayout + ScoringPage ayrı poller).
- Error boundary: TD-13 (`as AppError`).

---

## 27. Backup/restore/audit/preflight kapsamı

- ProjectStore tek yazar + atomic write + storage revision + write lease + CAS (commit_job) + transaction journal: mevcut ve testli (final_data_loss_proofs 11 test geçti).
- Performance task/rubric/evaluation backup'a giriyor: evet — tümü `project.json` içinde; backup_service tam ağaç arşivler.
- Restore sonrası aynı kalıyor: evet (JSON bazlı; restore equality proof'ları geçiyor).
- Preflight performans referanslarını doğruluyor: performans fiziksel artefact üretmediği için JSON-only; preflight parse/audit/orphan kontrolü proje dosyasına bağlı.
- Audit publish/evaluate/approve/delete/export: publish/approve/status/create/update evet; save (taslak)/report/export **hayır**; delete yok. P0-2 durum silme senaryosunda audit "status updated" yazar (olay adı yanıltıcı).
- Transaction journal kritik performans mutation'ları: `project_store.mutate` journal üzerinden geçer (tüm performans mutation'ları mutate içinde). ✓
- Stale command yeni kararı overwrite edebilir mi: TD-09 (frontend yarışı) + P0-2 (None dalı).

---

## 28. Test kapsamı ve eksik proof'lar

- Backend: performans servisi 11 birim test (publish, onay, eksik semantiği, sürüm roundtrip, duplicate anahtar reddi, roster reddi). Komut kontratı 4 serde testi. Fakat: onaylı kayıt + None status reddi testi **yok** (P0-2 kanıtsız); yabancı assessment_id duplicate testi yok (TD-05); class application silme dependency testi yok (TD-06).
- Frontend: performans için **sıfır test** (TD-23).
- Proof'lar: final_security_proofs 8, final_data_loss_proofs 11, project_lock 2 — hepsi geçiyor; yeni performans için data-loss proof yok (performans verisi backup/restore eşitliği testlerinde fixture olarak yok).
- Model runtime fixture'ları bu ortamda geçti (önceki raporlardaki hang sorunu görülmedi).

---

## 29. Teknik borç matrisi

| ID | Öncelik | Alan | Başlık | Durum | Veri riski | Kullanıcı etkisi | Efor | Önkoşul | Önerilen tur |
|---|---|---|---|---|---|---|---|---|---|
| TD-01 | P0 | Assessment scope | Yazılı veriler proje seviyesinde | CONFIRMED | Yüksek | İki yazılı sınav karışır | XL (S kapatma) | — | Tur 3 |
| TD-02 | P0 | Teacher decision | set_status None dalı onaylı kaydı siler | CONFIRMED | Yüksek | Final karar kaybı | S | — | Tur 1 |
| TD-03 | P1 | Workflow | Performance adım readiness frontend'de | CONFIRMED | Orta | Yanlış hazır sinyali | M | — | Tur 2 |
| TD-04 | P1 | Rubric versioning | Taslak save'de re-pin | CONFIRMED | Orta | Sessiz puan değişimi | S-M | — | Tur 1 |
| TD-05 | P1 | Assessment scope | Yabancı assessment_id duplicate | CONFIRMED | Orta | İki kayıt/karar | S | — | Tur 1 |
| TD-06 | P1 | Delete | Class application silmede performans kaybı | CONFIRMED | Yüksek | Veri kaybı | S | — | Tur 1 |
| TD-07 | P1 | Exports | Rapor geçici toplamı final gibi | CONFIRMED | Orta | Onaysız sonuç yayılır | S-M | — | Tur 1 |
| TD-08 | P1 | Exports | CSV formül enjeksiyonu | CONFIRMED | Orta | Excel saldırısı | S | — | Tur 1 |
| TD-09 | P1 | Frontend state | Eşzamanlı mutation/draft kaybı | CONFIRMED | Orta | Lost update | S-M | — | Tur 1/2 |
| TD-10 | P1 | Workflow | Birden fazla workflow otoritesi | CONFIRMED | Orta | Stage uyuşmazlığı | M | — | Tur 2 |
| TD-11 | P1 | Scoring | Count-only readiness | CONFIRMED | Yüksek | Eksik OCR'de false-ready | M | — | Tur 2 |
| TD-12 | P1 | Scoring | Legacy scoring_applied default true | PARTIAL | Orta | Eski kayıt "uygulanmış" | S-M | — | Tur 1 |
| TD-13 | P1 | Frontend state | `as AppError` unvalidated | CONFIRMED | Düşük | Ham hata görüntüsü | S | — | Tur 4 |
| TD-14 | P1 | Jobs | rehydrate hatası yutuluyor | CONFIRMED | Orta | Job state kaybı | S | — | Tur 4 |
| TD-15 | P1 | Jobs | Production unwrap + commit yutma | PARTIAL | Orta | Panic/sessiz kayıp | S-M | — | Tur 4 |
| TD-16 | P1 | OCR | Duplicate canonical/read model | CONFIRMED | Düşük | Bakım/divergans | M | — | Tur 4 |
| TD-17 | P1 | Observability | Teknik sızıntı (kod/UUID/enum) | CONFIRMED | Düşük | Kafa karışıklığı | S | — | Tur 1/2 |
| TD-18 | P1 | Jobs | Job polling çoğaltması | PARTIAL | Düşük | IPC maliyeti | M | — | Tur 4 |
| TD-19 | P1 | Model runtime | Extraction tüm sayfa tekrarı | CONFIRMED | Yok | Yavaşlık | M | — | Tur 5 |
| TD-20 | P1 | Model runtime | Rubrik retry full-resend | CONFIRMED | Yok | Maliyet | M | — | Tur 5 |
| TD-21 | P1 | OCR | Deskew/registration/OCR DPI yok | CONFIRMED | Orta | OCR kalitesi | L | Golden set | Tur 6 |
| TD-22 | P1 | OCR | Eager 5 preprocess varyantı | CONFIRMED | Yok | Maliyet | M | — | Tur 6 |
| TD-23 | P1 | Testing | Performans test kapsamı sıfır | CONFIRMED | Orta | Regresyon riski | M | — | Tur 0 |
| TD-24 | P2 | Architecture | Ham Project + types.ts monolit | CONFIRMED | Düşük | Drift | L | — | Tur 8 |
| TD-25 | P2 | Observability | Correlation zinciri kırık | CONFIRMED | Düşük | İzleme zayıf | M | — | Tur 4 |
| TD-26 | P2 | Architecture | Büyük servisler/AppState | CONFIRMED | Düşük | Bakım | L | — | Tur 8 |
| TD-27 | P2 | Prompt/schema | Legacy prompt fallback dormant | PARTIAL | Düşük | Schema'sız çağrı | S | — | Tur 4 |
| TD-28 | P2 | Scoring | Fingerprint calibration/anchor none | CONFIRMED | Orta | Cache geçersizliği | S | — | Tur 7 |
| TD-29 | P2 | Jobs | Merkezi job store kısmen | PARTIAL | Düşük | Maliyet | M | — | Tur 4 |
| TD-30 | P2 | Analysis | Analysis structured (çözüldü); performans entegrasyonu yok | ALREADY_FIXED/PARTIAL | Düşük | Analiz eksikliği | M | — | Tur 8 |
| TD-31 | P2 | TYMM perf | Şablonlar frontend'de | CONFIRMED | Düşük | Dağıtım | S | — | Ertelenebilir |
| TD-32 | P2 | Testing | Golden set/benchmark yok | CONFIRMED | Orta | Ölçülemezlik | XL | — | Tur 0/6 |
| TD-33 | P2 | Testing | Frontend integration testi az | CONFIRMED | Düşük | Regresyon | L | — | Tur 0 |
| TD-34 | P3 | Performance | Preprocess eager maliyeti | CONFIRMED | Yok | Maliyet | M | TD-22 | Tur 6 |
| TD-35 | P3 | Performance | Model/runtime tuning benchmark'sız | NOT_FOUND (kanıt yok) | Yok | İyileştirilemez | XL | TD-32 | Tur 7 |
| TD-36 | P3 | Performance | Speaking retry 2s sabit | CONFIRMED | Yok | Küçük | S | — | Ertelenebilir |
| TD-37 | P3 | Performance | Deterministik kapsam genişletilebilir | PARTIAL | Yok | Kalite | M | — | Tur 7 |
| TD-38 | P3 | Performance | Rapor O(n) tarama | CONFIRMED | Yok | Gecikme | S | — | Ertelenebilir |
| TD-39 | P3 | Exports | PDF window.print vs pdf_service | CONFIRMED | Yok | Format | S | — | Ertelenebilir |

---

## 30. Yapılmazsa ne olur?

- **TD-01:** Aynı projede ikinci bir yazılı sınav açıldığında öğretmen ikinci sınavın sorularını/OCR'ını birinciyle karışmış görecek; puanlamada yanlış öğrenci-soru eşleşmeleriyle yanlış notlar oluşacak. Tek sınavlı projede görünmez.
- **TD-02:** Öğretmen "Eksik" işaretlediğinde (stale ekran veya API çağrısı) öğrencinin onaylanmış final puanı, geri bildirimi ve onay bilgisi sessizce silinecek; rapor geçmişi olmadığı için geri dönülemez. Karar "öğretmen onayı olmadan" değiştirilmiş olur.
- **TD-06:** Öğretmen bir sınıf uygulamasını kaldırırsa o sınıftaki tüm onaylı performans kararları kaybolur.
- **TD-04:** Rubrik yeniden yayınlanınca öğretmenin önceki taslak toplamları sessizce değişir; öğretmen farkı fark etmeyebilir.
- **TD-07:** Onaylanmamış geçici puanlar rapora final puan gibi girer; veli/okul raporunda onaysız karar yayılır.
- **TD-08:** Öğrenci adına enjeksiyon içeren bir değer yazılırsa Excel'de formül çalışabilir (veri sızıntısı).
- **TD-11:** Bir öğrencinin OCR kaydı eksikken "hazır" görünür, puanlama başlar ve o öğrenci puanı eksik kalır.
- **TD-03/TD-10:** Ekran "hazır" dese de backend reddeder ya da tam tersi; öğretmen hangi adımda olduğunu yanlış okur.
- **TD-14/TD-15:** Uygulama yeniden başlarken job durumları kaybolur veya nadir panic job sistemi durdurur; öğretmen işlemi yeniden başlatmak zorunda kalır.
- **TD-19/TD-20/TD-22:** Uzun sınavlarda extraction/OCR 2-5 kat yavaşlar; time-out riski.

---

## 31. Zorunlu / pilot öncesi / ertelenebilir ayrımı

**A. Uygulamayı gerçek kullanımdan önce zorunlu**
- TD-01 (yazılı çoklu sınav kapatma veya kapsam migration), TD-02 (onaylı kayıt koruması), TD-06 (class application silme koruması).

**B. Kontrollü pilot öncesi önerilen**
- TD-04 (re-pin), TD-05 (duplicate ID), TD-07 (rapor geçici/final), TD-08 (CSV injection), TD-09 (frontend yarışı), TD-12 (legacy default), TD-17 (teknik sızıntı), TD-23 (test tabanı).

**C. Birden fazla sınav/performans görevi kullanmadan önce zorunlu**
- TD-01 kapsam migration'ı (yazılı taraf); performans tarafında TD-05 (duplicate kayıt) ve TD-03 (backend readiness) çözülmeden çok görevli pilot başlatılmamalı.

**D. Yeni özellik geliştirmeden önce ödenmesi önerilen**
- TD-03/TD-10 (tek otorite), TD-11 (readiness), TD-13/TD-14/TD-15 (command/job güvenliği), TD-16 (OCR canonical), TD-24 (DTO sınırı), TD-25 (correlation).

**E. Ertelenebilir**
- TD-19/TD-20 (extraction verimliliği — benchmark sonrası), TD-21/TD-22 (OCR kalitesi — golden set sonrası), TD-28, TD-32/TD-35 (benchmark), TD-36/38/39, TD-30/31.

**F. Kabul edilebilir/geçici borç**
- TD-31 (şablonlar frontend'de — publish backend doğrulaması var), TD-26 (servis büyüklüğü — kullanım kısıtı ile), TD-37 (deterministik kapsam), TD-39 (PDF print) — her biri açık guard/kullanım kısıtıyla ertelenebilir.

Gerekçe: A grubu puan/veri/karar doğruluğunu doğrudan etkiler (P0). B grubu öğretmen iş akışında veri karışıklığı/karar görünürlüğü oluşturur. C grubu scope bağımlı. D grubu borç üretimini durdurur. E/F ölçüm veya açık guard gerektirenlerdir.

---

## 32. Önerilen uygulama turları

**Tur 0 — Test tabanı (küçük):** Performans için backend kontrat testleri (P0-2, TD-05, TD-06 senaryoları) + frontend helper testleri (performanceOrganizationUi/performanceReportUi) + (opsiyonel) golden-set başlangıcı.
Dosyalar: performance_service.rs test modülü, assessment_organization_service.rs test modülü, yeni `src/pages/performance*.test.ts`.
Bağımlılık: yok. Migration: yok. Risk: düşük. Kabul: testler kırmızı senaryoyu yakalar.
Test: `cargo test performance`, `npm test`. Ayrı onay: gerekmez.

**Tur 1 — Veri/puan doğruluğu (kritik):** TD-02 (None dalı onay koruması), TD-06 (silme dependency), TD-05 (assessment_id çapraz doğrulama), TD-04 (re-pin koruması), TD-07 (rapor provisional/final), TD-08 (CSV injection), TD-12 (legacy default), TD-17 (teknik sızıntı), TD-09 (frontend eşzamanlımutation koruması).
Dosyalar: performance_service.rs, assessment_organization_service.rs, performanceReportUi.ts, PerformanceScoringPage.tsx, scoring.rs.
Bağımlılık: Tur 0 testleri. Migration: TD-12 semantic; geriye uyum dikkat. Risk: orta (yazılım davranış değişikliği yok, kural sıkılaştırma).
Kabul: P0-2 ve TD-06 için regresyon testleri yeşil; rapor geçici/final ayrımı UI'da görünür.
Test: `cargo test performance assessment_organization`, `npm test`, `cargo clippy --all-targets`.
Ayrı onay: TD-12 migration'ı kullanıcı onayıyla (eski kayıt anlamı).

**Tur 2 — Workflow tek otoritesi:** TD-03 (performans backend readiness DTO), TD-10 (workflow kısa devre + otorite), TD-11 (readiness set-based).
Dosyalar: performance_service.rs (get_performance_status), workflow_engine.rs, scoring.rs, examWorkspace.ts.
Migration: yok. Risk: orta. Test: workflow + performans kontrat testleri. Ayrı onay: gerekmez.

**Tur 3 — Assessment scope migration:** TD-01 kapsam migration (yazılı + listening için activity-scoped veri) VEYA en azından backend çoklu yazılı sınav kapatma.
Dosyalar: domain/project.rs, domain/assessment.rs, question_text_service, student_scan_service, student_answer_ocr_service, scoring_service, rubric_service, project_store migration, frontend DTO.
Migration: EVET (versioned, backup-gated). Risk: yüksek. Test: e2e iki yazılı sınav fixture + eski proje açılışı. Ayrı onay: ZORUNLU (kullanıcı/veri sahibi).

**Tur 4 — Command/error/job güvenliği:** TD-13 (error validator), TD-14 (rehydrate), TD-15 (unwrap/commit), TD-16 (OCR canonical), TD-18 (polling), TD-25 (correlation), TD-27 (legacy prompt fail-closed).
Migration: yok. Risk: düşük-orta. Ayrı onay: gerekmez.

**Tur 5 — Prompt/schema/model pipeline:** TD-19 (page window), TD-20 (rubric retry), (prompt/schema tamamlanmış — korunacak regression testleri).
Migration: yok. Risk: orta (model davranışı). Ayrı onay: gerekmez; benchmark zorunlu değilse feature flag.

**Tur 6 — OCR kalite hattı:** TD-32 golden set + TD-21 (DPI/deskew/registration), TD-22/TD-34 (adaptive preprocess). Benchmark zorunlu. Ayrı onay: gerekmez; ölçümle karar.

**Tur 7 — Deterministik scoring ve kalibrasyon:** TD-28 (fingerprint calibration/anchor), TD-37 (kapsam genişletme), kalibrasyon/anchor altyapısı.
Ayrı onay: gerekmez.

**Tur 8 — Modülerleşme:** TD-24 (DTO/read model), TD-26 (servis ayrımı), TD-30 (analiz entegrasyonu isteğe bağlı).
Migration: yok (DTO ekleme additive). Risk: düşük-orta. Ayrı onay: gerekmez.

---

## 33. Migration ve kullanıcı onay kapıları

- TD-12 (legacy scoring default): semantic migration — kullanıcı onayı ister (eski kayıtların anlamı değişir; puan silinmez).
- TD-01 kapsam migration'ı: veri taşıma — **kullanıcı/veri sahibi onayı zorunlu**; öncesinde verified backup + eski proje fixture doğrulaması.
- Performans alanları (mevcut): additive ve idempotent; kullanıcı onayı gerektirmez (zaten yayında).
- Genel kural: `MigrateWithVerifiedBackup` kapısı (project_store.rs:422-434) korunmalı; Tur 3/1'de bu kapıya uyulmalı.

---

## 34. Kalan belirsizlikler

- Smoke testi bu ortamda port çakışması nedeniyle koşulamadı (kullanıcının canlı vite süreçleri); smoke'un PASS olması kanıtlanmadı.
- `cargo clippy --all-targets --all-features -- -D warnings` 5 test-kodu hatasıyla exit 101; üretim kodu clippy-temiz değil mi diye ayrım yapılamadı (5 hata da test modülünde; üretim hedefleri için ayrı çalıştırılmadı).
- Kullanıcı onaylı live UI kabulü (gerçek sınıf/öğrenci üzerinde performans akışı) yapılmadı.
- P0-2'nin gerçek tetiklenme sıklığı (stale-listeyle UI) canlı oturum gerektirir; kod yolu backend'de kesin doğrulandı.
- Model ikilisinin varlığı ve bu ortamda model testlerinin neden geçtiği (önceki raporlarda hang notu vardı) doğrulanmadı.
- `derivePerformanceStepStatuses`'in backend DTO'su olmadan ne kadar güvenilir olduğu canlı senaryoda ölçülmedi.

---

## 35. Nihai değerlendirme

`performans_degerlendirme` dalı, önceki denetimin aksine **test ve kalite kapılarında tamamen yeşildir** (check:all PASS; cargo test 494+; frontend 147/147; yalnız `--all-targets` clippy'de 5 test-kodu lint'i ve port çakışması nedeniyle koşulmayan smoke). Yazılı scoring ve model pipeline tarafında önceki P0'ların çoğu gerçekten çözülmüştür (decision_state, needs_review final'den çıkarılması, canonical ID, deterministik scoring + cache, typed structuredAnswer, prompt izolasyonu, StrictLocal, completion probe kaldırılması).

Yeni TYMM performans özelliği **scope modeli ve veri tutarlılığı açısından iyi tasarlanmış** ve uygulanmıştır; ancak iki ciddi backend açığı (TD-02 onaylı kayıt koruması, TD-06 class-application silme veri kaybı) ve yazılı tarafın hâlâ proje seviyesinde olması (TD-01) release öncesi engeldir. Performans akışının Missing/NotPerformed/zero ayrımı, rubrik sürümleme kilidi ve onay kuralları doğru çalışmaktadır; raporun geçici/final ayrımı ve frontend eşzamanlılık koruması pilot öncesi tamamlanmalıdır.

P0'lar (TD-01 kapatma, TD-02) ve A grubu öğeler çözülmeden genel kullanıma açılmamalıdır. Kısıtlı tek ders/sınıf pilotu, TD-02 + TD-06 hızlı düzeltmeleri sonrasında mümkündür (PILOT_ONLY seviyesi); çoklu sınav/görev kullanımı Tur 3 kapsam migration'ını beklemelidir.

```text
Teknik borç denetim kararı: RELEASE_BLOCKED
```
