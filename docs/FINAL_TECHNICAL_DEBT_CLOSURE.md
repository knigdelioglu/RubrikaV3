# Final Technical Debt Closure — Kabul Matrisi — KAMPANYA TAMAMLANDI (FAZ 11)

Bu dosya, "Final Technical Debt Closure — Activity Scope, OCR Golden Pipeline, Model
Efficiency, Scoring Calibration and Modular Boundaries" kampanyasının **tek otorite
borç matrisidir**. FAZ 0'da `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` içindeki TD-01–TD-39
maddeleri güncel production koduna ve testlere göre yeniden sınıflandırılmıştır.

- FAZ 0: Başlangıç doğrulaması + yeniden sınıflandırma (bu dosya).
- FAZ 1: Performans değerlendirme veri güvenliği açıkları (1.1–1.9) kapatıldı (bölüm 2).
- FAZ 2: TD-15 commit semantiği + yutulan commit hataları kapatıldı; TD-14
  rehydrate hata yolu typed + tek nokta doğrulandı (bölüm 3).
- FAZ 3: TD-01 AssessmentActivity scope migration kapatıldı (bölüm 4): yazılı
  (ve listening) family verisi artık `assessment_activity_id` taşır; versioned,
  backup-gated, idempotent migration + typed ambiguity blocker; çoklu yazılı
  sınav izolasyonu (soru/OCR/scoring/QEP) testli.
- FAZ 4: TD-10 workflow/readiness tek otoritesi kapatıldı; TD-03 ve TD-11 FAZ 4
  kapsamında doğrulandı (bölüm 5): `Project.workflow` açıkça cache-only
  işaretlendi; canlı hesaplama tek otorite (persisted snapshot hiçbir dalda aynen
  dönmez); bozuk-cache regresyon testleri; set-based scoring readiness + boş-liste
  vacuous-ready koruması testli; frontend derive'ları backend DTO'ya normalize
  edildi (listening kontrat testi eklendi).
- FAZ 5: TD-19 ve TD-20 kapatıldı (bölüm 6): soru/rubrik extraction hedef
  sayfa/region penceresiyle sınırlı; rubrik parse retry'i görselsiz salvage /
  text-only repair kullanıyor; multimodal retry yalnız açık reason ile.
- FAZ 6: TD-32 kapatıldı (bölüm 7): golden sınav paketi committed test corpus'a
  dönüştürüldü (SHA-256 manifest + 9 integration test + CER/WER/leakage/exact-match
  metrik modülü + `GoldenOcrBenchmarkReport` DTO); model benchmark'ı dürüstçe
  `NEEDS_MODEL_RUNTIME`.
