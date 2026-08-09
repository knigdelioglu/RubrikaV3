# Görev: TYMM Performans UX Sadeleştirme (2026-08-08)

## Contract özeti
- Task type: refactor (frontend bilgi mimarisi; 20 maddelik iş emri)
- Backend: opencode 1.18.15, model `opencode-go/deepseek-v4-flash` (free katman)
- Policy: STRICT — read/edit whitelist; bash/task/webfetch tamamen deny; backend `src-tauri` yalnız 5 performans dosyası okunabilir
- Task: `/tmp/repo-engineer/tymm-perf-ux/task.md`, contract: `task-contract.md`

## Routing gerekçesi
Kullanıcı explicit: opencode + deepseek-v4-flash + STRICT. FAST PATH geçerliydi (capabilities.json 2026-08-08 taze, version 1.18.15 eşleşti, strict_direct=true, auth=true). Detection/routing araştırması atlanmadı gerekmedi.

## Süreç notları
- 1. run: config'de yalnız MUTLAK path allow verildi → opencode read tool'u göreli path gönderdiği için `"*": "deny"` kazandı; agent engeli STATUS ile raporladı, dosya değişmedi (git temiz kaldı).
- Fix: her kural HEM göreli HEM mutlak formda (adapter: farklı tool'lar farklı form gönderir). Aynı oturumdan `-s` resume edildi; context korundu.
- 2. run: TAMAMLANDI raporu; 9 dosya.

## Sonuç
- Değişen: PerformanceOrganizationPage, PerformanceScoringPage, performanceOrganizationUi, performanceScoringUi, performanceScoringUi.test, performanceReportUi.test, index.css, examWorkspace, FEATURE_FLOW_MAP
- Backend (src-tauri), commands.ts, types.performance.ts, assessmentMode.ts, projectRoutes.ts: DOKUNULMADI (edit whitelist'teydi ama gerekmedi)
- Hermes doğrulaması: typecheck ✓, npm test 173/173 ✓, lint 0 error ✓ (6 warning: 4 pre-existing + 2 yeni exhaustive-deps — davranış etkisi yok), build ✓
- Hermes mekanik düzeltmeleri: performanceScoringUi.ts `candidate` undefined guard + `.ts` uzantılı import (node ESM)
- Commit: YOK (kullanıcı istemedi). Kullanıcı stash'ı `tur0+tur1 WIP` korundu.
