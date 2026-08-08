# RubrikaV3 — Özet Diff (Final Technical Debt Closure Kampanyası)

**Tarih:** 2026-08-06 · **Baz:** `fdb8e6e` ("düzeltme") · **Ağaç durumu:** 52 değiştirilmiş + 39 yeni (untracked) dosya = **91 toplam** · **Commit yok**

> Değişikliklerin tamamı çalışma ağacında; hiçbiri commit edilmedi. Bu dosya, `git diff` içeriğinin **özeti**dir (tam diff: `git diff` — 6098 ekleme / 3372 silme).

---

## 1. Değiştirilmiş dosyalar (52) — diff stat

```
 docs/API_CONTRACTS.md                                  |    4 +-
 docs/ERROR_CODES.md                                    |    3 +
 src-tauri/src/commands/assessment_organization_commands.rs  |   10 +
 src-tauri/src/commands/performance_commands.rs          |   46 +-
 src-tauri/src/commands/scoring_commands.rs              |   11 +-
 src-tauri/src/commands/student_answer_ocr_commands.rs   |   23 +-
 src-tauri/src/diagnostics.rs                            |    3 +
 src-tauri/src/domain/errors.rs                          |   14 +
 src-tauri/src/domain/model.rs                           |   37 +
 src-tauri/src/domain/project.rs                         |  473 ++++
 src-tauri/src/domain/question.rs                        |    5 +
 src-tauri/src/domain/scoring.rs                         |  117 +-
 src-tauri/src/domain/student.rs                         |   10 +
 src-tauri/src/jobs/job_manager.rs                       |   82 +-
 src-tauri/src/lib.rs                                    |    4 +-
 src-tauri/src/services/analysis_service.rs              |    1 +
 src-tauri/src/services/assessment_organization_service.rs | 369 +++-
 src-tauri/src/services/exam_package_build_service.rs    |    7 +-
 src-tauri/src/services/generation_gc_service.rs         |    2 +
 src-tauri/src/services/graded_exam_review_service.rs    |    2 +
 src-tauri/src/services/llama_server_gateway.rs          |  240 +--
 src-tauri/src/services/mod.rs                           |    4 +
 src-tauri/src/services/ocr_image_preprocess_service.rs  |  210 ++
 src-tauri/src/services/pdf_preview_service.rs           |    2 +
 src-tauri/src/services/performance_service.rs           | 1443 ++++++------
 src-tauri/src/services/project_store.rs                 |  714 ++++++-
 src-tauri/src/services/prompt_contract.rs               |   88 +-
 src-tauri/src/services/question_text_service.rs         |  830 +++++++-
 src-tauri/src/services/rubric_extraction_service.rs     |  765 ++++++-
 src-tauri/src/services/rubric_service.rs                |    3 +
 src-tauri/src/services/school_class_service.rs          |   65 +
 src-tauri/src/services/scoring_anchor_service.rs        |    2 +
 src-tauri/src/services/scoring_cache_service.rs         |   72 +-
 src-tauri/src/services/scoring_consistency_service.rs   |    1 +
 src-tauri/src/services/scoring_service.rs               |  220 +-
 src-tauri/src/services/speaking_exam_service.rs         |  117 +-
 src-tauri/src/services/student_answer_crop_service.rs   |  128 +-
 src-tauri/src/services/student_answer_ocr_service.rs    |  819 +++++++-
 src-tauri/src/services/student_scan_service.rs          |   21 +-
 src-tauri/src/services/workflow_engine.rs               |  125 +-
 src/api/commands.ts                                     |    7 +
 src/api/types.ts                                        | 2211 +-------------------
 src/app/examWorkspace.test.ts                           |   78 +
 src/pages/CanonicalExamWorkspacePage.tsx                |   13 +-
 src/pages/DocumentsPage.tsx                             |    1 -
 src/pages/ExamPackageWorkspacePage.tsx                  |    1 -
 src/pages/PerformanceScoringPage.tsx                    |    5 +
 src/pages/ScoringPage.tsx                               |    4 -
 src/pages/StudentAnswerOcrIssueReviewPage.tsx           |    4 -
 src/pages/StudentAnswerOcrPage.tsx                      |    4 -
 src/pages/WorkflowPage.tsx                              |    4 -
 src/pages/performanceScoringUi.test.ts                  |   46 +
 52 files changed, 6098 insertions(+), 3372 deletions(-)
```

## 2. Yeni dosyalar (39, untracked)

### Kod (üretim + test) — 7
| Dosya | Faz | Açıklama |
|---|---|---|
| `src-tauri/src/bin/golden_ocr_benchmark.rs` | 7+ | Golden OCR benchmark runner (gerçek model, CER/WER/leakage) |
| `src-tauri/src/services/golden_ocr_metrics.rs` | 6 | CER/WER/critical-token/leakage saf ölçüm fonksiyonları |
| `src-tauri/src/services/ocr_image_geometry_service.rs` | 7 | Deskew/registration/DPI geometri hattı |
| `src-tauri/src/services/page_window_service.rs` | 5 | Question→page map + ±1 pencere eskalasyonu |
| `src-tauri/src/services/performance_dtos.rs` | 9 | Performans komut kontratı DTO'ları (15 tip) |
| `src-tauri/tests/golden_tymm_tde_001.rs` | 6 | Golden corpus entegrasyon testi (14 test) |
| `testdata/` (golden paket) | 6 | `golden/tymm_tde_001/` — 7 corpus dosyası + `manifest.sha256` |