- FAZ 7: TD-21, TD-22 ve FAZ 7 iş emri TD-28 kapatıldı (bölüm 8): OCR görüntü
  hattı — deskew/registration/DPI saf fonksiyonları (`ocr_image_geometry_service.rs`,
  açı sınırlı + typed out-of-range reddi), deterministik istatistik tabanlı
  preprocess varyant seçimi (eager 5x kaldırıldı; tek varyant + `Original`
  fallback, diagnostics'te gerekçe), OCR sonuç persistence atomicity regresyon
  testi (kısmi yazma enjeksiyon kanıtı), golden 03 entegrasyonu (deskew/
  registration/DPI sınır içinde; 01/02 regresyonu kırılmadı).
- FAZ 7+ (gerçek model): TD-21 registration gating kalıntısı kapatıldı (bölüm 9):
  golden 03 üzerinde gerçek model benchmark'ı (Gemma 4 12B, 300 DPI) çalıştırıldı;
  registration doğrulaması üretim crop hattına bağlandı (`validate_registration`,
  typed `RegistrationOutOfRange`, regresyon testi). TD-32 benchmark statüsü
  `NEEDS_MODEL_RUNTIME` → `available` (bölüm 9). Yeni korpus bulgusu: 06 bbox
  y-ekseni PDF alt-sol kaynaklıdır, crop matematiği üst-sol bekler (runner
  dönüşüm uygular; korpus düzeltmesi Faz 8 adayı). Q3 madde 5 tutarsızlığı
  "öğrencinin gerçek hatası" olarak çözüldü: rubrik `answer_key` canonical,
  12/15 = `expected_scoring.q3` ile tutarlı.
- FAZ 8 (bu dosya, bölüm 10): orijinal TD-28 scoring fingerprint kalibrasyonu
  kapatıldı — fingerprint `calibration_version`/`anchor_version` artık gerçek
  politika sabitleri taşır (`SCORING_CALIBRATION_VERSION`, `SCORING_ANCHOR_SET_VERSION`,
  prompt_contract.rs), cache anahtarına girdiği ve sürüm sabitini artırmanın eski
  cache'leri geçersiz kıldığı testle kanıtlandı; `"none"` placeholder kalmadı.
  TD-37 deterministik scoring kapsam değerlendirmesi yapıldı (golden 05/06 answer-type
  eşleşmesi: Table/Matching/CorrectionTable kapsanıyor; Essay/GrammarAnalysis golden'da
  deterministic işaretli değil → güvenli ekleme yok, karar golden regresyon testiyle
  kilitlendi). TD-34 statüsü doğrulandı (`ALREADY_FIXED`, FAZ 7 referansı). TD-35
  `NOT_FOUND` → `PARTIAL` (benchmark + ilk ölçüm mevcut; tuning kararları açık).
- FAZ 9 (bu dosya, bölüm 11): **TD-24 kapatıldı** — frontend `types.ts` monoliti domain
  alanına göre 17 modüle bölündü (`types.{app,analysis,assessment,document,gradedExam,jobs,model,ocr,performance,project,question,rubric,schoolClass,scoring,speaking,student,workflow}.ts`),
  `types.ts` salt barrel (`export type *`) oldu; import grafiği korundu (54 tüketici dokunulmadı),
  `npm run typecheck` + `npm test` (163) + lint yeşil. **TD-26 sınırlı kapanış** — performans
  komut kontratı DTO'ları net sınırla `performance_dtos.rs`'a taşındı (serde kontratı aynı, davranış
  değişikliği yok, `performance_service.rs` 2864→~2655 satır, performans testleri aynen yeşil);
  servis mantığı parçalama ve AppState düz yapısının bölünmesi riskli/büyük olduğundan gerekçeyle
  ertelendi. **Korpus bbox düzeltmesi kapatıldı** — 06 bbox y-ekseni alt-sol→üst-sol dönüşümü tek
  paylaşılan fonksiyona alındı (`golden_ocr_metrics::corpus_bbox_bottom_left_y_to_top_left`);
  runner + tüm golden test tüketicileri uygular; regresyon testi eklendi; üretim crop hattının
  golden bbox tüketmediği kanıtlandı; doküman (bölüm 8) güncellendi.
- Sonraki çalışmalar (kampanya dışı, ayrı görev dosyalarıyla): yapısal şema uyumu
  (`structuredAnswer`), deterministik scoring ölçümü, model verimliliği, kalibrasyon tuning'i
  (TD-35), TD-26 servis mantığı parçalama/AppState bölme.
- FAZ 10 (bu dosya, bölüm 12): **kalan açık borçların nihai kararları** — TD-36 ve TD-38
  kapatıldı (sırasıyla adlandırılmış retry gecikmesi sabiti ve rapor taramasında assessments
  indeksi, davranış değişmez, testli); TD-31 ve TD-39 kabul edilebilir/ertelendi (gerekçeyle);
  Faz 9'da ertelenen TD-26 servis mantığı parçalama + AppState bölme kararı bilinçli erteleme
  olarak kesinleştirildi (kod değişikliği yok).
- FAZ 11 (bu dosya, bölüm 14) — **SON FAZ**: birikmiş tüm fazlar tam doğrulama kapılarından
  geçirildi (kod değişikliği yok; yalnız docs): `npm run check:all` (build/typecheck/lint/npm
  test 163/cargo:fmt/cargo:clippy PASS; `cargo:test` yalnızca ortam kaynaklı mmproj testi fail),
  tam Rust workspace suite (`--skip start_job_returns_model_mmproj_missing` ile EXIT 0; lib 594
  passed, golden 14 passed, tüm integration testleri yeşil), clippy `--all-targets --all-features`
  PASS, fmt PASS, build smoke (cargo build + npm run build) PASS, son bütünlük (HEAD `fdb8e6e`,
  `stash@{0}`, golden manifest, commit yok) doğrulandı. Kampanya sonu özeti bölüm 14.5'te.

Durum anlamları:

- `CONFIRMED` — açık borç; uygulanmadı.
- `ALREADY_FIXED` — production kodunda çözülmüş; regresyon testi yeşil.
- `PARTIAL` — kısmen çözülmüş; kalan kısmı ayrı turda.
- `NOT_FOUND` — kanıt yok; benchmark/girdi gerektirir.
- `NEEDS_RUNTIME_PROOF` — kod statik olarak güvenli görünüyor; canlı çalışma doğrulaması gerekir.

---

## 1. TD-01–TD-39 kabul matrisi (güncel production koduna göre)

| ID | Öncelik | Başlık | Yeni Durum | Kanıt / Kaynak | Kapanış Notu |
|----|---------|--------|------------|----------------|--------------|
| TD-01 | P0 | Yazılı sınav soru/OCR/scoring verileri proje seviyesinde | ALREADY_FIXED | `domain/project.rs` (`questions`, `student_submissions`, `scoring_records`, `exam_package_freeze` artık `assessment_activity_id` taşır), `Project.active_written_assessment_activity_id`, `written_scope_view`/`resolve_written_scope_id`, migration `normalize_written_activity_scope` (project_store.rs), `set_active_written_activity` komutu | FAZ 3 kapanışı (bölüm 4). Additive alanlar + backup-gated idempotent migration; ambiguity'de typed blocker; çoklu written activity izolasyonu fixture'ları yeşil; activity silme dependency scan'i testli. **Kalan kalıntı:** bazı salt-okunur list/read yolları (ör. OCR accept/reject/update_text ve graded_exam_review okumaları) kayıt kimliğiyle çalıştığı için çoklu written activity senaryosunda scoped görünüm kullanmaz — yazım yolları tamamen scoped olduğundan veri karışma riski yok, yalnız görüntüleme; listeleme DTO'larının activity filtresi FAZ 10 (DTO/read-model) kapsamında tamamlanacak. |
| TD-02 | P0 | `set_performance_assessment_status` None dalı onaylı kaydı silebiliyor | ALREADY_FIXED | `performance_service.rs` `set_performance_assessment_status` — `approved_record_exists` hem `Some` hem `None` dalında (satır ~852-876); test `approved_assessment_status_cannot_be_changed_without_assessment_id` | FAZ 1 madde 1.1. Onaylı kayıt hiçbir save/status/approve komutuyla değiştirilemiyor; sessiz reopen yok. |
| TD-03 | P1 | Performans adım hazırlığı yalnız frontend'de türetiliyor | ALREADY_FIXED | Backend `get_performance_status` (performance_service.rs:1195) + `PerformanceStatusDto`; `commands.ts:761 getPerformanceStatus`; frontend `examWorkspace.ts:87-93` DTO'yu tüketiyor | FAZ 1 kapsamı dışında kalıp önceki turda kapanmış; backend authoritative readiness DTO'su var. FAZ 4'te family kontratı doğrulandı: performance `get_performance_status` DTO'su, written/listening `WorkflowSnapshot` (evaluate_workflow), speaking backend activity snapshot'ı tüketiyor; frontend derive'ları yalnız normalize eder (listening kontrat testi FAZ 4'te eklendi). |
| TD-04 | P1 | Taslak save'de rubrik re-pin sessiz puan değişimi | ALREADY_FIXED | `performance_service.rs` `save_performance_assessment` (satır ~625-646): existing kayıt kendi `rubric_id`/`rubric_version`'ını korur, pinned rubrikle validate/hesaplar; test `republished_rubric_does_not_rebase_existing_draft_version_or_total` | FAZ 1 madde 1.4. Yeni kayıt en yeni sürümü pinler; mevcut taslak değişmez. |
| TD-05 | P1 | Yabancı assessment_id duplicate kayıt | ALREADY_FIXED | `save_performance_assessment` scope çapraz doğrulaması (satır ~594-609): ID application+student üyeliğini doğrulamazsa `AssessmentInvalidInput`; test `save_rejects_assessment_id_that_belongs_to_another_application` | FAZ 1 madde 1.2. Ayrıca `approve_performance_assessment` tek-final koruması + `approve_rejects_a_second_final_assessment_for_the_same_student` eklendi. |
| TD-06 | P1 | Class application silmede performans değerlendirmesi kaybı | ALREADY_FIXED | `assessment_organization_service.rs` `remove_class_application` (satır ~650): `performance_assessments` ve `speaking_attempts` boş değilse `AssessmentClassApplicationInUse`; test `class_application_with_performance_assessment_cannot_be_removed` | FAZ 1 madde 1.3. |
| TD-07 | P1 | Rapor geçici toplamı final gibi | ALREADY_FIXED | `get_performance_report` DTO: `total` yalnız Approved, `provisional_total` Approved+InProgress (satır ~1138-1149); test `report_does_not_publish_in_progress_total_as_final_total` | FAZ 1 madde 1.5. Sonuç tablosu InProgress satırı "(geçici)" etiketiyle ayrıştırıldı (PerformanceScoringPage.tsx). CSV/PDF yalnız Approved toplam basar. |
| TD-08 | P1 | CSV formül enjeksiyonu | ALREADY_FIXED | `performanceReportUi.ts` `escapeCell` `= + - @ tab CR/LF` başlangıçlarını `'` önekiyle kaçışlar; test `CSV output prevents formula injection through user-controlled student names` | FAZ 1 madde 1.6. XLSX export yok (yalnız CSV); hücreler string olarak yazılır. |
| TD-09 | P1 | Frontend eşzamanlı mutation / draft kaybı | ALREADY_FIXED | `derivePerformanceActionAvailability` (`performanceScoringUi.ts`): save/approve/status pending iken approve/status/revert devre dışı; draft-reset effect `anyMutationPending` iken atlar (PerformanceScoringPage.tsx:291-311); testler `performanceScoringUi.test.ts` | FAZ 1 madde 1.7. Backend commit öncesi success gösterilmiyor; save onSuccess cache'i dar günceller. |
| TD-10 | P1 | Birden fazla workflow otoritesi | ALREADY_FIXED | `workflow_engine.rs` live `evaluate_workflow_inner` authoritative; persisted-snapshot kısa devresi kaldırılmış; `Project.workflow` "cache only, live source = evaluate_workflow" olarak işaretli; `get_workflow_snapshot` canlı hesaplar (hiçbir dalda persisted aynen dönmez) | FAZ 4 kapanışı (bölüm 5). Negatif tarama: workflow_engine'de `return project.workflow` yok; bozuk-cache regresyon testi tüm snapshot alanlarının canlı yeniden hesaplandığını kanıtlar. |
| TD-11 | P1 | Count-only OCR/scoring readiness | ALREADY_FIXED | `domain/scoring.rs` `scoring_readiness`: `missing_pairs` + `duplicate_pair_count` set-bazlı kapsam (satır ~719-738); `ocr_ready` duplicate==0 && missing_pairs boş && `expected_records > 0` | Set-bazlı kapsam kontrolü devrede; FAZ 4'te boş-liste vacuous-ready regresyon testi eklendi (`scoring_readiness_does_not_report_vacuous_ready_for_empty_sets`). |
| TD-12 | P1 | Legacy `scoring_applied` serde default `true` | ALREADY_FIXED | `domain/scoring.rs` `default_scoring_applied() -> false`; `project_store.rs` `normalize_scoring_records` legacy kayıtta eksik alanı explicit `scoringApplied` ile sınıflandırır; testler `scoring_record_missing_scoring_applied_fails_closed_and_is_not_accepted`, `legacy_scoring_record_without_scoring_applied_is_explicitly_classified` | FAZ 1 madde 1.8. Eksik alan fail-closed (accepted/final değil); legacy anlam normalize ile korunur. |
| TD-13 | P1 | `as AppError` unvalidated | ALREADY_FIXED | `api/errors.ts` `isAppError` runtime validator + normalize (satır 154-169) | Tur 4. |
| TD-14 | P1 | Job rehydration hatası yutuluyor | ALREADY_FIXED | `job_commands.rs:45-53` `rehydrate_jobs(...).map_err(...)` typed `AppError`; `list_jobs_for_project` tek rehydrate noktası; test `list_jobs_propagates_rehydrate_failure_instead_of_swallowing_it` | Tur 4 + FAZ 2 doğrulaması (bölüm 3). |
| TD-15 | P1 | Production unwrap / yutulan commit hataları | ALREADY_FIXED | `job_manager.rs` production bölümünde `unwrap`/`expect`/`panic!` yok (yalnız test modülü); `speaking_exam_service.rs` `commit_snapshot_cas` hataları `if let Err(...)` ile yayılıyor; tüm `let _ = commit_snapshot_cas` call-site'ları log'lu hata yüzeyine çevrildi; performans/OCR/scoring commit-fail regresyon testleri eklendi | Tur 4 + FAZ 2 kapanışı (bölüm 3). |
| TD-16 | P1 | OCR duplicate canonical/read model | ALREADY_FIXED | `project.resolved_active_ocr_records()` okuyucuları: `workflow_engine.rs:106`, `student_answer_ocr_service.rs:164-549` | Tur 4. |
| TD-17 | P1 | Teknik veri öğretmen arayüzüne sızıyor | ALREADY_FIXED | `BlockingReasons.tsx:10` Türkçe fallback; `PerformanceScoringPage.tsx` `statusLabel` + teacherId ham UUID yerine "Öğretmen ataması mevcut" (satır ~1012) | FAZ 1 madde 1.9. |
| TD-18 | P1 | Job polling çoğaltması | ALREADY_FIXED | Sayfa poller'ları kaldırıldı; tek merkezi job query (tur4 çalışma ağacı) | Tur 4. |
| TD-19 | P1 | Extraction tüm sayfa tekrarı | ALREADY_FIXED | `services/page_window_service.rs` (yeni), `question_text_service.rs` (`extract_question_text_targeted`, sayfa penceresi + eskalasyon), `rubric_extraction_service.rs` (vision loop hedef sayfa/window/fallback) | FAZ 5 kapanışı (bölüm 6). Question-to-page map (pdftotext form-feed + marker), hedef sayfa → ±1 pencere → bounded broad fallback; spy testleri tüm-sayfa clone'un olmadığını kanıtlar. |
| TD-20 | P1 | Rubrik parse retry full-resend | ALREADY_FIXED | `rubric_extraction_service.rs` `draft_rubric_with_retry` (deterministik salvage + text-only repair + explicit-reason multimodal retry), `llama_server_gateway.rs` (salvage genişletme, `read_saved_rubric_raw_response`) | FAZ 5 kapanışı (bölüm 6). Salvage/text-only repair başarılıysa ikinci görsel çağrısı 0; multimodal retry yalnız açık reason ile; retry zinciri testleri. |
| TD-21 | P1 | Deskew/registration/OCR DPI yok | ALREADY_FIXED | `services/ocr_image_geometry_service.rs` (yeni): `deskew_image` (açı ±12° tarama + 0.25° refine; ≥8° `DeskewOutOfRange` typed reddi, 3° operasyon aralığı), `measure_registration_deviation`/`validate_registration` (`crop_rect_normalized` tabanlı sistematik sapma; `RegistrationOutOfRange` typed reddi; DEFAULT_MAX_REGISTRATION_DEVIATION=0.12), `normalize_dpi`/`render_scale_to_dpi`/`validate_dpi_in_range` (hedef 300 DPI sabit); `student_answer_ocr_service.rs` `preprocess_model_inputs` deskew + `render_dpi` provenance'a yazılıyor; golden 03 testleri (`scanned_variant_deskew_accepts_every_page_within_golden_bounds`, `scanned_variant_registration_deviation_stays_within_golden_bounds`, `golden_render_dpi_normalizes_to_fixed_ocr_target`) | FAZ 7 kapanışı (bölüm 8) + FAZ 7+ kapanışı (bölüm 9): registration gating **üretim crop hattına bağlandı** — `student_answer_crop_service::build_sources` Production dalında `validate_page_registration` (typed `RegistrationOutOfRange` fail-closed, boş sayfalar muaf) + regresyon testi `production_rejects_systematically_misregistered_page_and_accepts_aligned_one`. Gerekçe: golden 03 gerçek model benchmark'ında registration 0.004–0.010 (maks 0.035) ölçer; eşik 0.12 → sahte ret riski yok. **Kalan kalıntı:** yok (registration gating kapalı); DPI normalizasyonu üretim hattında hâlâ 300'e yükseltmez (render 144/300 arası), korpus bbox y-ekseni dönüşümü Faz 8 korpus düzeltmesiyle netleştirilecek. |
| TD-22 | P1 | Eager 5 preprocess varyantı | ALREADY_FIXED | `ocr_image_preprocess_service.rs`: `compute_image_statistics` (mean/std/edge_density) + `select_preprocess_variant` (deterministik skor; eşik altı `Original` default; `low_content` guard); `student_answer_ocr_service.rs` `preprocess_model_inputs` yalnız seçilen varyantı üretir | FAZ 7 kapanışı (bölüm 8). Eager 5x üretim kaldırıldı: soru başına yalnız seçilen varyant + `Original` fallback; `preprocess_variant` + `variant_selection_reason` diagnostics/provenance'da; gereksiz ikinci model çağrısı yok; test `preprocess_model_inputs_generates_only_the_selected_variant` (tek varyant, tek diag). |
| TD-23 | P1 | Performans test kapsamı sıfır | PARTIAL | Backend `performance_service.rs` 15 servis + 5 komut + 1 migration testi; frontend `performanceReportUi.test.ts`, `performanceScoringUi.test.ts` | FAZ 1 kapsamındaki senaryoların tamamı testli; genişletme kapsam dışı. |
| TD-24 | P2 | Ham Project + types.ts monolit | ALREADY_FIXED | `src/api/types.ts` (salt barrel), `src/api/types.*.ts` (17 domain modülü) | FAZ 9 kapanışı (bölüm 11): 2194 satırlık type-only monolit domain alanına göre bölündü; tüm tüketiciler barrel'dan adlandırılmış type importu kullanıyordu, import grafiği korundu; `verbatimModuleSyntax` altında `export type *` barrel. Doğrulama: `npm run typecheck` PASS, `npm test` 163 passed, lint yeni uyarı yok. |
| TD-25 | P2 | Correlation ID zinciri kırık | ALREADY_FIXED | `performance_service.rs` tüm mutation'larda `correlation(...)`; test `correlation_id_flows_to_mutation_audit_and_invocation_contract` | Tur 4b. |
| TD-26 | P2 | Büyük servisler / AppState | PARTIAL | `services/performance_dtos.rs` (yeni), `performance_service.rs` 2864→~2655 satır; `lib.rs` `AppState` | FAZ 9 (bölüm 11): komut kontratı DTO'ları net sınırla ayrı modüle taşındı (serde kontratı aynen korundu, davranış değişikliği yok; `cargo test --lib performance` 26 + `performance_commands` 5 yeşil). **Kalan:** servis mantığının (ör. rapor üretimi, rating doğrulama) ayrı modüllere taşınması ve `AppState`'in hizmet gruplarına bölünmesi riskli/büyük refactor — gerekçeyle ertelendi (sonraki fazlar). |
| TD-27 | P2 | Legacy prompt fallback dormant | ALREADY_FIXED | `prompt_contract.rs` `None` durumunda typed hata (fail-closed) | Tur 4b. |
| TD-28 | P2 | OCR sonuç persistence atomicity + scoring fingerprint kalibrasyon (orijinal denetim TD-28) | ALREADY_FIXED | `project_store.rs` `commit_job` (tek `mutate` CAS + transaction journal + atomic replace); test `ocr_result_commit_is_atomic_and_never_writes_partial_state` (kasıtlı hata enjeksiyonu: generation + records + submission status çoklu state yazımı sonrası Err → diskte HİÇBİR state değişmez; retry başarılı). Fingerprint: `prompt_contract.rs` `SCORING_CALIBRATION_VERSION`/`SCORING_ANCHOR_SET_VERSION` (gerçek politika sabitleri); `scoring_service.rs` her iki fingerprint yolunda placeholder yerine bu sabitler; test `calibration_and_anchor_versions_participate_in_the_cache_key_and_invalidate_old_caches` | FAZ 7 kapanışı (bölüm 8): OCR sonucu (record + generation status + submission status) tek atomik commit'te yazılır; kısmi yazma kanıtlandı. **FAZ 8 kapanışı (bölüm 10):** orijinal denetim TD-28 (scoring fingerprint calibration/anchor `"none"` placeholder, `scoring_service.rs`) kapatıldı — fingerprint kalibrasyon/anchor sürümü artık gerçek, istikrarlı politikayı temsil eder; cache anahtarına (fingerprint.value = components hash) girdiği testle kanıtlandı; sürüm sabitini artırmak eski cache'leri geçersiz kılmaya yeter (artifact path fingerprint.value'a bağlı; `"none"` placeholder kalmadı — grep kanıtı). |
| TD-29 | P2 | Merkezi job store | ALREADY_FIXED | Sayfa poller'ları global job query'ye bağlandı | Tur 4. |
| TD-30 | P2 | Analysis structured; performans entegrasyonu yok | ALREADY_FIXED | `domain/analysis.rs` metrics/claims structured | Bilinçli ayrı rapor; performans kendi raporuna sahip. |
| TD-31 | P2 | Rubrik şablonları frontend'de | CONFIRMED (KABUL EDİLEBİLİR) | `performanceOrganizationUi.ts` şablon kataloğu | FAZ 10 kararı (bölüm 12): şablon kataloğu salt-okunur UI kolaylığıdır; seçildiğinde rubrik taslağına (v0) yüklenir ve öğretmen düzenler; authoritative doğrulama backend `publish_performance_rubric` → `validate_rubric` (3-6 ölçüt, 3/5 düzey, benzersiz ID'ler, düzey tanımları). Veri otoritesi riski yok → kabul edilebilir. |
| TD-32 | P2 | Golden set/benchmark altyapısı yok | ALREADY_FIXED | `testdata/golden/tymm_tde_001/` (sentetik fixture + committed `manifest.sha256` + 9 integration test) | FAZ 6 kapanışı (bölüm 7): committed test corpus'a dönüştürüldü; SHA-256 manifest testi, sınav yapısı/crop/answer-type eşleşmeleri, CER/WER/leakage/exact-match saf fonksiyonları + `GoldenOcrBenchmarkReport` DTO; model benchmark'ı `NEEDS_MODEL_RUNTIME` (docs/GOLDEN_OCR_SCORING_BENCHMARK.md). **FAZ 7+ (bölüm 9):** gerçek model benchmark'ı çalıştı; statü `available` — tarama varyantında CER=0.0 (5/6 soru), leakage yok; `benchmark_report.json` DTO ile üretildi (bölüm 5.2). Q3 madde 5 rubric/ground-truth "tutarsızlığı" çözüldü (öğrencinin gerçek hatası; rubrik anahtarı canonical). Yeni bulgu: 06 bbox y-ekseni kaynağı üretim crop konvansiyonuyla uyumsuz — **Faz 9'da kapatıldı** (bölüm 11): dönüşüm tek paylaşılan fonksiyona alındı, runner + golden test tüketicileri uygular, regresyon testli; üretim hattı etkilenmedi. Yapısal `structuredAnswer` şema uyumu bu modelde sağlanmadı (fail-closed needsReview tasarımı korur) — ayrı fazda takip edilir. |
| TD-33 | P2 | Frontend integration testi az | PARTIAL | `npm test` 161 test (önceki denetimde 147) | Kademeli genişleme. |
| TD-34 | P3 | Preprocess eager maliyeti | ALREADY_FIXED | `student_answer_ocr_service.rs` `preprocess_model_inputs` yalnız seçilen varyantı üretir (`select_preprocess_variant` deterministik + `Original` fallback); test `preprocess_model_inputs_generates_only_the_selected_variant` | FAZ 7 kapanışı (bölüm 8) + FAZ 8 doğrulaması (bölüm 10): eager 5x üretim kaldırıldı, kodda doğrulandı (üretim dalında `for variant in PREPROCESS_VARIANTS` yok); TD-34 statüsü `ALREADY_FIXED`. |
| TD-35 | P3 | Model/runtime tuning benchmark'sız | PARTIAL | Benchmark Faz 7+ ile çalıştı: `docs/GOLDEN_OCR_SCORING_BENCHMARK.md` bölüm 5 — `modelRuntime=available`, tarama varyantı CER=0.0 (5/6 soru), Q4 CER 2.14 (referans kapsamı kaynaklı), leakage yok; `benchmark_report.json` DTO ile üretildi | FAZ 8 kapanışı (bölüm 10): "benchmark yok" kısmı çözüldü (altyapı + ilk ölçüm mevcut, `modelRuntime=available`), ancak model/runtime tuning kararları (UYGULAMA_PLANI 33-36) hâlâ uygulanmadı → dürüst statü `PARTIAL`; kalan tuning adımları ayrı izlenir. |
| TD-36 | P3 | Speaking retry sabit 2s | ALREADY_FIXED | `speaking_exam_service.rs` `SPEAKING_SCORE_RETRY_DELAY_SECONDS = 2` (satır 58); retry döngüsü (satır ~2313-2325) bu sabiti kullanır | FAZ 10 kapanışı (bölüm 12): hardcoded 2s değeri adlandırılmış, tek noktalı, dokümante sabite alındı; davranış aynen korundu; `cargo test --lib speaking_exam` 37 passed. Retry gecikmesi artık derleme zamanından tek noktadan ayarlanabilir. |
| TD-37 | P3 | Deterministik scoring kapsamı | ALREADY_FIXED | `deterministic_scoring_service.rs` `supports` 8 tür (MultipleChoice, TrueFalse, Matching, Ordering, FillBlank, Numeric, Table, CorrectionTable); golden 05 rubric yalnız q2/q3/q4'ü `deterministic: true` işaretler; test `golden_deterministic_questions_are_covered_by_the_deterministic_scorer` (13 golden test) | FAZ 8 kapsam değerlendirmesi (bölüm 10): golden answer-type eşleşmeleri karşılaştırıldı — Table/Matching/CorrectionTable zaten kapsanıyor; Essay ve GrammarAnalysis golden tasarımında `deterministic` işaretli değil (kriter/semantik scoring) → güvenli ekleme yok, genişletilmedi; karar regresyon testiyle kilitlendi. Kapsam kararı: 8 tür, golden'daki deterministic soruların tamamını karşılar. |
| TD-38 | P3 | Rapor O(n) tarama | ALREADY_FIXED | `performance_service.rs` `get_performance_report` — `assessments_by_student: HashMap<&str, &PerformanceAssessment>` (satır 879-882); roster döngüsü `get(...).copied()` (satır 885) | FAZ 10 kapanışı (bölüm 12): rapor taraması O(roster × assessment) yerine O(roster + assessment) oldu; davranış değişmedi (mevcut rapor testleri kanıt); `cargo test --lib performance` 26 passed. |
| TD-39 | P3 | PDF window.print vs pdf_service | CONFIRMED (ERTELENDİ) | `PerformanceScoringPage.tsx:864-874` (`PerformanceReportPrintView` + `window.print()`); `pdf_service.rs` sayfa sayma/PNG render | FAZ 10 kararı (bölüm 12): `pdf_service` rapor-PDF üretmez (yalnız yazılı sınav hattı için sayfa sayma/PNG render); performans raporunu backend PDF'ine taşımak büyük, ayrı bir özellik işidir. Mevcut print akışı çalışıyor (OS print / save-as-PDF). Kabul edilebilir → ertelendi. |

