# Tur 4 devam — Madde 6 (TD-25) ve Madde 7 (TD-27)

Proje: /Users/kadir/Desktop/RubriKa/RubrikaV3 (branch: main, HEAD fdb8e6e — Tur 0/1/2 çalışması bu commit'te yerleşik; Tur 4 maddeler 1-5 çalışma ağacında)

## Bağlam

Tur 4'ün ilk yarısı (Maddeler 1-5) tamamlandı ve çalışma ağacında duruyor — bu dosyalara DOKUNMA, yalnız onların üzerine inşa et:
- TD-13: `src-tauri/src/lib.rs` — isAppError runtime validator (tamam)
- TD-14: `src-tauri/src/jobs/job_manager.rs` — rehydrate hataları typed (tamam)
- TD-15: `src-tauri/src/jobs/job_manager.rs` + `src-tauri/src/services/speaking_exam_service.rs` + `src-tauri/src/lib.rs` — lock unwrap temizliği + commit hataları audit'e (tamam)
- TD-16: `src-tauri/src/domain/project.rs` + `scoring_service.rs` + `student_answer_ocr_service.rs` + `workflow_engine.rs` — tüm okuyucular `resolved_active_ocr_records()` üzerinden (tamam)
- TD-18/29: `src/pages/{ScoringPage,WorkflowPage,StudentAnswerOcrPage,StudentAnswerOcrIssueReviewPage,ExamPackageWorkspacePage,DocumentsPage}.tsx` — sayfa poller'ları kaldırıldı, tek merkezi job query (tamam)

Kalan iki madde:

## MADDE 6 (TD-25, P2-M): Correlation ID zinciri

Referans rapor: docs/CURRENT_TECHNICAL_DEBT_AUDIT.md TD-25.

Talimat:
1. `ModelInvocationContract` (src-tauri/src/domain/model.rs) → `MutationOptions` / `AuditEntryInput` → komut → servis zincirinde correlation_id akışını incele. Zincir bugün kırık: komut katmanında üretilen id servise, oradan mutation/audit/model invocation'a düzgün geçmiyor.
2. Zinciri tek yönlü akıt: komut katmanı (örn. `performance_commands.rs`, `scoring_commands.rs`) bir correlation_id üretir (Uuid::new_v4) → servis metoduna parametre olarak geçer → mutation + audit kaydı + model invocation contract aynı id'yi taşır.
3. İmza değişikliği gerektiren servis metodları için çağrıcıları (komutlar + testler) güncelle. Test çağrıcı sayısı fazla ise en az akışı temsil eden iki akışı (performans scoring akışı + OCR/job akışı) uçtan uca bağla; geri kalan noktalarda da aynı deseni uygula (yarım bırakma — zincir ya tam kurulur ya da madde kapsamı daraltılıp raporda açıkça not edilir).
4. Kabul kriteri: bir komut çağrısından model invocation + audit entry'ye kadar aynı correlation_id taşınır. Bunu kanıtlayan bir test yaz (ör. job_manager'daki proof_10 benzeri akış testi veya servis seviyesi test: verilen correlation_id'nin audit kaydına ve invocation contract'ına yazıldığını doğrula).
5. Mevcut davranış korunur: id üretimi yalnız komut katmanında; servisler kendileri id üretmez (testlerde üretebilir).

## MADDE 7 (TD-27, P2-S): Legacy prompt fallback fail-closed

Referans rapor: docs/CURRENT_TECHNICAL_DEBT_AUDIT.md TD-27.

Talimat:
1. src-tauri/src/services/prompt_contract.rs içindeki legacy prompt fallback'ini bul (request_contract'te None dönen yol — dormant legacy dal).
2. Fallback'i kaldır; `None` durumu typed hata olarak reddedilsin (fail-closed). Hata mesajı kullanıcı dostu + teknik detay taşısın (diagnostics deseni).
3. Test: legacy/eksik alan durumunda artık fallback DEĞİL hata dönüldüğünü kanıtlayan regresyon testi yaz.
4. prompt_contract.rs'teki mevcut testleri güncelle (fallback bekleyen testler varsa).

## Genel kurallar
- TDD: önce RED test, sonra uygulama, sonra GREEN.
- Hiçbir git commit yapma; yalnız çalışma ağacında değişiklik bırak.
- Çalışma ağacındaki mevcut 13 dosyalık değişikliğe (maddeler 1-5) dokunma.
- Kapsam dışı: Tur 0/1/2 dosyaları (artık fdb8e6e commit'inde), `.audit_cache/`, docs/, paket sürümleri.

## Doğrulama (iş bitince çalıştır, sonuçları log'a yaz)
- cargo fmt --check
- cargo clippy --workspace --all-targets -- -D warnings
- cargo test --workspace --all-features (veya en azından: cargo test --lib scoring; cargo test --lib workflow; cargo test --lib job_manager; cargo test --lib performance; cargo test --lib speaking_exam_service; cargo test --lib student_answer_ocr_service)
- npx tsc -b
- npm test

## Final rapor formatı (log sonuna yaz)
STATUS: TAMAM / YARIM
NEXT_ACTION: ...
Her madde için: ne değişti, hangi testler eklendi, doğrulama çıktıları (test sayıları, clippy/fmt durumu).
