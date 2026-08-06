# Final Technical Debt Closure — FAZ 4 (Workflow ve Readiness Tek Otoritesi)

## Kapsam

Proje kökü: `/Users/kadir/Desktop/RubriKa/RubrikaV3`

"**Final Technical Debt Closure**" kampanyasının dördüncü uygulama aşaması. FAZ 0+1, FAZ 2, FAZ 3 tamamlandı (`docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` — tek otorite matris; oku ve FAZ 4 sonucuyla güncelle). Bu faz yalnız iş emrinin 4. bölümünü kapsar (TD-03, TD-10, TD-11 ile ilgili).

**Yetki ve yasaklar:**
- Production kodunda değişiklik serbest. Git commit oluşturma. Kullanıcıya ait değişiklikleri silme, stash yapma, geri alma. Mevcut WIP (`stash@{0}`) ve 50+ değişik dosya korunur.
- Migration/repair/cleanup çalıştırma (bu fazda veri migration'ı yok).
- Explore/keşif ajanı kullanma; plan sunma; onay isteme. Doğrudan uygula, sonunda STATUS raporu ver.

## Bağlam (mevcut durum — doğrula, tekrar keşfetme)

- `workflow_engine.rs::evaluate_workflow_inner` zaten canlı hesaplıyor ve `written_scope_view` kullanıyor; "live evaluation must not short-circuit to a stale persisted build stage" testi var (~:1688). Eski persisted-snapshot kısa devresi (eski :384-400) kaldırılmış görünüyor — **negatif tarama ile teyit et**: workflow_engine'de persisted `project.workflow`'u aynen döndüren dal kalmamalı.
- `project.workflow` hâlâ her mutation'da yeniden yazılıyor (project_store cache) ve `Project.workflow` alanı "authoritative" izlenimi verebilir.
- `scoring.rs` readiness artık set-based: `expected_pairs`, `missing_pairs`, `duplicate_pair_count` (scoring.rs:664-738) — **teyit et**: boş `student_submissions`/`questions` vacuous ready üretmiyor (`expected_records > 0` koruması), duplicate false-ready'yi engelliyor.
- Frontend `examWorkspace.ts`: `derivePerformanceStepStatuses` artık backend `PerformanceStatusDto` tüketiyor (~:93, :410). `deriveExamStepStatuses` (~:83, :474) hâlâ var — bunun ne kadar domain kararı ürettiğini denetle.
- `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` TD-03 (performans readiness frontend'de), TD-10 (birden çok workflow otoritesi), TD-11 (count-only readiness) referans.

## 4. Workflow ve readiness tek otoritesini finalleştir

Tur 2 değişikliklerini yeniden doğrula ve eksik kalanları kapat:

- Persisted `project.workflow` authoritative olmamalı; yalnız cache/diagnostic ise **açıkça işaretlenmeli** (domain alanına yorum + gerekirse serde dokümanı; `Project.workflow` alanı "cache only, live source = evaluate_workflow" notu taşımalı). Yanlış otorite kullanımına karşı regresyon testi: persisted workflow'u elle bozulmuş projede `get_workflow_snapshot` yine doğru canlı stage döndürmeli.
- Her workflow sonucu canlı canonical state'ten hesaplanmalı (negatif tarama + mevcut testlerle teyit).
- Performance, Written, Speaking ve Listening aynı backend snapshot sözleşmesini kullanmalı: Written/Listening/Speaking → `WorkflowSnapshot` (evaluate_workflow), Performance → `get_performance_status` DTO. Dört family için kontrat testleri mevcut mu, eksikse ekle (frontend'de family başına backend DTO tüketimi).
- Frontend `derive*Statuses` domain kararı üretmemeli: her derive helper yalnız backend DTO'yu normalize edip render etmeli; backend'de olmayan readiness kararı (ör. "rubrik yayınlandı → blocked" türetimi) üretmemeli. Kalan domain-karar türeten kolları backend DTO alanına taşı veya kaldır.
- Scoring readiness gerçek `(submission_id, question_id)` kümesi üzerinden duplicate/missing kontrolü yapmalı (mevcut set-based kod teyit + duplicate fixture false-ready testi).
- Boş öğrenci/soru listesi vacuous `.all()` ile hazır sayılamaz (boş-list regression testi).

## ÇALIŞMA SÖZLEŞMESİ

- Kapsam dışı dosyaları değiştirme; gereksiz refactor/biçimlendirme yapma.
- `git reset/clean/checkout --/restore`, force push, rebase, geçmiş değiştiren komutlar yasak.
- Hiçbir koşulda git commit/branch/tag/PR oluşturma.
- Çalışma sonunda:

```text
STATUS: COMPLETED | BLOCKED | APPROVAL_REQUIRED | FAILED
SUMMARY: En fazla 10 satır
CHANGED_FILES: ...
VALIDATION: komut, exit code, passed/failed, süre
RISKS: ...
NEXT_ACTION: ...
```

## Doğrulama (bu faz)

- `cargo test --manifest-path src-tauri/Cargo.toml --lib workflow` ve `--lib scoring` (readiness)
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` (TAM lib — ~24s)
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check` (bozuksa yalnız değiştirdiğin dosyaları formatla)
- Frontend değişikliği varsa: `npm run typecheck`, `npm run lint`, `npm test -- --run`
- `git diff --check`

Tam suite (check:all, smoke, build, integration) FAZ 11'e aittir; bu fazda çalıştırma.

## Kabul kriterleri (bu faz)

- Persisted workflow cache-only işaretli; canlı hesaplama tek otorite (negatif tarama + bozuk-cache regression testi).
- Dört family backend snapshot tüketiyor; frontend derive'ları yalnız normalize (test edilmiş).
- Scoring readiness set-based + vacuous-ready koruması testli.
- TD-03/TD-10/TD-11 matriste FAZ 4 ile güncellendi.
- Yeni commit yok; kullanıcı değişiklikleri korunmuş.