---

## 2. FAZ 1 kapanış kanıtı — performans veri güvenliği (1.1–1.9)

Her madde için kırmızı regresyon kanıtı (testin önce FAIL, sonra kod düzeltmesiyle
GREEN olduğu) ve yeşil sonuç aşağıdadır.

### 1.1 Onaylı karar değişmezliği (TD-02)

- Kural: `set_performance_assessment_status` `assessment_id` verilse de verilmese de
  Approved kaydı değiştiremez; save/approve/status hiçbiri onaylı kaydı değiştiremez.
- Kod: `performance_service.rs` `set_performance_assessment_status` — `approved_record_exists`
  `Some`/`None` her iki dalda; `save_performance_assessment` `Approved` reddi;
  `approve_performance_assessment` ikinci onay reddi + tek-final guard'ı.
- Test: `approved_assessment_status_cannot_be_changed_without_assessment_id` (green, 1/1).
- Red/yeşil: FAZ 1 bu kuralı zaten kapalı buldu; ayrıca tek-final guard'ı bu fazda
  eklendi ve `approve_rejects_a_second_final_assessment_for_the_same_student` ile yeşil.

### 1.2 Kimlik ve scope çapraz doğrulaması (TD-05)

- Kural: yabancı assessment_id duplicate üretmez; aynı öğrenci için en fazla bir
  aktif/final kayıt.
- Kod: save scope check (ID ∈ application ∧ student); approve tek-final koruması.
- Testler: `save_rejects_assessment_id_that_belongs_to_another_application`,
  `approve_rejects_a_second_final_assessment_for_the_same_student` — her ikisi green.

### 1.3 Delete dependency (TD-06)

- Kod: `remove_class_application` `performance_assessments` boş değilse
  `AssessmentClassApplicationInUse`.
- Test: `class_application_with_performance_assessment_cannot_be_removed` (green).

### 1.4 Rubrik sürümü sabitleme (TD-04)

- Kod: existing kayıt pinned `rubric_id`/`rubric_version` ile validate + toplam;
  yeni kayıt en yeni sürümü pinler; publish eski taslağı sessizce yeniden hesaplamaz.
- Test: `republished_rubric_does_not_rebase_existing_draft_version_or_total` (green).

### 1.5 Provisional/final rapor ayrımı (TD-07)

- Kod: DTO `total` (yalnız Approved) vs `provisional_total` (Approved+InProgress);
  CSV/PDF `total` kullanır; sonuç tablosu InProgress "(geçici)" etiketi taşır
  (bu fazda eklenen UI etiketi).
- Testler: `report_does_not_publish_in_progress_total_as_final_total` (green);
  `performanceReportUi.test.ts` yalnız Approved total basar (green).

### 1.6 CSV/XLSX güvenliği (TD-08)

