# RubrikaV3 Kod Borcu Tespit Raporu

**Denetim tarihi:** 2026-08-10  
**Kapsam:** `main` dalı, `734172d` (`performans modülü silindi`)  
**Yöntem:** Kaynak kodu, Tauri komut sınırı, kalıcı veri modeli, mobil API, kalite betikleri ve mevcut dokümantasyon salt-okunur incelendi. Ardından frontend ve Rust kalite kapıları çalıştırıldı.

## Sonuç

Kod tabanı derlenebilir ve mevcut test kapsamı güçlüdür. Buna rağmen bakım maliyetini ve ilerideki regresyon riskini artıran dört ana sınıf borç var:

1. Rust–TypeScript kontratları elle senkronize ediliyor ve şu anda drift mevcut.
2. Tam kalite/release kapısı yerel komutlarla tanımlı olsa da otomatik CI ile zorlanmıyor.
3. Yazılı sınav verileri, çoklu aktivite desteği için hâlâ aktif işaretçisi ve filtrelenmiş uyumluluk görünümü üzerinden taşınıyor.
4. AppState, servisler ve frontend komut adaptörü büyümüş durumda; sınırların ayrıştırılması ertelenmiş.

**Genel karar:** Şu an bir P0 veri kaybı veya derleme kırılması tespit edilmedi. Ancak API kontrat drift’i ve CI eksikliği giderilmeden yeni özelliklerin güvenli biçimde ölçeklenmesi zor. İlk iki iyileştirme release öncesi önceliklendirilmelidir.

## Önceliklendirilmiş bulgular

Öncelikler: **P1** release veya ortak mimari risk; **P2** sürdürülebilirlik/release hijyeni; **P3** düşük etkili iyileştirme.

