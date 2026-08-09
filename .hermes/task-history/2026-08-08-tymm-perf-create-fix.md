# 2026-08-08 — TYMM "Görevi oluştur" regresyon düzeltmesi

## Contract özeti
- **Task type:** bugfix (frontend wiring regresyonu)
- **Backend:** antigravity (agy 1.1.11) — kullanıcı explicit: "bu iş emrini antigravitye yaptır, 3.6 flash high, workspace çalışsın"
- **Model:** `Gemini 3.6 Flash (High)` (kullanıcı kısaltması '36. flash high'; smoke test ile doğrulandı)
- **Policy:** WORKSPACE
- **Kök neden (kanıtlanmış):** UX refactor'unda readiness'tan rubrik koşulu kaldırıldı (backend kontratıyla tutarlı — `initial_rubric` opsiyonel); ancak gradeLevel'sız sınıf seçiminde `missingFields` boş kalırken `mutationFn` ham `throw new Error('Form tamamlanmadı')` fırlatıyordu → `ErrorBanner` `safeMessage` bulamıyor → tıklama sessiz, görev oluşmuyor.

## Routing gerekçesi
- FAST PATH geçerli (task type + backend + model + policy explicit; capabilities.json güncel, agy 1.1.11 eşleşiyor).
- `--sandbox` headless'ta "unsandboxed permission auto-denied → no output produced" üretti → settings.json zaten WORKSPACE izinlerini verdiğinden flag'siz çalıştırıldı (bilinen tuzak, adapter'a işlendi).

## Sonuç
- **Değişen:** performanceOrganizationUi.ts (derivePerformanceCreateReadiness / buildCreatePerformanceTaskInput / executePerformanceCreateSubmit — create gate tek kaynak), PerformanceOrganizationPage.tsx (gate entegrasyonu, form grid sarmalayıcı, değerlendirme butonu rubrik gate'i), errors.ts (normalizeAppError ham Error → teacher-safe), LoadingButton.tsx (loadingText), index.css (.assessment-form-grid textarea), package.json (test listesi), performanceOrganizationUi.test.ts (8 regresyon testi).
- **Doğrulama (Hermes tarafından tekrar çalıştırıldı):** typecheck ✓ · npm test 181/181 ✓ · lint 0 error / 6 warning (hepsi pre-existing; 0 yeni) ✓ · build ✓
- Backend'e dokunulmadı; kullanıcı stash'ı korundu; commit yapılmadı.
- Canlı Tauri manuel acceptance: kullanıcı tarafında (11-C → 1. Dönem → 1. görev → oluştur).