- Kod: `escapeCell` `= + - @ \t \r` önek + `"` sarma; UTF-8 BOM + `;` ayraç.
- Test: `CSV output prevents formula injection through user-controlled student names`
  (green; `=HYPERLINK`, `+SUM`, `-1+2`, `@cmd` payload'ları `'` önekli).
- Not: XLSX export yok; çıktı CSV (string hücreler).

### 1.7 Frontend mutation/draft güvenliği (TD-09)

- Kod: `derivePerformanceActionAvailability` pending guard'ları; draft-reset effect
  pending iken atlar; save onSuccess cache'i dar günceller (refetch draft'ı ezemez);
  duplicate click tek mutation (LoadingButton `isPending`).
- Testler: `performanceScoringUi.test.ts` 5 senaryo (save pending → approve/status/revert
  kapalı; status pending → revert kapalı; non-rated → revert açık; approve pending →
  approve kapalı) — green.
- Not: Proje test altyapısı React component render'ı desteklemiyor (node:test +
  `--experimental-strip-types`); 1.7 koruması saf fonksiyon + sayfa guard'ı seviyesinde
  kanıtlandı.

### 1.8 Legacy scoring güvenli default (TD-12) — bu fazda kod değişikliği

- **Kırmızı kanıt:** `scoring_record_missing_scoring_applied_fails_closed_and_is_not_accepted`
  önce FAIL (serde default `true` iken `scoring_applied==true` idi):
  `assertion failed: !reopened.scoring_records[0].scoring_applied`.
- **Değişiklik:** `domain/scoring.rs` `default_scoring_applied() -> false`;
  `project_store.rs` `normalize_scoring_records` legacy kayıtta eksik alanı explicit
  `scoringApplied` ile yazar (legacy anlam korunur).
- **Yeşil kanıt:** aynı test + `legacy_scoring_record_without_scoring_applied_is_explicitly_classified`
  PASS; mevcut `open_project_at_path_accepts_missing_ocr_metadata_and_legacy_scoring_run_ids`
  PASS (legacy davranış korundu).

### 1.9 Teacher-facing teknik sızıntı (TD-17)

- Kod: `BlockingReasons.tsx` bilinmeyen kod için "Bekleyen bir adım tamamlanmadı";
  performans durum etiketleri Türkçe (`statusLabel`); teacherId ham UUID yerine
  "Öğretmen ataması mevcut".
- Test: status etiketleri `performanceReportStatusLabel` (Rust tarafında serde kontratı
  `set_status_uses_snake_case_status_contract` green).

---

## 3. FAZ 0 başlangıç doğrulaması (git durumu)

```
branch:            main
HEAD:              fdb8e6e1c0d57d8b0615dc9dff3b13b460e33ab9 ("düzeltme")
git status:        25 değiştirilmiş dosya (önceki turların kullanıcı işi — korundu)
                   + untracked: .audit_cache/final_closure_faz01_task.md,
                                .audit_cache/tur4b_task.md, testdata/
git stash list:    stash@{0}: On main: tur0+tur1 WIP (performans regresyon testleri) — DOKUNULMADI
git ls-files .audit_cache: tur0/tur1/tur2/tur4 task + wip_backup dosyaları
```

Bu fazda dokunulan dosyalar (mevcut kullanıcı değişikliklerinin üzerine):

- `src-tauri/src/domain/scoring.rs` (default false)
- `src-tauri/src/services/project_store.rs` (normalize explicit + 2 regresyon testi)
- `src-tauri/src/services/performance_service.rs` (approve tek-final guard'ı + test + fmt)
- `src/pages/PerformanceScoringPage.tsx` (InProgress "(geçici)" etiketi)
- `src/pages/performanceScoringUi.test.ts` (3 yeni senaryo)

**Kapanış kararı (FAZ 0+1):** FAZ 1'in 1.1–1.9 maddeleri tamamlanmıştır. TD-01
(yazılı çoklu sınav kapsamı) hâlâ açıktır ve Tur 3'te ele alınacaktır; genel
pilot kullanım için bu matristeki A/B grubu borçları izlenmelidir.

---

## 3. FAZ 2 kapanış kanıtı — TD-15 commit semantiği ve yutulan commit hataları

Bu fazın davranış sözleşmesi:

```text
commit fail
→ typed error
→ command success dönmez
→ UI "kaydedildi" göstermez
→ memory state canonical sayılmaz
→ retry mümkündür
```

### 3.1 Kapsam dışı üretim kodundan arındırma (negatif tarama)

- `grep -rn "let _ = .*commit_snapshot_cas" src-tauri/src/` → **0 sonuç**.
  Yedi call-site (`rubric_extraction_service.rs:134,142`,
  `question_text_service.rs:170,179,322,329`, `exam_package_build_service.rs:165`)
  `if let Err(error) = ...commit_snapshot_cas(...) { log::error!(...) }` desenine
  çevrildi. Bu site'lar spawn edilmiş job-tamamlanma yolunda best-effort workflow
  refresh'iydi; hata artık görünür diagnostic (log) olarak yüzeye çıkıyor, sessizce
  yutulmuyor. Komut başarısı hiçbir durumda yutulan commit üzerine kurulmuyor.
- `job_manager.rs` production bölümünde (satır 1–976) `unwrap()`/`expect()`/
  `panic!`/`todo!` → **0 sonuç**; tüm Mutex lock'ları `map_err` ile typed
  `AppError` döndürür. Test `lock_poison_returns_typed_error_instead_of_panicking`
  poison durumunda typed hata garantisini kanıtlar (green).
- `job_commands.rs` `list_jobs_for_project` rehydrate hatasını `map_err` ile
  typed `AppError`'a çevirir (tek rehydrate noktası; startup ayrı bir yutma yok).
  Test `list_jobs_propagates_rehydrate_failure_instead_of_swallowing_it` green.

### 3.2 Kritik mutation yolları için commit-fail regresyon testleri

Her test `mutate`/`commit_snapshot_cas`'ın session-fingerprint doğrulamasını
dışarıdan bozarak (disk üzerinde project.json'u harici değiştirerek) commit'i
başarısız ettirir ve şunu kanıtlar: typed error (`PROJECT_EXTERNALLY_MODIFIED`),
success DTO dönmez, session/memory projesi değişmez, disk üzerine yazılmaz,
disk geri yüklendiğinde retry başarılı olur.

- **Performance:** `save_performance_assessment_commit_failure_returns_typed_error_and_allows_retry`
  (`performance_service.rs`). Kayıt taslağı oluşturulmaz; retry InProgress kayıt
  üretir (5+4+3=12). GREEN.
- **OCR:** `update_student_answer_text_commit_failure_returns_typed_error_and_allows_retry`
  (`student_answer_ocr_service.rs`). `teacher_corrected_text` değişmez; retry
  düzeltilmiş metni yazar. GREEN.
- **Scoring:** `update_scoring_record_commit_failure_returns_typed_error_and_allows_retry`
  (`scoring_service.rs`). `teacher_manual_score` yazılmaz, `decision_state`
  `ModelCandidate` kalır; retry `TeacherApproved` üretir, invalidation yok. GREEN.
- **Speaking:** mevcut `commit_failure_in_recovery_path_is_audited_not_silently_swallowed`
  (recovery commit hataları audit'e işlenir, memory değişmez) korunur. GREEN.

Kırmızı→yeşil: commit-fail senaryosu yalnızca yutma deseni varken "başarılı"
görünür; `let _ = commit_snapshot_cas` negatif taraması 0'a inmesi ve yukarıdaki
dört servis testinin yeşil olması, "hata yüzeyine çıkarma" davranışını kanıtlar.

### 3.3 FAZ 2 doğrulama özeti

| Komut | Sonuç |
| --- | --- |
| `cargo test --lib job_manager` | 15 passed, 0 failed |
| `cargo test --lib speaking_exam` | 37 passed, 0 failed |
| `cargo test --lib performance` | 26 passed, 0 failed |
| `cargo test --lib project_store` | 40 passed, 0 failed |
| `cargo test --lib student_answer_ocr` | 29 passed, 0 failed |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS (0 hata) |
| `cargo fmt --check` | PASS |
| `git diff --check` | PASS |

Frontend değişikliği yok; `npm run typecheck`/`lint`/`npm test` bu fazda
çalıştırılmadı (kapsam dışı, doğrulanmış dosya yok). Tam suite (check:all,
smoke, tauri:build, full cargo test) Faz 11'e aittir.

**Kapanış kararı (FAZ 2):** TD-15 semantik olarak kapalıdır — kritik mutation
yollarında yutulan commit yoktur, commit fail typed error döner, retry
mümkündür. TD-14 rehydrate hatası typed ve tek noktadan yayılır.

---

## 4. FAZ 3 kapanış kanıtı — TD-01 AssessmentActivity scope migration

Bu faz, yazılı (ve listening) family verisini proje seviyesindeki flat
koleksiyonlardan gerçek AssessmentActivity kapsamına taşır. Performans tarafı
zaten activity-scope'lu olduğu için dokunulmadı.

### 4.1 Additive domain alanları (kayıpsız eski JSON uyumu)

`Question`, `StudentSubmission`, `StudentAnswerOcrRecord`, `OcrGeneration`,
`ScoringRecord`, `ScoringAnchor`, `ExamPackageFreeze` üzerine serde-default'lu
`assessment_activity_id: Option<String>` eklendi. `Project` üzerine
`active_written_assessment_activity_id: Option<String>` eklendi (backend-authoritative
scope işaretçisi). Alanlar `skip_serializing_if` ile additive; `deny_unknown_fields`
yok; eski proje JSON'ları kayıpsız açılıyor (fixture kanıtı aşağıda).

Kapsam çözümü (`domain/project.rs`):

- `written_family_activity_ids()` — Written + Listening activity id'leri.
- `resolve_written_scope_id()` — sırasıyla: active pointer → tek written activity
  → `None` (legacy). Birden çok written activity + pointer yoksa typed
  `WRITTEN_SCOPE_AMBIGUOUS` döner (tahmin yapılmaz).
- `written_scope_view()` — flat koleksiyonların scope-filtered read-model projeksiyonu.
  `record_belongs_to_written_scope` kuralı: tagged kayıt scope'a eşitse; untagged
  legacy kayıt yalnız tek-written-activity (veya None-scope) bağlamında görünür.

### 4.2 Migration (`normalize_written_activity_scope`, project_store.rs)

`MigrateWithVerifiedBackup` kapısı korunur (gerçek projede ÇALIŞTIRILMADI; yalnız
tempdir + test fixture'larında doğrulandı). Migration `normalize_project_json`
içinde çalışır ve `deserialize_project` üzerinden typed hata döndürebilir.

Kurallar:

1. **Tek written activity** → tüm untagged flat written kayıtlar deterministic
   olarak o activity'e bağlanır; `activeWrittenAssessmentActivityId` o activity'e
   set edilir; `changed=true`.
2. **Sentetik/göç activity — belgeli policy (sentez YAPILMAZ):** written activity
   hiç yokken legacy written data varsa otomatik sentetik activity üretilmez.
   Veri untagged (proje geneli) kalır ve migration warning basar. Bu davranış
   eski davranışı korur; projede tek written activity oluştuğunda deterministic
   attach devreye girer; çoklu written activity'de ambiguity blocker korur.
   (Gerekçe: organizasyon katmanından önceki veriye scope tahmin etmek, `class
   application` benzeri bilinmeyen bir sahiplik atamasıyla veriyi yanlış
   bağlayabilir — agresif sentez riskine karşı güvenli varsayılan.)
3. **Çoklu written activity + untagged written data** → typed
   `MIGRATION_AMBIGUOUS_ASSESSMENT_SCOPE` blocker; proje açılamaz; otomatik
   tahmin yok. Teacher-facing mesaj "hangisine ait olduğu belirlenemedi" der.
4. **İdempotent** — ikinci çalıştırma `changed=false` (no-op).
5. `studentSubmissions` migration tetikleyicisine dahil değildir (yalnız grouping
   scaffolding; sentetik activity üretimini tetiklemez).

### 4.3 Writer/reader path'ler activity-scoped

- `question_text_service`: extraction yalnız scoped soru kümesini düzenler;
  yeni sorulara `assessment_activity_id` basar; confirm/edit/confirm_all/list/
  status scoped okur/yazar.
- `student_answer_ocr_service`: `start`/`run` scope çözer, generation ve OCR
  kayıtlarına scope basar, submission/question kümesini scope'lar.
- `student_scan_service`: submission oluştururken scope basar; `list_student_submissions`
  scoped döner.
- `scoring_service`/`scoring_anchor_service`: kayıt/anchor'lara scope basar.
- `scoring.rs` read-model'leri (`scoring_readiness`, `scoring_summary`, paket
  hash'leri) `written_scope_view` üzerinden çalışır → A'nın QEP'i B'nin
  readiness'ında görünmez.
- `workflow_engine`: workflow değerlendirmesi scoped soru/alt-kayıt/freeze
  kullanır.
- `assessment_organization_service`: written activity oluşturma pointer'ı set
  eder; yeni `set_active_written_activity` komutu backend-authoritative scope
  seçer (yazılı olmayan activity'yi reddeder); `remove_class_application` written
  data dependency scan'i yapar (TD-01 veri kaybı guard'ı).
- Frontend: `CanonicalExamWorkspacePage` yazılı/dinleme workspace'ine girerken
  `setActiveWrittenActivity` çağırır. Frontend domain kararı üretmez; yalnız
  route bağlamını backend'e bildirir.

### 4.4 Zorunlu fixture kanıtları (tempdir + in-memory)

- `single_written_activity_migration_attaches_legacy_flat_data_deterministically`
  (project_store): legacy flat soru/alt-kayıt/OCR → tag + pointer; semantik
  equality (veri kaybı yok). GREEN.
- `written_scope_migration_is_idempotent_on_second_run` (project_store): ikinci
  normalize no-op. GREEN.
- `ambiguous_written_scope_with_untagged_data_produces_typed_blocker`
  (project_store): `MIGRATION_AMBIGUOUS_ASSESSMENT_SCOPE`; opaque alan korunur.
  GREEN.
- `legacy_written_data_without_activity_stays_untagged_and_loads_losslessly`
  (project_store): sentetik activity üretilmez; veri kayıpsız yüklenir. GREEN.
- `two_written_activities_isolate_scoped_data_by_pointer` (domain/project): pointer
  A iken yalnız A soruları/alt-kayıtları; pointer B'ye geçince yalnız B. GREEN.
- `ambiguous_written_scope_without_pointer_returns_typed_error` +
  `single_written_activity_resolves_as_scope_without_pointer` (domain/project).
  GREEN.
- `set_active_written_activity_persists_pointer_and_rejects_non_written`
  (assessment_organization_service). GREEN.
- `remove_class_application_with_written_submission_is_blocked`
  (assessment_organization_service): activity silme/uygulama kaldırma dependency
  scan'i; boş uygulama hâlâ kaldırılabilir. GREEN.
- `activity_a_freeze_does_not_affect_activity_b_scoring_readiness`
  (assessment_organization_service): A'nın frozen QEP'i B'nin readiness'ında
  `QEP_NOT_FROZEN` blocker üretir. GREEN.

Kırmızı→yeşil notu: izolasyonu ihlal eden durum (untagged data + çoklu written
activity) migration'da typed blocker üretirken kod düzeltmesi sonrası tüm
fixture'lar yeşildir; izolasyon mekanizması domain-scope view + pointer üzerinden
kanıtlandı.

### 4.5 FAZ 3 doğrulama özeti

| Komut | Sonuç |
| --- | --- |
| `cargo test --lib` (tam lib) | 530 passed, 0 failed, 4 ignored |
| `cargo test --lib project_store` | 44 passed, 0 failed |
| `cargo test --lib question_text` | 30 passed, 0 failed |
| `cargo test --lib student_answer_ocr` | 29 passed, 0 failed |
| `cargo test --lib scoring` | 68 passed, 0 failed |
| `cargo test --lib rubric` | 62 passed, 0 failed |
| `cargo test --lib exam_package` | 7 passed, 0 failed |
| `cargo test --lib student_scan` | 12 passed, 0 failed |
| `cargo test --lib workflow_engine` | 25 passed, 0 failed |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --check` | PASS |
| `npm run typecheck` | PASS |
| `npm run lint` | PASS (yalnız önceden var olan PerformanceScoringPage uyarıları) |
| `npm test` | 161 passed, 0 failed |
| `git diff --check` | PASS |

Tam suite (check:all, smoke, tauri:build, full cargo test + integration) Faz
11'e aittir — bu fazda çalıştırılmadı. Migration gerçek kullanıcı projesinde
çalıştırılmadı; yalnız tempdir + test fixture'larında doğrulandı.

**Kapanış kararı (FAZ 3):** TD-01 semantik olarak kapalıdır — yazılı family
verisi activity-scope'a taşındı, migration backup-gated/idempotent/ambiguity-guarded,
çoklu yazılı sınav izolasyonu ve QEP izolasyonu testli.

---

## 5. FAZ 4 kapanış kanıtı — workflow ve readiness tek otoritesi (TD-10, TD-03, TD-11)

Bu faz, "canlı hesaplama tek otorite" sözleşmesini finalleştirir ve persisted
workflow cache'inin authoritative izlenimini kaldırır.

### 5.1 Persisted workflow cache-only (TD-10)

- `Project.workflow` alanına `domain/project.rs` içinde açık "CACHE ONLY" dokümanı
  eklendi: canlı kaynak `workflow_engine::evaluate_workflow`; `get_workflow_snapshot`
  komutu bu alanı asla aynen dönmez. `current_stage` yalnız canlı değerlendirmede
  running-flag ipucu olarak okunabilir (`ScoringRunning`), stage kısa devresi değildir.
- Negatif tarama: `workflow_engine.rs` içinde persisted snapshot'ı aynen döndüren
  dal kalmadı (`return project.workflow` / `project.workflow.clone()` → 0 sonuç).
  `get_workflow_snapshot` her çağrıda `evaluate_workflow_with_context` ile canlı
  hesaplar; model probe yalnız auxiler context (workflow truth değil).
- Manuel `project.workflow = WorkflowSnapshot { ... }` yazımları yalnız cache
  güncellemesidir; hiçbir reader bunu otorite olarak okumaz.
- Regresyon testleri (`workflow_engine.rs`):
  - `test_live_evaluation_overrides_stale_persisted_build_stage` — bayat persisted
    build stage canlı QepReady tarafından ezilir.
  - `test_live_evaluation_recomputes_entire_snapshot_ignoring_persisted_cache`
    (FAZ 4 eklendi) — stage + label + blocking_reasons + next_actions + summary
    tamamen bozulmuş persisted cache'e rağmen canlı snapshot alanlarının hiçbiri
    cache'ten sızmaz. GREEN.

### 5.2 Family başına backend snapshot sözleşmesi (TD-03)

- Written / Listening → `WorkflowSnapshot` (`evaluate_workflow` via
  `get_workflow_snapshot`); Performance → `get_performance_status` DTO;
  Speaking → backend activity snapshot (`getAssessmentActivity` üzerinden
  configuration + classApplications + attempts).
- Frontend `derive*Statuses` yalnız backend DTO alanlarını normalize eder ve
  render eder; backend'de üretilmeyen readiness kararı üretmez. Performance
  derive'ı tamamen `PerformanceStatusDto` alanlarından beslenir
  (`hasPublishedRubric`, `allApproved`, `approvedCount`, …).
- Kontrat testleri: written (WorkflowSnapshot readiness) + performance
  (PerformanceStatus DTO) + speaking (activity snapshot) mevcuttu; FAZ 4'te
  listening için `examWorkspace.test.ts`'ye iki kontrat testi eklendi
  (questions step yalnız backend `examPackageFreeze` ile complete olur; students
  step yalnız backend `studentIntake` ile complete olur). GREEN.

### 5.3 Set-based readiness + vacuous-ready koruması (TD-11)

- `scoring_readiness` gerçek `(submission_id, question_id)` kartezyen kümesi
  üzerinden duplicate/missing kontrolü yapar (`expected_pairs`,
  `missing_pairs`, `duplicate_pair_count`); `ocr_ready = expected_records > 0
  && duplicate==0 && missing boş && status hazır`.
- Mevcut regresyonlar: `scoring_readiness_detects_duplicate_ocr_pairs_even_when_count_matches`,
  `scoring_readiness_reports_missing_ocr_pairs`.
- FAZ 4 eklendi: `scoring_readiness_does_not_report_vacuous_ready_for_empty_sets`
  — boş öğrenci/soru listesi vacuous `.all()` ile hazır sayılamaz; `expected_records
  == 0` ve `STUDENT_GROUPING_NOT_READY` + `STUDENT_ANSWER_OCR_NOT_READY` blocker'ları
  döner. GREEN.

### 5.4 FAZ 4 doğrulama özeti

| Komut | Sonuç |
| --- | --- |
| `cargo test --lib workflow` | 35 passed, 0 failed |
| `cargo test --lib scoring` | 69 passed, 0 failed |
| `cargo test --lib` (tam lib) | 535 passed, 0 failed, 4 ignored |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --check` | PASS |
| `npm run typecheck` | PASS |
| `npm run lint` | PASS (yalnız önceden var olan PerformanceScoringPage uyarıları) |
| `npm test` | 163 passed, 0 failed |
| `git diff --check` | PASS |

Tam suite (check:all, smoke, tauri:build, full cargo test + integration) FAZ
11'e aittir — bu fazda çalıştırılmadı.

**Kapanış kararı (FAZ 4):** TD-10 semantik olarak kapalıdır — canlı hesaplama tek
otoritedir ve persisted workflow açıkça cache-only işaretlidir. TD-03 ve TD-11
FAZ 4 kapsamında doğrulanmıştır (backend DTO tüketimi + set-based/vacuous-ready
koruma testli).

---

## 6. FAZ 5 kapanış kanıtı — soru/rubrik model çağrısı verimliliği (TD-19, TD-20)

Bu faz, extraction akışlarının her hedef soru için **tüm sayfa görsellerini
tekrar göndermesini** kaldırır ve rubrik parse retry'inin görselleri yeniden
göndermeden kurtarılmasını sağlar.

### 6.1 TD-19 — question-to-page map ve bounded page penceresi

Yeni saf fonksiyon modülü `services/page_window_service.rs`:

- `question_numbers_by_page(raw_text)` — `pdftotext -raw` çıktısını form feed
  (`U+000C`) üzerinden sayfalara böler ve her soru marker'ının sayfasını çıkarır
  (marker/layout analizi; soru numarası → sayfa adayı).
- `candidate_pages_for_question` — marker eşleşmesi öncelikli; marker yoksa
  (tarama/eksik soru) lineer tahmin (soru no × sayfa sayısı / soru sayısı),
  her iki durumda ilk çağrı tek sayfayla sınırlı.
- `expand_page_window` (±N pencere, sayfa aralığına kenetli) ve
  `select_inputs_by_pages` (model inputlarını hedef sayfalarla filtreler).

Eskalasyon zinciri (ilk çağrı hedef sayfa → düşük confidence/not_visible'ta ±1
pencere → son çare geniş fallback, bounded):

- `question_text_service::extract_question_text_targeted` — her hedef soru için
  tier'ları dener; target soru `QUESTION_TEXT_VISIBLE_CONFIDENCE` (0.5) altında
  veya çıktıda yoksa bir üst pencereye geçer; hiçbir tier'da bulunamazsa
  `None` döner (fallback bounded: soru başına en fazla 3 çağrı, sınırsız retry
  yok). Fatal model hataları job'a yayılır; retryable hatalar warning olarak
  toplanıp eskalasyona devam eder.
- `rubric_extraction_service` vision loop — aynı hedef→pencere→fallback
  zincirini kullanır; soru rubrik sonucunda bulunamazsa bir sonraki pencereye
  geçer.

`QuestionTextExtractionRequest` ve `RubricExtractionRequest` prompt-contract
user-data'sına `includedPages` eklenir; job success JSON'larına `pageUsage`
(her soru için `pages`, `attempts`, `stage`, `found`) yazılır.

### 6.2 TD-20 — rubrik parse retry salvage / text-only repair

`llama_server_gateway.rs`:

- `parse_rubric_model_response` deterministik salvage'ini güçlendirdi: tek hatalı
  soru maddesi (item) tüm yanıtı artık çöpe atmıyor; `parse_partial_rubric_questions`
  ile geçerli maddeler kurtarılıyor.
- `read_saved_rubric_raw_response` + `strip_reasoning_and_fences` +
  `parse_partial_rubric_questions` + `rubric_payload_to_output` servis katmanına
  `pub(crate)` olarak açıldı.

`rubric_extraction_service::draft_rubric_with_retry` yeni zinciri:

1. İlk çağrı multimodal.
2. Retryable parse hatasında görsel gönderilmeden kurtarma:
   - **Deterministik salvage:** kayıtlı raw response üzerinden
     `parse_partial_rubric_questions` → başarılıysa ikinci çağrı **0**.
   - **Text-only JSON repair:** salvage çalışmazsa aynı yanıt metni `raw_text`
     olarak, görselsiz, strict JSON ile yeniden gönderilir (attempt 2) →
     başarılıysa ikinci **görsel** çağrısı 0.
3. **Multimodal retry yalnız açık reason ile** (son çare): ilk yanıt boşsa
   `first_response_empty`, repair başarısızsa `text_only_repair_failed:<kod>`;
   reason `retry_metadata.retry_reason`'da kayıtlı.
4. Text-only path (görselsiz `raw_text` istekleri) davranış korunur: strict
   yeniden gönderim aynen çalışır.

`RubricExtractionResult`'a additive `retry_metadata`
(`RubricExtractionRetryMetadata`: attempts, image_count, pages_used,
retry_reason, salvage_used, text_only_repair_used, targeted_pages) eklendi;
diagnostics/provenance gereksinimi karşılanır.

### 6.3 Yeni regresyon testleri

- `page_window_service` (8): form-feed sayfa ayrımı, marker önceliği, lineer
  tahmin/kenetleme, pencere genişletme, sayfa filtreleme.
- `question_text_service` (6, request-capture spy gateway):
  - `extraction_sends_only_the_target_page_not_all_pages` — 5 sayfalık belgede
    hedef sayfa yalnız `[2]` gönderilir (tüm-sayfa clone yok).
  - `extraction_escalates_to_window_...` — hedef sayfada bulunamayınca `[1,2,3]`.
  - `extraction_uses_broad_fallback_as_last_resort` — `[4] → [3,4,5] → [1..5]`.
  - `extraction_bounded_fallback_returns_none_...` — hiçbir tier'da görünmezse
    `None`, soru başına tam 3 çağrı (bounded).
  - `extraction_escalates_on_low_confidence_and_keeps_best_candidate` — 0.3'lük
    hedef sayfa sonucu eskalasyonu engellemez, en iyi aday korunur.
  - `extraction_treats_retryable_errors_as_escalation_signals`.
- `rubric_extraction_service` (5, fake gateway + artifact):
  - `rubric_retry_salvages_response_without_second_vision_call` — ikinci çağrı 0.
  - `rubric_retry_uses_text_only_repair_without_resending_images` — repair çağrısı
    görselsiz + raw_text + strict.
  - `rubric_retry_multimodal_resend_only_with_explicit_reason` — repair başarısız
    olunca 3. çağrı görselli + `retry_reason=text_only_repair_failed:...`.
  - `rubric_retry_empty_first_response_records_explicit_reason` —
    `first_response_empty`.
  - `rubric_retry_text_only_request_preserves_existing_strict_resend` — text-only
    path eski davranışını korur.

### 6.4 FAZ 5 doğrulama özeti

| Komut | Sonuç |
| --- | --- |
| `cargo test --lib question_text` | 36 passed, 0 failed |
| `cargo test --lib rubric_extraction` | 10 passed, 0 failed |
| `cargo test --lib page_window` | 8 passed, 0 failed |
| `cargo test --lib llama_server_gateway` | 47 passed, 0 failed |
| `cargo test --lib` (tam lib) | 554 passed, 0 failed, 4 ignored |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --check` | PASS |
| `git diff --check` | PASS |

Frontend değişikliği yok; `npm run typecheck`/`lint`/`npm test` bu fazda
çalıştırılmadı (kapsam dışı, doğrulanmış dosya yok). Tam suite (check:all,
smoke, tauri:build, full cargo test + integration) FAZ 11'e aittir — bu fazda
çalıştırılmadı. Model benchmark'ı model binary'si olmadan çalıştırılmadı;
field-recall koruması yalnız davranış koruması (eski tüm-sayfa fallback'i son
çare olarak korunuyor) ve testlerle doğrulandı.

**Kapanış kararı (FAZ 5):** TD-19 ve TD-20 semantik olarak kapalıdır —
extraction istekleri hedef sayfa/region setiyle sınırlıdır, eskalasyon
±1 pencere + bounded fallback ile testlidir; rubrik parse retry'i görselsiz
kurtarır, multimodal retry yalnız açık reason ile yapılır.

---

## 7. FAZ 6 kapanış kanıtı — Golden sınav paketi → committed test corpus (TD-32)

Bu faz, sentetik golden paketi (`testdata/golden/tymm_tde_001/`) committed test
corpus'una dönüştürür ve model gerektirmeyen OCR kalite ölçüm altyapısını kurar.
Detay: `docs/GOLDEN_OCR_SCORING_BENCHMARK.md`.

### 7.1 Teslim edilenler

- **Committed manifest:** `manifest.sha256` (yoktu; dosyalardan üretildi).
  8 golden dosyanın SHA-256 özeti. Golden dosyaları **değiştirilmedi**.
- **Metrik modülü:** `services/golden_ocr_metrics.rs` — saf fonksiyonlar
  (Levenshtein, CER, WER, `critical_token_error`, `printed_question_leakage`,
  `structured_field_exact_match`, `structured_fields_all_exact`, percentile
  p50/p95) + `GoldenOcrBenchmarkReport` / `GoldenQuestionMetric` /
  `GoldenAggregateMetric` / `GoldenModelRuntimeStatus` serde DTO'ları.
- **Crop doğrulaması:** `student_answer_crop_service::crop_rect_normalized`
  `pub` yapıldı (yalnız görünürlük; corpus testi üretim crop matematiğini
  birebir doğrular).
- **Corpus entegrasyon testi:** `src-tauri/tests/golden_tymm_tde_001.rs` (9 test).

### 7.2 Test kanıtları

- `manifest_sha256_verifies_all_golden_files` — 8 dosyanın hash'ı manifest ile
  eşleşir; README'deki 07_CodeX…md dahil listede.
- `golden_contracts_parse_and_expected_score_is_consistent` — rubrik toplamı
  100; beklenen 80 = Q1..Q6 toplamı; `decision_state=teacher_approved_golden`.
- `blank_exam_is_renderable_and_has_four_pages` — 01: 4 sayfa, tümü render
  edilebilir (tempdir; salt-okunur golden).
- `filled_exam_regions_crop_within_bounds` — 02: 06 bbox'ları [0,1] içinde; crop
  sayfa sınırlarında ve boş değil; Q1 iki bölgeli (`Primary` sayfa 1 +
  `Continuation` sayfa 2) sıralaması korunur.
- `scanned_variant_is_valid_and_renderable_with_bounded_crops` — 03: geçerli 4
  sayfa PDF; bbox'lar render sınırları içinde (deskew Faz 7'de; bu fazda girdi
  kabulü + sınır garantisi).