| ID | Öncelik | Borç | Kanıt ve etki | Önerilen ödeme |
|---|---|---|---|---|
| TD-01 | P1 | Model-runtime testi zamanlamaya duyarlı | İlk tam workspace çalıştırmasında `services::rubric_extraction_service::tests::test_start_import_auto_starts_managed_model_when_closed` `rubric_extraction_service.rs:1713` satırında “model never became healthy” ile başarısız oldu. İzole tekrar ve tam workspace tekrarı geçti. Test, 5 saniyelik polling penceresiyle süreç sağlığını bekliyor; bu durum CI’da aralıklı kırmızı sonuç üretebilir. | Mock süreç ile deterministik “started/healthy” bariyeri kullanın; polling yerine test edilebilir readiness sinyali ekleyin; child-process/log temizliğini `Drop`/finally yolunda garanti edin. |
| TD-02 | P1 | Rust–TypeScript hata ve komut kontratı drift’i | Rust `AppErrorCode` 206 varyant, TypeScript union 121 varyant içeriyor; 89 Rust-only ve 4 TS-only kod var. `src/api/errors.ts:154-166` yalnızca `code` alanının string olmasını doğruluyor. `src/api/commands.ts` içinde 137 invoke, Rust tarafında 139 Tauri komutu bulundu; `get_model_runtime_status` ve `get_model_log_tail` için frontend wrapper yok. | Hata kodları ve command listesi için üretim veya CI’da çalışan tek kaynak/kontrat testi oluşturun. Eksik komutları ya wrapper’layın ya da kayıt listesinden kaldırın. `labels.ts` ve DTO’ları aynı kapıya bağlayın. |
| TD-03 | P1 | Otomatik CI ve gerçek release kalite kapısı yok | Repository’de `.github`/CI konfigürasyonu bulunmuyor. `package.json:14-15` içindeki `quality`, varsayılan package Clippy/test komutlarını çalıştırıyor; `CONTRIBUTING.md:119-120` ise workspace + all-targets + all-features ve Tauri build/smoke beklentisi tanımlıyor. Bu iki kapı eşdeğer değil. | CI’da frontend typecheck/lint/test, `cargo fmt`, `cargo clippy --workspace --all-targets --all-features`, `cargo test --workspace --all-features`, Tauri smoke/build ve artifact/provenance kontrolünü zorunlu hale getirin. |
| TD-04 | P1 | Yazılı sınav verisi aktivite başına tam ayrışmamış | `Project` içinde `questions`, öğrenci gönderimleri, OCR ve scoring koleksiyonları proje seviyesinde; `exam_package_freeze` tekil ve `active_written_assessment_activity_id` ile aktif aktiviteye bağlanıyor (`src-tauri/src/domain/project.rs:68-125`). `resolve_written_scope_id` ve `written_scope_view` (`:210-360`) legacy flat veriyi aktif işaretçiye göre filtreliyor. Testler izolasyonu doğruluyor; fakat aynı projede birden fazla yazılı aktivitenin eşzamanlı canonical saklanması hâlâ uyumluluk görünümüne bağlı. | Versiyonlu migrasyonla canonical veriyi aktivite sahibi altında saklayın. Geçiş süresince flat fallback’i yalnız legacy okuma ile sınırlayın ve aktif pointer/ambiguous durumlarının telemetri veya açık kullanıcı blokajı ile ölçüldüğünü doğrulayın. |
| TD-05 | P2 | AppState ve servisler monolitikleşmiş | `AppState` 28 servis/runtime alanı taşıyor (`src-tauri/src/lib.rs:41-69`). Büyük üretim dosyaları: `project_store.rs` 5.959 toplam satır, `speaking_exam_service.rs` 5.246, `student_answer_ocr_service.rs` 4.725, `llama_server_gateway.rs` 5.387, `question_text_service.rs` 3.366. Bu yapı startup wiring, test izolasyonu ve değişiklik yayılımını pahalılaştırıyor. | Büyük refactor yerine kademeli port/read-model ayrımı uygulayın: domain mutation, persistence, runtime ve DTO katmanlarını önce arayüzlerle ayırın; AppState’i özellik gruplarıyla compose edin. |
| TD-06 | P1 | Tauri sınırında runtime payload tipi zayıf | `invoke<T>` TypeScript derleme garantisidir; başarılı payload’ı runtime’da doğrulamaz. `commands.ts:97-110` hata normalizasyonu yapıyor, ancak birçok sayfa `unknown` değeri `as AppError` ile cast ediyor. `types.jobs.ts`, OCR/speaking/model tiplerinde `unknown` alanlar ve `cleanupJobHistory: Promise<unknown>` (`commands.ts:1116-1118`) kontratın sınırını belirsizleştiriyor. | Komut dönüşleri için merkezi runtime validator veya Rust’tan üretilen şemalar kullanın. `unknown` alanları yalnız serbest biçimli model teşhisi gibi gerekçeli alanlarda bırakın; kalan DTO’ları somutlaştırın ve UI cast’lerini `normalizeAppError`/type guard üzerinden geçirin. |
| TD-07 | P2 | Opt-in LAN API MVP hardening borcu | API loopback varsayılanı ve loopback dışı kullanımda token şartı ile sınırlanmış durumda (`mobile_server.rs:28-57`), ancak HTTP katmanı elle ayrıştırılıyor: tek `read` + 16 KiB sabit buffer (`:83-96`), bağlantı başına sınırsız thread (`:69-75`), timeout/istek boyutu/rate limit yok, token karşılaştırması düz eşitlik (`:110-122`), serialization ve socket yazma hataları fallback/ignore ediliyor (`:125-132`, `:147-167`, `:177-191`). | Yeni endpoint eklemeden önce request deadline/limit, bounded worker veya connection limit, constant-time token karşılaştırması, kontrollü shutdown ve gerçek HTTP parser/kitaplık kullanımı ekleyin. Read-only ve opt-in olduğu için bu bulgu acil açık değil; genişletme öncesi hardening gerektirir. |
| TD-08 | P2 | Teknik borç dokümantasyonu güncel ağaçla drift etmiş | `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` eski tarih/dal/HEAD ve silinmiş performance dosyalarını referanslıyor. `docs/FINAL_ACCEPTANCE_REVIEW.md`, `docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` ve `task.md` de kaldırılmış `performance_service.rs`, `PerformanceScoringPage.tsx` gibi yolları aktif bağlamda tutuyor. Bu, geçmiş kararları bugünkü gerçek durum gibi gösteriyor. | Tarihsel raporları `superseded/historical` olarak işaretleyin veya arşivleyin; kökte güncel tek bir debt ledger tutun. CI’da gerçek dosya yolları için düşük false-positive’li doküman referans kontrolü ekleyin. |
| TD-09 | P2 | Release metadata ve sürüm kaynağı dağınık | `package.json` adı `rubrikav3temp`, sürümü `0.0.0`; `src-tauri/Cargo.toml` açıklama/yazar/lisans/repository alanları placeholder; Cargo/Tauri sürümü `0.1.0`. Bu runtime hatası değil, paket provenance ve yayın hazırlığı borcudur. | Uygulama adı/sürümünü tek kaynaktan üretin; Cargo açıklama, authors, license, repository ve package metadata alanlarını yayın bilgileriyle tamamlayın. |
| TD-10 | P2 | Gerçek kullanıcı OCR kanıtı sentetik golden ile sınırlı | `docs/GOLDEN_OCR_SCORING_BENCHMARK.md:10` corpus’un tamamen sentetik olduğunu, gerçek model benchmark’ının bu corpus üzerinde çalıştığını belirtiyor. Bu, pipeline kontratını doğruluyor; farklı cihazlardan gerçek el yazısı, ışık, eğim ve tarayıcı varyasyonları için ürün kabul kanıtı değil. | Anonimleştirilmiş gerçek örneklerden oluşan, izinli bir değerlendirme seti ve CER/WER/critical-token/registration eşikleri oluşturun; model, cihaz ve preprocessing profiliyle birlikte sürümleyin. |

