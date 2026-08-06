# Tur 2 — Mimari sadeleştirme: workflow tek otorite (RubrikaV3 performans değerlendirme)

Proje: /Users/kadir/Desktop/RubriKa/RubrikaV3 (branch: main, HEAD 2060ed8 — performans özelliği cherry-pick ile main'e taşındı)
Referans: `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` (bölüm 6: TD-03, TD-10, TD-11; bölüm 32: Tur 2 tanımı)

## Durum (Tur 0 ve Tur 1 tamamlandı)

- Tur 0: 7 regresyon testi eklendi ve kırmızıydı; Tur 1'de yeşile döndü (TD-02/04/05/06/07/08/09 + TD-17 + clippy + smoke).
- Çalışma ağacında Tur 0/1 değişiklikleri duruyor (commit edilmedi). Bu görev onların ÜZERİNE eklenir.
- Görev dosyaları: `src-tauri/src/services/performance_service.rs` (2192 satır, Tur 0 testleri dahil), `src/app/examWorkspace.ts` (derivePerformanceStepStatuses), `src-tauri/src/services/workflow_engine.rs`, `src-tauri/src/domain/scoring.rs` (scoring_readiness).

## Bu görevin kapsamı — 4 madde

### 1. Performance readiness backend'e alın (TD-03)
- Backend'e salt-okunur authoritative snapshot komutu/DTO'su ekle: `get_performance_status` (veya benzeri). Önerilen DTO alanları:
  - `hasPublishedRubric: bool`
  - `publishedRubricVersion: Option<u32>`
  - `totalStudents: u32`, `approvedCount: u32`, `inProgressCount: u32`, `missingCount: u32`, `notPerformedCount: u32`
  - `allApproved: bool` (approvedCount >= totalStudents && totalStudents > 0)
- Kaynak veri: activity.performance_details.rubric_versions + class applications'ın performance_assessments'ı (performans servisindeki mevcut okuma yollarını kullan; yeni servis ekleme — performance_service.rs içine salt-okunur metod).
- Komut katmanı: `performance_commands.rs` içine typed komut (input: project_id + activity_id; çıktı: DTO). Command kontrat testi ekle.
- Frontend: `derivePerformanceStepStatuses` (examWorkspace.ts:396-470) ARTIK bu DTO'yu tüketmeli; ham `performanceDetails`/`classApplications` türetimi kaldırılmalı. DTO alanlarıyla birebir eşleşen adımlar (task: hasPublishedRubric; assessment: allApproved; results: approvedCount > 0). DTO gelmiyorsa güvenli fallback: "blocked" + RUBRIC_MISSING benzeri mesaj.
- Mevcut davranış korunmalı: "blocked (rubrik yayınlanmadan)" ve "ready (results approvedCount > 0)" semantiği aynı kalır, yalnız karar backend'den gelir.

### 2. Frontend domain kararları kaldırılsın (TD-03 devamı)
- `derivePerformanceStepStatuses` içindeki tüm domain kararları (rubrik yayın mı, onay tamam mı, rapor hazır mı) backend DTO'suna devredildikten sonra helper ya sadece DTO'yu render eder ya da tamamen kaldırılır. Frontend'de kalan hiçbir yerde `performance_assessments.length`/`rubricVersions` üzerinden "hazır mı" hesabı kalmasın.
- examWorkspace.ts'de performans için kullanılan veri yollarını DTO ile değiştir; TypeScript tiplerini `src/api/types.ts`'e ekle (performans status DTO tipi).

### 3. Scoring readiness gerçek (submission_id, question_id) kümesiyle (TD-11)
- `scoring.rs:648-751` `scoring_readiness` fonksiyonu: count-only eşitlik (`len() == expected_records`) yerine **set-based coverage**:
  - Beklenen ikili küme: tüm (submission_id, question_id) kombinasyonları (student_submissions × questions).
  - Gerçek kayıtlardan (submission_id, question_id) çiftlerini topla; duplicate çift varsa readiness'ı bloke et (typed Blocker/ErrorCode ekle).
  - Beklenen kümenin her elemanı gerçek kümede olmalı; eksik varsa readiness false + eksik çift listesi structured olarak dönsün (DTO'da `missingPairs: Vec<(String, String)>` gibi).
  - Mevcut TeacherApproved && !needs_review koşulları korunur.
- Testler: duplicate kayıt içeren fixture'da readiness false (mevcut kodda count eşitliği true verebilir); eksik çiftte false; tam kümede true. Bu testler önce kırmızı olabilir, sonra düzeltmeyle yeşil — TDD akışını kullan (test yaz → düzelt → yeşil).

### 4. Workflow tek otorite (TD-10)
- `workflow_engine.rs:384-400` kısa devresi kaldırılsın: persisted `project.workflow` snapshot'ı "olduğu gibi döndürmek" yerine her çağrıda canlı `evaluate_workflow` hesaplansın. Persisted workflow yalnız cache/geriye uyum olarak kalabilir ama karar verici olarak KULLANILMASIN.
- Elle WorkflowSnapshot yazan servisler (student_scan_service.rs:1407-1547, exam_package_build_service.rs:142-464, student_answer_crop_service.rs:852) — dokunma, yalnız kısa devreyi kaldır; eğer bu servislerin yazdığı snapshot artık okunmuyorsa mevcut testlerin kırılmadığını doğrula (yazılı/speaking akışları değişmemeli).
- Frontend: `deriveExamStepStatuses`/`resolveNextExamStep`/`NextActions` backend workflow snapshot'ına dayanıyor (zaten öyle); doğrula ve dokunma.
- workflow_engine için birim testler: kısa devre kaldırıldıktan sonra canlı hesabın persisted snapshot'tan farklı sonuç üretebildiği bir fixture testi (ör. snapshot eski stage'deyken canlı hesap yeni stage üretir) + mevcut workflow testlerinin yeşil kalması.

## Dokunulmayacaklar
- Tur 0/1 testleri ve düzeltmeleri (TD-02/04/05/06/07/08/09, TD-17, clippy düzeltmeleri) — DEĞİŞTİRİLMEZ, yalnız üzerine ekleme yapılır.
- TD-01 (yazılı veri scope'u — Tur 3), TD-12 (legacy default — kullanıcı onayı), Tur 4+ maddeleri (TD-13/14/15/16/18/25/27).
- Migration kodu, backup/restore davranışı.
- Kapsam dışı refactor yok; 4 maddenin dışına çıkma.

## Doğrulama (koşturulacaklar — sözleşme istisnası: bu görev test gerektirir)
1. `cargo test --manifest-path src-tauri/Cargo.toml performance` — mevcut 21 test + yeni status/readiness testleri yeşil.
2. `cargo test --manifest-path src-tauri/Cargo.toml scoring` — scoring readiness testleri yeşil.
3. `cargo test --manifest-path src-tauri/Cargo.toml workflow` — workflow testleri yeşil.
4. `npm test` — mevcut 149 + yeni frontend testleri yeşil (yeni test dosyası ekliyorsan `package.json` test script listesine EKLEMEYİ UNUTMA).
5. `npm run lint`, `npm run typecheck`.
6. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` — exit 0 (Tur 1'de temizlendi, burada yeni hata üretme).
7. `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
8. `npm run check:all` — tam doğrulama (yaklaşık 8 dk).

## Çıktı formatı
STATUS / SUMMARY (4 madde için ayrı ayrı: ne yapıldı, hangi dosya) / CHANGED_FILES / VALIDATION (her komutun sonucu) / RISKS / NEXT_ACTION

## Kurallar
- Türkçe kullanıcı mesajları ve hata metinleri (mevcut kod Türkçe safeMessage kullanıyor — aynı deseni izle).
- Hiçbir koşulda git commit/add/push/stash yapma; değişiklikleri yalnız çalışma ağacında bırak.
- `cargo fmt` çalıştır (kendi kodun için), `cargo clippy` yeni hata üretmesin.
- AGENTS.md kurallarına uy: küçük kapsam, typed hatalar, production'da unwrap/panic yok (test kodu hariç), UI'da teknik gösterim yok.

---

# ÇALIŞMA SÖZLEŞMESİ (her oturumda geçerli)

- **Değişiklik izni:** Yalnız görevde listelenen dosyaları/alanları değiştir. Kapsam dışı refactor, dosya taşıma, isim değişikliği, "iyileştirme" yapma.
- **Değiştirme yasağı:** Mevcut production davranışını, migration'ları, kullanıcı verisi formatını değiştirme. Yalnız görevin gerektirdiği davranış değişikliğini yap.
- **Test/gate koşturma:** Yalnızca görevde açıkça istenmişse koştur. Koşturduysan sonuçları raporda listele. Yüksek süreli kapılar (cargo test, check:all) için istenen hedefli testleri koş.
- **Commit yasağı:** Hiçbir koşulda git commit, amend, push, pull, merge, rebase, cherry-pick, stash, branch oluşturma/silme yapma. Çalışma ağacında bırak.
- **Hata ve yarıda kesme:** Bir hata takılırsa 2-3 denemeden sonra dur, kısmi durumu raporla, devam etme.
- **Kapsam dışı:** Görevle ilgisiz dosyalara dokunma. İlgisiz bir sorun görürsen raporda not et, çözme.
- **İstek sınırı:** Görev başına tek istek. Ara soru sorma; kendi kararını ver, riske girme, raporla.
- **Dil:** Kullanıcı mesajları, audit ve hata mesajları Türkçe. Kod yorumları mevcut stile uygun.
- **Ortam:** macOS, Rust+TS. opencode kendi kök dizinini kullanır; proje dışı dosyalara erişim gerekirse görevde belirtilir.
- **Yanlışlıkla kapsam ihlali:** Fark edersen geri al, raporda belirt.