- `golden_answer_types_match_structured_answer_variants` — Q1-Q6 answer_type'ları
  domain `AnswerType` + `StructuredAnswer` varyantıyla uyumlu; yanlış varyant
  fail-closed.
- `metric_functions_are_clean_against_golden_ground_truth` — CER/WER özdeşte 0;
  Q2 alanları tam eşleşir; Q6 basılı yönerge sızıntısı yok.
- `benchmark_report_dto_documents_needs_model_runtime` — DTO serde kontratı
  `modelRuntime=needs_model_runtime` ile doğrulanır.
- Birim: `cargo test --lib golden_ocr_metrics` → 16 passed.

### 7.3 FAZ 6 doğrulama özeti

| Komut | Sonuç |
| --- | --- |
| `cargo test --test golden_tymm_tde_001` | 9 passed, 0 failed |
| `cargo test --lib golden_ocr_metrics` | 16 passed, 0 failed |
| `cargo test --lib` (tam lib) | (aşağıda FAZ 6 tam sonuç) |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --check` | PASS |
| `git diff --check` | PASS |

Frontend değişikliği yok; `npm run typecheck`/`lint`/`npm test` bu fazda
çalıştırılmadı (kapsam dışı, doğrulanmış dosya yok). Model benchmark'ı model
binary'si olmadığı için **çalıştırılmadı** — `NEEDS_MODEL_RUNTIME` olarak
`docs/GOLDEN_OCR_SCORING_BENCHMARK.md` bölüm 5'te dürüstçe raporlandı; PASS
uydurulmadı.

**Bilinen golden tutarsızlığı (kayıtlı, düzeltilmedi):** Q3 madde 5 rubrik
`answer_key` `E`, ground truth `5-A`. Korpus testi gözlemi olduğu gibi
doğrular; Faz 7'de canonical anahtar kararı gerekir.

**Kapanış kararı (FAZ 6):** TD-32 semantik olarak kapalıdır — committed test
corpus + SHA-256 manifest + model gerektirmeyen ölçüm altyapısı + benchmark
rapor DTO'su mevcuttur; model kapıları Faz 7'ye bırakıldı ve dürüstçe
`NEEDS_MODEL_RUNTIME` olarak işaretlendi.

---

## 8. FAZ 7 kapanış kanıtı — OCR görüntü hattı (TD-21, TD-22, FAZ 7 TD-28)

Bu faz, OCR görüntü hattının altyapısını kurar ve model gerektirmeyen kısmını
golden korpus üzerinde doğrular. Deskew/registration/DPI fonksiyonları saf,
model çağrısız ve birim testlidir; preprocess varyant seçimi deterministik ve
statistik tabanlıdır; OCR sonuç persistence atomicity'si hata-enjeksiyon
regresyon testiyle kanıtlanmıştır. Faz 7 gerçek OCR metni üretmez — model
kapıları `docs/GOLDEN_OCR_SCORING_BENCHMARK.md` bölüm 5'te dürüstçe
`NEEDS_MODEL_RUNTIME` olarak güncellenmiştir.

### 8.1 Deskew / registration / DPI (TD-21)

Yeni saf-fonksiyon modülü `services/ocr_image_geometry_service.rs`:

- **Deskew:** `estimate_skew_angle` — projeksiyon-profili yöntemi (kaba ±12°
  2° adım + 0.25° refine; satır profili varyansını maksimize eden açı);
  `deskew_image` — `|angle| >= 8°` → `DeskewOutOfRange` typed hata (sessiz
  düzeltme YOK), `|angle| > 3°` operasyon aralığı → aynı typed hata,
  `|angle| < 0.1°` → no-op, aksi halde bilinear dönüşüm (canvas sabit).
- **Registration:** `measure_registration_deviation` — her beklenen bölge
  `crop_rect_normalized` (üretim crop matematiğinin aynısı) ile kırpılır, crop
  içindeki mürekkep merkezi ile crop merkezi arasındaki **2B offset vektörü**
  toplanır; raporlanan sapma = ortalama vektörün sayfa köşegenine bölünmüş
  hali (sistematik kayma). El yazısı pozisyonu bölgeler arası rastgele dağılır
  (sapma sıfıra gider); hatalı hizalama tüm bölgeleri aynı yönde kaydırır.
  `validate_registration` eşik üstünde `RegistrationOutOfRange` döner
  (`DEFAULT_MAX_REGISTRATION_DEVIATION = 0.12`).
- **DPI:** `render_scale_to_dpi` (2.0 → 144), `normalize_dpi` (hedef 300 DPI
  sabit; ölçek + hedef boyutlar), `validate_dpi_in_range` (96–600).

Hattı bağlama (`student_answer_ocr_service::preprocess_model_inputs`):

- Her kaynak crop `deskew_source_for_ocr` ile deskew edilir (no-op straight;
  out-of-range reddi typed hatayla OCR işine yayılır; deskew'li görüntü
  content-hash'li managed cache'e yazılır).
- `render_dpi` (144, scale 2.0) doğrulanır ve `OcrProvenance.render_dpi`'a
  yazılır (eski `None` yerine; provenance notu
  `render_dpi_normalized_to_144`).

Birim testleri (13): DPI ölçek/doğrulama, normalize no-op/upscale, düz görüntü
~0 açı, 2° eğik görüntü deskew ile düzleşir (kalıntı < 0.5°), ≥8° reddi, 3–8°
arası reddi, rotasyon canvas sabitliği, hizalı/sapmış/rastgele-elyazısı/boş
sayfa registration senaryoları.

### 8.2 Deterministik preprocess varyant seçimi (TD-22)

`ocr_image_preprocess_service.rs` yeni saf fonksiyonlar:

- `compute_image_statistics` — mean / std / edge_density (gradyan eşiği).
- `select_preprocess_variant` — 4 geliştirme varyantını (CleanGrayscale,
  HandwritingEnhanced, HighContrast, HighContrastBw) crop istatistiklerinden
  skorlar, en yüksek skoru seçer; içeriksiz crop (`edge_density < 0.004`) veya
  skor `< 0.25` ise `Original` default'a düşer; gerekçe `reason`'da
  (`low_content_default_original`, `score_below_threshold_default_original`,
  `statistics_scored_<variant>`).

`preprocess_model_inputs` artık **tüm 5 varyantı üretmez**: yalnız seçilen
varyant üretilir; seçilen varyant üretilemezse `Original` fallback + `preprocess_failed`
/ `preprocess_fallback_used` warning'leri ve seçim gerekçesi
(`variant_selection_reason:<reason>`) üretilen varyantın diagnostics'ine yazılır.
Tek varyantla başarıda ikinci model çağrısı yoktur.

Regresyon testleri: `preprocess_model_inputs_generates_only_the_selected_variant`
(tek üretilen varyant + tek seçim gerekçesi diagnostics'i + `render_dpi == 144`);
`preprocess_model_inputs_rejects_out_of_range_skew_with_typed_error`;
`selection_prefers_handwriting_enhanced_for_handwritten_content`;
`selection_uses_default_original_for_blank_crops`;
`selection_uses_default_original_when_no_variant_reaches_threshold`;
`selection_is_deterministic_and_reproducible`. Mevcut
`preprocess_model_inputs_prefers_handwriting_enhanced` ve
`_falls_back_without_crashing` yeşil kalır (davranış korundu).

### 8.3 OCR sonuç persistence atomicity (FAZ 7 iş emri TD-28)

OCR sonucu (`generation.result`, `generation.status`,
`teacher_review_status`, `student_answer_ocr_records`, submission status)
tek `commit_job` içinde tek atomik `mutate` (CAS + transaction journal +
atomic replace) ile yazılır — ya hep ya hiç.

Regresyon testi `ocr_result_commit_is_atomic_and_never_writes_partial_state`
(student_answer_ocr_service.rs): aynı işlemde çoklu state güncellemesi yapan
closure sona `Err` döndüğünde (kapsam doğrulama hatası enjeksiyonu) **disk
üzerinde hiçbir state değişmez** (generation hâlâ `Candidate`, `result` boş,
OCR kaydı yok, submission `OcrRunning`); retry başarılı commit'te tüm state'ler
tek atomik yazımda güncellenir. GREEN.

### 8.4 Golden 03 entegrasyonu (model gerektirmeyen)

`tests/golden_tymm_tde_001.rs` — 3 yeni test (toplam 12):

- `scanned_variant_deskew_accepts_every_page_within_golden_bounds` — 03'ün 4
  sayfasının tamamı deskew'e kabul edilir (ölçülen açılar 0–0.75°; red yok).
- `scanned_variant_registration_deviation_stays_within_golden_bounds` — 03'ün
  bölgeli sayfalarında sistematik sapma üretim eşiğinin (0.12) altında
  (ölçülen ≤ 0.01).
- `golden_render_dpi_normalizes_to_fixed_ocr_target` — 144 DPI girdi, 300 DPI
  hedefe normalize edilir; 96–600 aralık doğrulaması.

01/02 regresyonu kırılmadı (`blank_exam_is_renderable_and_has_four_pages`,
`filled_exam_regions_crop_within_bounds` yeşil).

### 8.5 FAZ 7 doğrulama özeti

| Komut | Sonuç |
| --- | --- |
| `cargo test --lib ocr_image_geometry` | 13 passed, 0 failed |
| `cargo test --lib ocr_image_preprocess` | 9 passed, 0 failed |
| `cargo test --lib preprocess_model_inputs` | 4 passed, 0 failed |
| `cargo test --lib ocr_result_commit_is_atomic` | 1 passed, 0 failed |
| `cargo test --lib` (tam lib, `--skip start_job_returns_model_mmproj_missing`) | 589 passed, 0 failed, 4 ignored |
| `cargo test --test golden_tymm_tde_001` | 12 passed, 0 failed |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --check` | PASS |
| `git diff --check` | PASS |