## Borç olarak sayılmayan doğrulanmış alanlar

Bu incelemede mevcut testlerle güçlü görünen alanlar borç listesine eklenmedi:

- `cargo clippy --workspace --all-targets --all-features -- -D warnings` geçti.
- Tam `cargo test --workspace --all-features` tekrarı geçti; `app` library hedefi 572/572 başarılı ve 4 ignored, tüm entegrasyon/workspace hedefleri de başarısız olmadan tamamlandı.
- `npm run build`, `npm run lint` ve `npm test` geçti; frontend testlerinde 158/158 başarılı.
- Atomic project persistence, write lease, path/symlink kontrolleri, audit/revision invariant’ları ve backup/security proof’ları workspace testlerinde doğrulandı.
- Workflow, rubric freeze, OCR review ve scoring gate’leri backend testleriyle korunuyor; bu alanlarda UI-only bypass tespit edilmedi.

## Çalıştırılan kontroller

| Kontrol | Sonuç |
|---|---|
| `npm run build` | PASS |
| `npm run lint` | PASS |
| `npm test` | PASS — 158/158 |
| `npm run cargo:fmt` | PASS |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --manifest-path src-tauri/Cargo.toml --workspace --all-features` | PASS — `app` library 572 passed / 4 ignored; entegrasyon ve diğer workspace hedefleri başarılı |
| `npm run tauri:build` | Bu audit kapsamında çalıştırılmadı; paketleme sonucu ayrıca doğrulanmalı |

İlk tam Rust çalıştırmasındaki tek model-runtime testi hatası izole tekrar ve tam workspace tekrarında oluşmadı. Bu nedenle raporda kalıcı fonksiyon hatası olarak değil, test deterministikliği borcu olarak sınıflandırılmıştır.