### Frontend tip modülleri (17) — Faz 9 (TD-24: types.ts monoliti bölündü)
`src/api/types.analysis.ts` · `types.app.ts` · `types.assessment.ts` · `types.document.ts` · `types.gradedExam.ts` · `types.jobs.ts` · `types.model.ts` · `types.ocr.ts` · `types.performance.ts` · `types.project.ts` · `types.question.ts` · `types.rubric.ts` · `types.schoolClass.ts` · `types.scoring.ts` · `types.speaking.ts` · `types.student.ts` · `types.workflow.ts`
→ `src/api/types.ts` artık barrel (2211 satır → ~50).

### Dokümantasyon + görev dosyaları (15)
`docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` (matris + faz kapanış kanıtları + §14 KAMPANYA TAMAMLANDI) · `docs/GOLDEN_OCR_SCORING_BENCHMARK.md` (benchmark metodolojisi + bölüm 5 sonuçlar) · `.audit_cache/final_closure_faz01…faz11_task.md` (13 deterministik görev dosyası) · `.audit_cache/tur4b_task.md`

## 3. Değişikliklerin faz eşlemesi (özet)

| Faz | Anahtar dosyalar | Ne değişti |
|---|---|---|
| 0+1 | `domain/scoring.rs`, `project_store.rs`, `performance_service.rs`, `PerformanceScoringPage.tsx`, `performanceScoringUi.test.ts` | Veri güvenliği varsayılanları, normalize explicit, approve tek-final guard |
| 2 | `job_manager.rs` (+82) | Yutulan commit hataları → typed error |
| 3 | `domain/project.rs` (+473), `project_store.rs` (+714 büyük), `domain/question.rs`, `domain/student.rs` | `activity_scope` migration + izolasyon testleri |
| 4 | `workflow_engine.rs` (+125), `project.rs` | workflow tek otoritesi, cache-only |
| 5 | `question_text_service.rs` (+830), `rubric_extraction_service.rs` (+765), `llama_server_gateway.rs` (−240 net), `page_window_service.rs` (yeni) | Hedefli extraction, ±1 pencere, bounded fallback |
| 6 | `golden_ocr_metrics.rs` (yeni), `golden_tymm_tde_001.rs` (yeni), `testdata/` (yeni) | Golden corpus + ölçüm altyapısı |
| 7 | `ocr_image_geometry_service.rs` (yeni), `ocr_image_preprocess_service.rs` (+210), `student_answer_ocr_service.rs` (+819), `student_answer_crop_service.rs` (+128) | Deskew/registration/DPI, tek varyant, atomik commit |
| 7+ | `golden_ocr_benchmark.rs` (yeni), `student_answer_crop_service.rs` | Gerçek model benchmark + registration gating |
| 8 | `scoring_service.rs` (+220), `scoring_cache_service.rs` (+72), `scoring_anchor_service.rs`, `scoring_consistency_service.rs`, `domain/scoring.rs` | Fingerprint kalibrasyon sabitleri, cache-key sürümleme |
| 9 | `types.ts` (−2211), 17 yeni `types.*.ts`, `performance_dtos.rs` (yeni), `performance_commands.rs` (−46), `assessment_organization_commands.rs` | Tip monoliti bölme, DTO taşıma, bbox dönüşümü |
| 10 | `speaking_exam_service.rs`, `assessment_organization_service.rs` (+369) | Retry sabiti, O(1) rapor indeksi |
| 11 | `docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` | Yalnız dokümantasyon (§14) — kod yok |

## 4. Test büyümesi (kampanya boyunca)

| Ölçüt | Başlangıç | Son | Δ |
|---|---|---|---|
| cargo lib (ortam testi hariç) | 530 | **594** | +64 |
| golden entegrasyon | — | **14** | +14 |
| npm test | 161 | **163** | +2 (UI senaryoları) |
| Yeni test modülleri | — | page_window, golden_ocr_metrics, golden_tymm_tde_001, ocr geometry/preprocess | 4+ |

## 5. Dikkat edilecekler

- `performance_service.rs` 1443 satırlık diff büyük görünür ama **Faz 0/1–10 birikimi** (guard'lar, DTO taşıması, indeks, adlandırma) — davranış değişikliği testlerle kilitli.
- `src/api/types.ts`'deki −2211 satır barrel'e taşındı, tip kaybı yok (typecheck + 163 test).
- `testdata/` tamamı yeni (golden corpus) — commit edilecekse `manifest.sha256` ile birlikte.
- `.audit_cache/` görev dosyaları kampanya izi — istersen commit dışı bırakılabilir.
- **Commit öncesi öneri:** `git add` + tek tematik commit (ör. "technical debt closure: TD-01–39, golden corpus, OCR pipeline") — senin kararın; otomatik commit yapılmadı.