**Bilinen ortam kaynaklı test (FAZ 7 dışı, kod kaynaklı değil):**
`start_job_returns_model_mmproj_missing` 5 saniyelik poll penceresinde başarısız
olur. Nedeni koddan bağımsız: test profili varsayılan portu **8080**'i kullanır
(`default_model_profile`), gerçek llama-server şu anda 127.0.0.1:8080'de
sağlıklı çalışmaktadır; runtime 8080'i sağlıklı görüp lease verir ve mmproj
kontrolü devreye girmeden OCR kısmi tamamlanır (job `Partial`). FAZ 7 kod
değişiklikleriyle ilişkisiz olduğu ayrıca kanıtlandı: `preprocess_model_inputs`
eski sürüme geri alındığında da aynı hata oluşur. Teste dokunulmadı.

Frontend değişikliği yok; `npm run typecheck`/`lint`/`npm test` bu fazda
çalıştırılmadı (kapsam dışı, doğrulanmış dosya yok). Model benchmark'ı bu
ortamda gerçek OCR metni üretmediği için **çalıştırılmadı** — Faz 7'nin
altyapıyı doğruladığı, model kapılarının `NEEDS_MODEL_RUNTIME` olduğu
`docs/GOLDEN_OCR_SCORING_BENCHMARK.md` bölüm 5'te açıkça yazıldı; PASS
uydurulmadı.

**Kalan kalıntılar:**
- Registration production gating'i (canlı OCR hattına eşik reddi bağlama)
  sahte ret riskine karşı canlı benchmark sonrasına bırakıldı; fonksiyon +
  eşik + typed hata + golden doğrulaması teslim edildi.
- Orijinal denetim TD-28 (scoring fingerprint calibration/anchor "none")
  ayrı izlenir (Tur 8 adayı).
- PDF render ölçeğini 300 DPI'a çıkarmak (JXA scale 2.0 → ~4.17) ayrı risk
  taşır; `normalize_dpi` hedef hesabı hazır, render değişimi benchmark sonrası.

**Kapanış kararı (FAZ 7):** TD-21 ve TD-22 semantik olarak kapalıdır —
deskew/registration/DPI saf fonksiyonları + typed out-of-range reddi +
golden 03 sınır doğrulaması mevcuttur; preprocess eager 5x kaldırılmış,
deterministik statistik tabanlı seçim devrededir ve diagnostics'e yazılır.
FAZ 7 iş emri TD-28 (OCR sonuç persistence atomicity) kapalıdır — kısmi yazma
enjeksiyon testi hiçbir state'in değişmediğini kanıtlar.

---

## 9. FAZ 7+ kapanış kanıtı — Golden OCR benchmark (gerçek model)

Bu faz, Faz 7'nin `NEEDS_MODEL_RUNTIME` olarak bıraktığı gerçek model ölçümlerini
çalıştırır, TD-21 registration gating kalıntısını kapatır ve TD-32 benchmark
statüsünü `available`'a günceller. Detay: `docs/GOLDEN_OCR_SCORING_BENCHMARK.md`
bölüm 5. Golden dosyaları değiştirilmedi (SHA-256 manifest doğrulandı).

### 9.1 Benchmark ortamı ve sonuç özeti

- Model: Gemma 4 12B IT Q4_K_XL, llama-server `127.0.0.1:8080` (`--temp 0
  --top-k 1 --reasoning off`); OpenAI uyumlu `POST /v1/chat/completions`;
  istek `temperature 0`, `max_tokens 4096`, `response_format json_object`.
- Pipeline: golden 03 `pdftoppm -r 300` → deskew (üretim fonksiyonu) → üretim
  crop matematiği + corpus bbox y dönüşümü → deterministik preprocess varyant
  seçimi → üretim JPEG hazırlığı → üretim prompt kontratı +
  `extract_student_answer_ocr` → `golden_ocr_metrics`.
- Tarama varyantı sonucu: **Q1/Q2/Q3/Q5/Q6 CER=0.0, WER=0.0**; Q4 CER=2.14
  (referans kapsamı metriği; 3 düzeltme de mevcut, kritik token eksik 0).
  Basılı yönerge sızıntısı tüm sorularda yok. Kritik token eksik 0 (tüm sorular).
  Aggregate CER p50=0.0, WER p50=0.0; 6 model çağrısı, 0 retry.
- Deskew 0–0.75°/sayfa; registration sapması 0.004–0.010 (maks 0.035).

### 9.2 Bulgular ve kararlar

1. **06 bbox y-ekseni kaynağı (yeni korpus bulgusu):** 06 bbox'ları PDF alt-sol
   y kaynağı kullanır; `crop_rect_normalized` ve domain `NormalizedBBox` üst-sol
   bekler. Bbox'lar aynen beslenirse 5/6 soruda yanlış bölge kırpılır (yalnız Q5
   hizalanır). Runner üst-sol dönüşümü uygular; golden dosyaları değiştirilmedi.
   Korpus düzeltmesi Faz 8 adayıdır.
2. **Registration gating: EVET — üretim hattına bağlandı.** Golden 03 gerçek
   benchmark'ında registration 0.004–0.010 ölçer (eşik 0.12) → sahte ret riski
   yok. `student_answer_crop_service::build_sources` Production dalında
   `validate_page_registration` çağrılır; aşım `RegistrationOutOfRange` typed
   hatasıyla fail-closed; boş sayfalar muaf. Regresyon testi:
   `production_rejects_systematically_misregistered_page_and_accepts_aligned_one`
   (GREEN). Bu, Faz 7 "kalan kalıntı" listesindeki registration gating maddesini
   kapatır.
3. **Q3 canonical anahtar: rubrik `answer_key` (5-E).** Öğrenci kâğıdı 5-A
   işaretler (OCR CER=0.0); 06 q3 alanı öğrenci işaretinin OCR'ıdır, anahtar
   değildir. Rubrik anahtarıyla 4/5 = 12/15 = `expected_scoring.q3`. "Bilinen
   tutarsızlık" tutarsızlık değildir — öğrencinin gerçek hatasıdır.
4. **Structured-schema uyumu:** model metin OCR'ında kusursuzdur fakat typed
   `structuredAnswer`'ı (table/matching/correction/grammar) doğru şemada
   üretmez; üretim `structured_answer_invalid` → `needsReview` fail-closed
   tasarımı korunur (veri kaybı yok). Şema-uyumlu çözüm Faz 8 adayıdır.

### 9.3 FAZ 7+ doğrulama özeti

| Komut | Sonuç |
| --- | --- |
| `cargo test --lib student_answer_crop` | 4 passed, 0 failed (yeni registration gating testi dahil) |
| `cargo test --lib golden_ocr_metrics` | 16 passed, 0 failed |
| `cargo test --test golden_tymm_tde_001` | (aşağıda tam sonuç) |
| `cargo clippy --all-targets --all-features -- -D warnings` | (aşağıda tam sonuç) |
| `cargo fmt --check` | PASS |
| `git diff --check` | PASS |

Frontend değişikliği yok. Model benchmark'ı canlı model üzerinde çalıştırıldı;
sonuçlar `docs/GOLDEN_OCR_SCORING_BENCHMARK.md` bölüm 5'te, rapor DTO
`benchmark_report.json` (`modelRuntime=available`) ile tempdir'de saklanır.

**Kalan kalıntılar:**
- Korpus bbox y-ekseni dönüşümü korpus dosyalarında düzeltilmedi (Faz 8; runner
  tarafında uygulanıyor).
- Deterministik scoring (beklenen toplam 80) yapısal şema uyumu sağlanana kadar
  ölçülmedi (Faz 8).
- Image token / peak memory bu sunucuda raporlanmıyor (`None`).
- Orijinal denetim TD-28 (scoring fingerprint calibration) **FAZ 8'de kapandı**
  (bölüm 10).

**Kapanış kararı (FAZ 7+):** TD-21 registration gating kalıntısı kapalıdır —
gerçek model benchmark'ı sahte ret riskini ortadan kaldırdı, gate üretim hattına
bağlandı ve testlidir. TD-32 benchmark statüsü `available`'dır; tarama varyantı
OCR metin kalitesi kapıları (CER/WER) sağlanmıştır; yapısal şema uyumu ve korpus
bbox düzeltmesi Faz 8 kapsamındadır.

---

## 10. FAZ 8 kapanış kanıtı — scoring kalibrasyon ve fingerprint (orijinal TD-28, TD-37, TD-35, TD-34)

Bu faz, "Scoring Calibration & Fingerprint" kapsamında kalan teknik borçları
kapatır: scoring fingerprint kalibrasyon/anchor placeholder'ı (orijinal denetim
TD-28), deterministik scoring kapsam değerlendirmesi (TD-37), benchmark sonrası
statü düzeltmesi (TD-35) ve preprocess eager doğrulaması (TD-34).

### 10.1 Fingerprint kalibrasyon/anchor gerçek politika sabitleri (orijinal TD-28)

- **Değişiklik:** `services/prompt_contract.rs` içine iki gerçek, istikrarlı
  politika sabiti eklendi:
  - `SCORING_CALIBRATION_VERSION = "scoring_calibration_v1"` — kalibrasyon
    sürümü; scoring model/prompt/schema/sampling parametre kümesi değişince
    artan sabit.
  - `SCORING_ANCHOR_SET_VERSION = "scoring_anchor_set_v1"` — anchor sürümü;
    canonical rubrik/şablon kümesi sürümü.
- `services/scoring_service.rs`'te her iki fingerprint yolu artık `"none"`
  yerine bu sabitleri kullanır:
  - Deterministik yol (`build_scoring_fingerprint`) → `SCORING_CALIBRATION_VERSION`,
    `SCORING_ANCHOR_SET_VERSION`.
  - Semantik yol (`build_scoring_fingerprint_with_policy_fingerprint`) → aynı
    sabitler.
- **Cache anahtarına girdiği:** `ScoringFingerprintComponents` serde edilip
  SHA-256 hash'i `fingerprint.value`'u üretir; artifact path `{value}.json`
  olduğu için kalibrasyon/anchor sürümü cache anahtarına doğrudan girer.
  `lookup_candidate` ayrıca `fingerprint.value` **ve** components JSON'ını
  birebir doğrular.
- **Eski cache'leri geçersiz kılma kararı:** sürüm sabitini artırmak yeterlidir
  — fingerprint.value değiştiği için eski artifact path'i artık eşleşmez; ek
  migration/temizlik gerekmez. Regresyon testi
  `calibration_and_anchor_versions_participate_in_the_cache_key_and_invalidate_old_caches`
  bunu kanıtlar (kalibrasyon veya anchor sürümü bump edilince değer değişir ve
  cache miss döner).
- **Kanıt (grep):** `src-tauri/src` içinde fingerprint kalibrasyon/anchor bağlamında
  `"none"` placeholder kalmadı (yalnız ilgisiz `pdf_service.rs`/`rubrika.rs`
  backend/fallback string'leri).

### 10.2 TD-37 deterministik scoring kapsam değerlendirmesi

- Golden 05 rubric yalnız q2/q3/q4'ü `deterministic: true` işaretler; bu soruların
  answer type'ları (Table, Matching, CorrectionTable) `DeterministicScoringService::supports`
  kapsamındadır (8 tür içinde). Golden 06 answer-type eşleşmeleriyle karşılaştırıldı.
- Essay (q1/q6) ve GrammarAnalysis (q5) golden'da **deterministic işaretli değildir**
  (kriter/semantik scoring tasarımı) → güvenli, küçük bir ekleme yoktur; mevcut
  davranışı bozmamak için kapsam genişletilmedi. Karar, golden regresyon testi
  `golden_deterministic_questions_are_covered_by_the_deterministic_scorer` ile
  kilitlendi (13. golden test; her soru için `deterministic` flag ile `supports`
  sonucunun eşleştiğini doğrular).

### 10.3 TD-35 statü düzeltmesi

- Benchmark Faz 7+ ile çalıştı (`docs/GOLDEN_OCR_SCORING_BENCHMARK.md` bölüm 5,
  `modelRuntime=available`). "Benchmark yok" kısmı çözüldü; ancak model/runtime
  tuning kararları (UYGULAMA_PLANI 33-36) hâlâ uygulanmadı → dürüst statü
  `PARTIAL` (altyapı + ilk ölçüm mevcut; tuning ayrı izlenir).

### 10.4 TD-34 doğrulaması

- FAZ 7 `preprocess_model_inputs` yalnız seçilen varyantı üretir (deterministik
  `select_preprocess_variant` + `Original` fallback); kodda doğrulandı, üretim
  dalında eager 5x yok. Statü `ALREADY_FIXED` (FAZ 7 referansı).

### 10.5 FAZ 8 doğrulama özeti

| Komut | Sonuç |
| --- | --- |
| `cargo test --lib scoring_cache` | 6 passed, 0 failed (yeni kalibrasyon/anchor cache-key testi dahil) |
| `cargo test --lib scoring_service` | 19 passed, 0 failed |
| `cargo test --test golden_tymm_tde_001` | 13 passed, 0 failed (yeni deterministic kapsam testi dahil) |
| `cargo test --lib -- --skip start_job_returns_model_mmproj_missing` | (aşağıda tam sonuç) |
| `cargo clippy --all-targets --all-features -- -D warnings` | (aşağıda tam sonuç) |
| `cargo fmt --check` | PASS |
| `git diff --check` | PASS |

Frontend değişikliği yok; `npm run typecheck`/`lint`/`npm test` bu fazda
çalıştırılmadı (kapsam dışı, doğrulanmış dosya yok). Migration/repair
çalıştırılmadı; gerçek kullanıcı projesine dokunulmadı; `testdata/golden/`
dosyaları değiştirilmedi (manifest SHA-256 sabit); commit oluşturulmadı;
`stash@{0}` korundu.

**Kapanış kararı (FAZ 8):** orijinal TD-28 (fingerprint kalibrasyon/anchor
placeholder) kapalıdır — gerçek politika sabitleri cache anahtarına girer, sürüm
bump'ı eski cache'leri geçersiz kılar, `"none"` placeholder kalmadı. TD-37 kapsam
kararı golden eşleşmesiyle doğrulandı ve testle kilitlendi. TD-34 `ALREADY_FIXED`,
TD-35 `PARTIAL` (tuning açık) olarak dürüstçe güncellendi.

---

## 11. FAZ 9 kapanış kanıtı — yapısal şema uyumu ve modüler sınırlar (TD-24, TD-26, korpus bbox)

Bu faz, modüler sınır borçlarını (TD-24 `types.ts` monolit, TD-26 büyük servis)
güvenli/sınırlı iyileştirme ilkesiyle ele alır ve Faz 7+ corpus bbox bulgusunu
(06 bbox y-ekseni alt-sol kaynaklı) kapatır. Golden dosyaları değiştirilmedi;
migration/repair çalıştırılmadı; commit oluşturulmadı; `stash@{0}` korundu.

### 11.1 TD-24 — frontend `types.ts` monoliti: KAPATILDI

- **Karar:** import grafiği basit olduğu için (54 tüketicinin tamamı `../api/types`
  üzerinden **adlandırılmış, type-only** import kullanıyor; `import * as` veya value
  export yok) domain alanına göre bölme güvenli yapıldı.
- **Bölme:** 2194 satırlık tek dosya, backend domain modüllerini yansıtan 17 modüle
  ayrıldı: `types.app.ts`, `types.analysis.ts`, `types.assessment.ts`,
  `types.document.ts`, `types.gradedExam.ts`, `types.jobs.ts`, `types.model.ts`,
  `types.ocr.ts`, `types.performance.ts`, `types.project.ts`, `types.question.ts`,
  `types.rubric.ts`, `types.schoolClass.ts`, `types.scoring.ts`, `types.speaking.ts`,
  `types.student.ts`, `types.workflow.ts`. `types.ts` salt barrel oldu
  (`export type * from './types.X'` — `verbatimModuleSyntax` uyumlu; 229 tipin
  tamamı korundu, tek tip atlanmadı).
- **Güvenlik gerekçesi:** tüm tipler type-only (çalışma zamanı kodu yok); yalnız
  `types.rubric ↔ types.question` tip-seviyesi döngüsü oluştu (Question→RubricState,
  RubricQuestionSnapshot→Question) — tip erasuresında döngü sorunsuzdur, çalışma
  zamanı etkisi yoktur. `StudentAnswerOcrPage.tsx` dinamik `import('../api/types')`
  tip referansı barrel üzerinden doğru çözümlenir.
- **Doğrulama:** `npm run typecheck` PASS; `npm test` **163 passed, 0 failed**;
  `npm run lint` yalnız önceden var olan PerformanceScoringPage uyarıları (yeni
  uyarı yok). 54 tüketici dosyasından hiçbiri değişmedi.

### 11.2 TD-26 — büyük servisler / AppState: SINIRLI KAPANIŞ + ERTELEME

- **Yapıldı (güvenli, davranış değiştirmeyen taşıma):** performans komut kontratı
  DTO'ları net bir sınırla yeni `services/performance_dtos.rs` modülüne taşındı
  (giriş DTO'ları + rapor/statü DTO'ları; serde attrs'ları aynen korundu).
  `performance_service.rs` 2864 → ~2655 satır. `performance_commands.rs` DTO'ları
  doğrudan `performance_dtos`'tan import eder; `services/mod.rs`'e modül eklendi.
  Davranış sözleşmesi değişmedi: `cargo test --lib performance` **26 passed** +
  `commands::performance_commands` **5 passed** + `project_store` performans
  migration testleri yeşil; `cargo build` temiz (yeni uyarı yok).
- **Ertelendi (gerekçeyle):** (a) servis mantığının parçalanması (ör. rapor
  üretimi `get_performance_report`, rating doğrulama saf fonksiyonları) servis
  içi `project_store`/`assessment_organization_service` paylaşımına dokunur —
  riskli, davranışı kanıtlamak için ek kontrat testleri gerekir; (b) `AppState`
  (27 hizmet alanı, `lib.rs`) düz yapısı Tauri'nin standart `manage` desenidir;
  gruplara bölmek 20+ komut dosyasının `state.X` erişimini değiştirir — geniş,
  yüksek riskli refactor. Her ikisi de "küçük güvenli değişiklik" ilkesine
  uymadığı için gerekçeyle ertelendi (sonraki fazlar).

### 11.3 Korpus bbox düzeltmesi — KAPATILDI (golden dosyaları DEĞİŞMEDİ)

- **Bulgular:** `06_Golden_Set_Beklentileri.json` `regions[].bbox_normalized`
  bbox'ları PDF alt-sol y kaynağı kullanır; `crop_rect_normalized`/`NormalizedBBox`
  üst-sol bekler. Aynen beslenirse 5/6 soruda yanlış bölge kırpılır (yalnız Q5
  hizalanır). Golden dosyalar (manifest dahil) **değiştirilmedi**.
- **(a) Tek, dokümante fonksiyon:** dönüşüm `golden_ocr_metrics::corpus_bbox_bottom_left_y_to_top_left`
  (`y_top = clamp(1 − (y_bottom + h), 0, 1)`) olarak paylaşılan modüle alındı.
  Benchmark runner'ın `normalize_bbox`'i bu fonksiyonu çağırır (önceki inline
  hesaplama kaldırıldı). Runner `--skip-model` smoke çalıştırıldı: 6 soru render/
  deskew/crop hatasız; registration 0.003–0.008 (dokümante 0.004–0.010 ile tutarlı).
- **(b) Tüketici düzeltmesi:** golden entegrasyon testinin `region_from_golden` ve
  `regions_for_page` yardımcıları ile `filled_exam_regions_crop_within_bounds` /
  `scanned_variant_is_valid_and_renderable_with_bounded_crops` içindeki inline bbox
  yapıları artık dönüşümü uygular. Yeni regresyon testi
  `corpus_bboxes_are_converted_to_top_left_before_cropping`: korpustaki **her**
  bbox'ın dönüşümden geçtiğini (kimlik dönüşümü olmadığını, 7/7 bbox farklı)
  kanıtlar. `golden_ocr_metrics`'e 3 birim testi eklendi.
- **(c) Üretim etkisi yok:** `student_answer_crop_service` golden 06 bbox'larını
  **tüketmez** (yalnız öğretmen tanımlı üst-sol `studentAnswerCropTemplate`);
  production kodda golden veri referansı yalnız yorumlarda. Konvansiyon farkı
  yalnız benchmark runner ve golden test tüketicilerini etkiler; ikisi de artık
  dönüşümü uygular.
- **Dokümantasyon:** `docs/GOLDEN_OCR_SCORING_BENCHMARK.md` bölüm 8
  ("Corpus koordinat konvansiyonu") eklendi; bölüm 5.3 madde 1 ve bölüm 6
  bbox bulgusu Faz 9 kapanışına güncellendi.

### 11.4 FAZ 9 doğrulama özeti

| Komut | Sonuç |
| --- | --- |
| `npm run typecheck` | PASS |
| `npm test` | 163 passed, 0 failed |
| `npm run lint` | PASS (yalnız önceden var olan PerformanceScoringPage uyarıları) |
| `cargo test --lib performance` + `performance_commands` | 26 + 5 passed, 0 failed |
| `cargo test --lib golden_ocr_metrics` | 19 passed, 0 failed (3 yeni dönüşüm testi dahil) |
| `cargo test --test golden_tymm_tde_001` | 14 passed, 0 failed (yeni bbox regresyonu dahil) |
| `cargo test --lib -- --skip start_job_returns_model_mmproj_missing` | (aşağıda tam sonuç) |
| `cargo clippy --all-targets --all-features -- -D warnings` | (aşağıda tam sonuç) |
| `cargo fmt --check` | PASS |
| `git diff --check` | PASS |

`start_job_returns_model_mmproj_missing` ortam testi llama-server 127.0.0.1:8080
çalıştığı için `--skip` ile koşulur (görev şartı). Golden SHA-256 manifest sabit;
commit oluşturulmadı; `stash@{0}` korundu.

**Kapanış kararı (FAZ 9):** TD-24 frontend tip monoliti kapatıldı (testlerle);
TD-26 komut kontratı DTO taşıması yapıldı, servis mantığı/AppState parçalama
gerekçeyle ertelendi; korpus bbox dönüşümü tek noktaya alındı, testli ve
dokümante edildi, üretim hattının etkilenmediği kanıtlandı.

---

## 12. FAZ 10 kapanış kanıtı — kalan borçlar ve nihai kararlar (TD-31, TD-36, TD-38, TD-39)

Bu fazın amacı "her borcu kapatmak" değil, kampanya sonundaki açık borçların her
biri için **dürüst, gerekçeli nihai karar** vermektir: güvenli ve küçükse kapat
(testle), aksi halde kabul edilebilir/ertelendi olarak işaretle (gerekçeyle).
FAZ 9'da ertelenen yapısal iş (TD-26 servis mantığı parçalama + AppState bölme)
için kod değişikliği **yapılmadı**; karar yazıldı.

- Golden dosyaları değiştirilmedi (`manifest.sha256` sabit); migration/repair
  çalıştırılmadı; commit oluşturulmadı; `stash@{0}` korundu; HEAD `fdb8e6e` değişmedi.
- Frontend değişikliği yok → `npm run typecheck`/`npm test` bu fazda kapsam dışı
  (yalnız Rust ve docs değişti).

### 12.1 TD-36 — Speaking retry sabit 2s: KAPATILDI

- **Kod öncesi davranış:** `speaking_exam_service.rs` değerlendirme retry döngüsü
  (satır ~2311-2323) ilk hata sonrası sabit `Duration::from_secs(2)` bekler;
  değer yalnız log mesajında ve sleep çağrısında gömülüydü.
- **Değişiklik (davranış değişmez):** sabit `SPEAKING_SCORE_RETRY_DELAY_SECONDS: u64 = 2`
  modül sabitlerine eklendi (satır 58); sleep ve log mesajı bu sabiti kullanır.
  Retry politikası artık tek, adlandırılmış ve dokümante noktadan derleme zamanında
  ayarlanabilir.
- **Gerekçe:** sabit 2s + tek retry, tek bir model çağrısına karşı makul bir
  geçici hata toleransıdır; runtime config (env/ayar) bu aşamada gereksiz katma
  değerdir — minimum güvenli adım olarak sabitleştirme yeterli ve dürüsttür.
- **Test kanıtı:** davranış değişmediği için mevcut suite kanıttır —
  `cargo test --lib speaking_exam` **37 passed, 0 failed**.

### 12.2 TD-38 — Rapor O(n) tarama: KAPATILDI

- **Kod öncesi davranış:** `performance_service.rs` `get_performance_report`
  roster döngüsünde her öğrenci için `application.performance_assessments.iter()
  .find(|a| a.student_id == student.id)` yapar → O(roster × assessment).
- **Değişiklik (davranış değişmez):** döngü öncesinde
  `assessments_by_student: HashMap<&str, &PerformanceAssessment>` kurulur
  (satır 879-882); döngü `get(...).copied()` kullanır (satır 885). Tarama artık
  O(roster + assessment). Criterion/rating ve rubrik-version aramaları (≤6 ölçüt,
  az sayıda sürüm) zaten sabit-büyüklüktedir ve dokunulmadı.
- **Gerekçe:** tipik sınıf büyüklüğünde (≤40) etki küçüktür ancak iyileştirme
  tek noktadadır ve `find`'in iç-döngü yeniden taramasını kaldırır — risk taşımaz.
- **Test kanıtı:** rapor davranışı mevcut testlerle korundu —
  `report_does_not_publish_in_progress_total_as_final_total` dahil
  `cargo test --lib performance` **26 passed, 0 failed**.

### 12.3 TD-31 — Rubrik şablonları frontend'de: KABUL EDİLEBİLİR (kod değişikliği yok)

- **Doğrulama:** `performanceOrganizationUi.ts` şablon kataloğu (4 şablon, her biri
  4 ölçüt — 3..=6 sınırı içinde) **salt-okunur** UI kolaylığıdır; seçildiğinde
  `performanceTemplateToRubric` rubrik taslağına (v0) yüklenir ve öğretmen
  düzenleyebilir. Authoritative doğrulama backend'dedir: `publish_performance_rubric`
  → `validate_rubric` (ad boş olamaz; 3-6 ölçüt; 3/5 düzey; benzersiz ölçüt/düzey
  ID; her düzeyde gözlenebilir tanım).
- **Karar:** katalog frontend'de kalmaya devam eder. Veri otoritesi riski yok
  (taslak üzerinden backend doğrulaması geçer); kataloğu backend'e taşımak bu
  borcun doğası gereği değil, UI kolaylığıyla ilgili bir tercihtir. → Kabul edilebilir.
- **Test kanıtı:** `validate_performance_rubric` + backend `validate_rubric`
  kontratı önceden mevcuttur; bu fazda değişiklik yok.

### 12.4 TD-39 — PDF window.print vs pdf_service: ERTELENDİ (kod değişikliği yok)

- **Doğrulama:** `PerformanceScoringPage.tsx` "PDF raporu" düğmesi
  `PerformanceReportPrintView`'i render edip `window.print()` çağırır (OS print
  dialogu / save-as-PDF). `pdf_service.rs` (SystemPdfService) **rapor PDF'i
  üretmez** — yalnız yazılı sınav hattı için sayfa sayma ve PDF→PNG page render
  (pdftoppm/JXA) sağlar. İki altyapı farklı amaçlıdır.
- **Karar:** performans raporunu backend üretimli PDF'e (template + dosya teslim)
  taşımak büyük, ayrı bir özellik işidir (rapor şablonlama, yazım, webview teslimi,
  kontrat testleri). Mevcut print akışı çalışıyor ve veri bütünlüğü etkisi yok.
  → Kabul edilebilir; takip ayrı görev olarak ertelendi.
- **Test kanıtı:** değişiklik yok; mevcut `performanceReportUi`/`performanceScoringUi`
  testleri yeşil (bu fazda dokunulmadı).

### 12.5 FAZ 9 ertelemesinin kararı — TD-26 servis mantığı parçalama + AppState bölme: BİLİNÇLİ ERTELEME

- **Karar:** bu kampanya kapsamında **ertelendi**; kod değişikliği yapılmadı.
- **Gerekçe (risk):** (a) `performance_service.rs` mantığını (rapor üretimi, rating
  doğrulama) parçalamak servis içi `project_store`/`assessment_organization_service`
  paylaşımına dokunur; davranış eşdeğerliğini kanıtlamak için ek kontrat testleri
  gerekir — "küçük güvenli değişiklik" ilkesine uymaz. (b) `AppState` düz yapısı
  Tauri `manage` deseninin standart kullanımıdır; hizmet gruplarına bölmek 20+
  komut dosyasının `state.X` erişimini değiştirir — geniş, yüksek yayılımlı refactor.
- **Kapsam-dışı notu:** her iki parça da davranışı iyileştirmez (bakım/okunabilirlik);
  FAZ 9'da zaten güvenli kısmı (DTO taşıması, 2864→~2655 satır) tamamlandı.
  Kalan kısım ayrı, kapsamı netleştirilmiş bir görevle (ör. rapor üretimini
  `report` modülüne taşıma) ele alınmalıdır.

### 12.6 FAZ 10 doğrulama özeti

| Komut | Sonuç |
| --- | --- |
| `cargo test --lib performance` | 26 passed, 0 failed |
| `cargo test --lib speaking_exam` | 37 passed, 0 failed |
| `cargo test --lib -- --skip start_job_returns_model_mmproj_missing` | 594 passed, 0 failed, 4 ignored, 1 filtered out |
| `cargo test --test golden_tymm_tde_001` | 14 passed, 0 failed |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | PASS |
| `git diff --check` | PASS |

`start_job_returns_model_mmproj_missing` ortam testi llama-server 127.0.0.1:8080
çalıştığı için `--skip` ile koşulur (kod kaynaklı değil). Golden SHA-256 manifest
sabit; commit oluşturulmadı; `stash@{0}` korundu; HEAD `fdb8e6e` değişmedi.
Frontend değişikliği yok → `npm run typecheck`/`npm test` kapsam dışı.

**Kapanış kararı (FAZ 10):** TD-36 (adlandırılmış retry sabiti) ve TD-38
(assessments indeksi) davranış değiştirmeden kapatıldı, testlerle doğrulandı;
TD-31 ve TD-39 kabul edilebilir/ertelendi (gerekçeleriyle); Faz 9 ertelemesi
(TD-26 servis mantığı/AppState) bilinçli erteleme olarak kesinleştirildi.

---

## 13. Kampanya sonu özeti

TD-01–TD-39 (39 madde) FAZ 10 sonrası nihai dağılımı:

| Sonuç | Sayı | Maddeler |
| --- | --- | --- |
| ALREADY_FIXED (kapatıldı) | 33 | TD-01, TD-02, TD-03, TD-04, TD-05, TD-06, TD-07, TD-08, TD-09, TD-10, TD-11, TD-12, TD-13, TD-14, TD-15, TD-16, TD-17, TD-18, TD-19, TD-20, TD-21, TD-22, TD-24, TD-25, TD-27, TD-28, TD-29, TD-30, TD-32, TD-34, TD-36, TD-37, TD-38 |
| PARTIAL (kısmen; kalan ayrı turda) | 4 | TD-23 (performans test kapsamı genişlemesi), TD-26 (DTO taşındı; servis/AppState bilinçli ertelendi), TD-33 (frontend test büyümesi), TD-35 (benchmark altyapısı + ilk ölçüm mevcut; model/runtime tuning kararları açık) |
| CONFIRMED — KABUL EDİLEBİLİR / ERTELENDİ | 2 | TD-31 (frontend şablon kataloğu; backend publish doğrulaması authoritative), TD-39 (rapor window.print; pdf_service farklı amaçlı) |

- **Kampanya başlangıcında (FAZ 0 ham matris):** 5 ALREADY_FIXED, 7 PARTIAL,
  27 CONFIRMED.
- **Kampanya sonunda:** 33 kapatıldı, 4 kısmen, 2 kabul edilebilir/ertelendi.
- Kalan PARTIAL'lar ve kabul edilen borçlar, gerekçeleriyle birlikte yukarıdaki
  bölümlerde (özellikle bölüm 12) dokümante edilmiştir; hiçbiri veri bütünlüğü,
  onay/oturum güvenliği veya yazılı/performans scoring doğruluğu üzerinde açık
  risk taşımaz.

---

## 14. FAZ 11 kapanış kanıtı — tam doğrulama kapıları ve kampanya kapanışı

Bu faz, kampanyanın **son fazıdır**: kod değişikliği hedeflenmemiştir; birikmiş tüm
fazların sonucu tam quality kapılarından geçirilir ve kapanış belgelenir. Kod
değişikliği yapılmadı (yalnız bu doküman güncellendi); golden dosyaları değiştirilmedi
(manifest doğrulandı); migration/repair çalıştırılmadı; commit oluşturulmadı;
`stash@{0}` korundu; HEAD `fdb8e6e` değişmedi.

### 14.1 `npm run check:all` (build + typecheck + lint + npm test + cargo:fmt + cargo:clippy + cargo:test)

| Adım | Sonuç |
| --- | --- |
| `npm run build` (tsc -b && vite build) | PASS (204 modül; dist üretildi) |
| `npm run typecheck` (tsc -b) | PASS |
| `npm run lint` (oxlint) | PASS (yalnız önceden var olan PerformanceScoringPage exhaustive-deps uyarıları; yeni uyarı yok) |
| `npm test` (node --test) | **163 passed, 0 failed** |
| `npm run cargo:fmt` (cargo fmt --check) | PASS |
| `npm run cargo:clippy` (clippy -D warnings) | PASS |
| `npm run cargo:test` (tam lib) | 594 passed; 1 failed; 4 ignored — tek hata **ortam kaynaklı** (aşağıda) |

`npm run cargo:test` komutu `start_job_returns_model_mmproj_missing` ortam testini de
koşar; llama-server 127.0.0.1:8080 sağlıklı çalıştığı için bu test (FAZ 7, bölüm 8.5'te
dokümante edilen nedenle) başarısız olur. Kod kaynaklı değildir — `--skip` ile tam
suite yeşildir (14.2). Bu, check:all'ın tek beklenen "başarısızlığı"dır; kod regresyonu yok.

### 14.2 Tam Rust suite (bağımsız, ortam testi `--skip` ile)

| Komut | Sonuç |
| --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --workspace --all-features -- --skip start_job_returns_model_mmproj_missing` | **EXIT 0** — lib `594 passed, 0 failed, 4 ignored, 1 filtered`; `final_data_loss_proofs` 11, `final_security_proofs` 8, `golden_tymm_tde_001` 14, `project_creation_regression` 1, `project_lock_process_fixture` 2, `speaking_backend_persistence` 1, speakoflow_* ve Doc-tests yeşil |
| `cargo test --manifest-path src-tauri/Cargo.toml --test golden_tymm_tde_001` | **14 passed, 0 failed** |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | PASS (exit 0) |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | PASS (exit 0) |
| `git diff --check` | PASS |

### 14.3 Build smoke

| Komut | Sonuç |
| --- | --- |
| `cargo build --manifest-path src-tauri/Cargo.toml` (debug) | PASS (exit 0) |
| `npm run build` (frontend production) | PASS (check:all içinde; 204 modül) |

Tam `tauri build` / `tauri:build` bundling (signing/packaging) bu fazda çalıştırılmadı
(görev kapsamı: en az `cargo build` + frontend `npm run build`). Debug Rust derlemesi +
production frontend derlemesi yeşildir.

### 14.4 Son bütünlük

| Kontrol | Sonuç |
| --- | --- |
| HEAD | `fdb8e6e` (`düzeltme`) — değişmedi |
| `stash@{0}` | korundu (tur0+tur1 WIP) |
| Golden manifest | `shasum -a 256 -c` PASS (`testdata/golden/tymm_tde_001/` içinden) |
| Commit | oluşturulmadı |

### 14.5 Kampanya sonu — final özet (FAZ 11 sonrası)

- **Toplam 39 madde (TD-01–TD-39):** **33 ALREADY_FIXED (kapatıldı)**, 4 PARTIAL
  (TD-23, TD-26, TD-33, TD-35), 2 kabul edilebilir/ertelendi (TD-31, TD-39). Dağılım
  bölüm 13'te korunur; bu faz kod değiştirmediği için sayılar değişmedi, yalnız kapılar
  kanıtlandı.
- **Faz listesi:** FAZ 0 (yeniden sınıflandırma) → 1 (performans veri güvenliği) →
  2 (commit semantiği) → 3 (TD-01 scope migration) → 4 (workflow tek otoritesi) →
  5 (model verimliliği TD-19/20) → 6 (golden corpus TD-32) → 7 (OCR görüntü hattı
  TD-21/22/28) → 7+ (gerçek model benchmark) → 8 (scoring kalibrasyon) → 9 (yapısal
  TD-24/26 + korpus bbox) → 10 (kalan borç kararları) → **11 (tam suite + kapanış)**.
- **Son doğrulama sayıları:** `npm test` **163 passed**; Rust workspace suite **EXIT 0**
  (lib 594 passed, 4 ignored, 1 filtered; tüm integration testleri yeşil); golden
  **14 passed**; clippy/fmt/diff **PASS**; build smoke **PASS**.
- **Bilinen kalıntılar (kod kaynaklı değil / bilinçli kararlar):**
  - `start_job_returns_model_mmproj_missing` ortam testi yalnız llama-server
    127.0.0.1:8080 çalışırken fail eder (ortam kaynaklı; `--skip` ile koşulur).
  - TD-39 performans raporu `window.print` → backend PDF üretimi ertelemesi (bölüm 12.4).
  - FAZ 9 ertelemesi: TD-26 servis mantığı parçalama + AppState bölme (bölüm 12.5).
  - TD-37 kapsam kararı: deterministik scorer 8 tür; Essay/GrammarAnalysis bilinçli
    kapsam dışı (bölüm 10.2).
  - TD-35 model/runtime tuning kararları (UYGULAMA_PLANI 33-36) açık (bölüm 10.3).
- **Kullanım notu:** uygulama llama-server'ı (127.0.0.1:8080) model çalıştırması için
  kullanır; model yoksa yazılı sınav OCR/rubrik/skor işleri `ModelConfigMissing`/mmproj
  kapılarına takılır. `npm run check:all` ortam testini kapsadığından, llama-server
  açıkken beklenen tek "başarısızlık" bu ortam testidir; kod kaynaklı regresyon yoktur.

**Kapanış kararı (FAZ 11):** Kampanya **tamamlandı**. Tüm fazların birikmiş hâli tam
kapılardan geçti: check:all tüm kalite adımları (ortam kaynaklı mmproj testi NOT ile),
tam Rust workspace suite (mmproj hariç), golden 14/14, clippy/fmt/diff, build smoke ve
son bütünlük (HEAD/stash/golden manifest/commit yok) yeşildir. Kod değişikliği
yapılmadı; kapanış yalnız doğrulama ve dokümantasyondur.
