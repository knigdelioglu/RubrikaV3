# Tur 4 — Güvenilirlik ve gözlemlenebilirlik: hata yolları, job altyapısı, correlation (RubrikaV3)

Proje: /Users/kadir/Desktop/RubriKa/RubrikaV3 (branch: main, HEAD 2060ed8 — performans özelliği cherry-pick ile taşındı)
Referans rapor: docs/CURRENT_TECHNICAL_DEBT_AUDIT.md (salt okunur denetim; değiştirme — görev kaynağı olarak kullan)
Mevcut durum: Tur 0 (kırmızı regresyon testleri), Tur 1 (veri/karar doğruluğu), Tur 2 (mimari sadeleştirme) TAMAMLANDI. Çalışma ağacında Tur 0/1/2 değişiklikleri duruyor — bunlar TEMELDİR, bozma.

## KURALLAR (Tur 1/2 ile aynı)

1. **Test-first (TDD):** Her madde için önce kırmızı test yaz, sonra düzelt, sonra yeşil olduğunu gör. Red→Green döngüsünü logla.
2. **Hiçbir git commit/amend/push/stash yapma.** Yalnız çalışma ağacında değişiklik bırak.
3. Kapsam dışı dosyalara dokunma; sadece ilgili modül + testleri.
4. AGENTS.md kuralları geçerli: typed errors, no unwrap (production), no silent fallback.
5. Tur 0/1/2 testlerini DEĞİŞTİRME (performans 23, scoring 64, workflow 34, frontend 154 test yeşil olmalı korunmalı).
6. Her madde sonunda hedefli testi koş; tüm maddeler bitince tam doğrulama (aşağıda).
7. Çalışma ağacı dışarıdan değişirse DUR ve rapor et (ajan kendisi commit etmeyecek).

## MADDE 1 — TD-13: `as AppError` unvalidated cast → runtime validator (Kapsam S)

**Problem:** `src/api/commands.ts:115-117` ve `src/pages/PerformanceScoringPage.tsx:345-350/804`'te `if (typeof e === 'object' && e !== null && 'code' in e) { throw e as AppError; }` — yalnız "code" anahtarı var diye kabul edilir; safeMessage/recoveryAction eksikse ErrorBanner çökebilir.

**Talimat:**
1. Önce kırmızı test: uydurma hata nesnesi (`{code: 123}` sayı, `{code: 'X'}` ama safeMessage yok, null, string) `isAppError` guard'ından GEÇMEMELİ; gerçek AppError şekli (`{code: string, safeMessage: string, recoveryAction?: string}`) GEÇMELİ.
2. `src/api/errors.ts` (veya mevcut uygun utils dosyası — projede nerede hata tipi tanımlıysa orası) içine `isAppError(e: unknown): e is AppError` tip guard'ı ekle.
3. `commands.ts` ve `PerformanceScoringPage.tsx`'teki cast noktalarını guard üzerinden normalize et: uymayan değeri `UNKNOWN_ERROR` şekline (code: 'UNKNOWN_ERROR', safeMessage: Türkçe genel mesaj) düşür.
4. Guard + normalize fonksiyonunun birim testlerini ekle (frontend test düzeni: `src/pages/*.test.ts`, `node --test --experimental-strip-types` — package.json'daki test listesine yeni dosyayı ekle).

**Kabul:** Guard testleri yeşil; `as AppError` ham cast'leri kalmadı (grep `as AppError` → yalnız güvenli normalize noktaları).

## MADDE 2 — TD-14: Job rehydration hatası sessizce yutuluyor (Kapsam S)

**Problem:** `src-tauri/src/commands/job_commands.rs:52` `let _ = state.job_manager.rehydrate_jobs(...)` — hata diskteki job'ları yüklemeyi bırakır; Running job'lar Interrupted'a alınmadan kaybolmuş görünür.

**Talimat:**
1. Kırmızı test: rehydrate başarısız olduğunda hata artık yutulmuyor (komut seviyesinde typed AppError'a dönüşüyor veya görünür diagnostic üretiyor). Mevcut test altyapısına uygun test yaz (komut kontrat testi desenini incele).
2. `rehydrate_jobs` hata yolunu typed AppError olarak yay; komut seviyesinde yakala, kullanıcıya teacher-safe mesaj ver (raw hata diagnostic'e).
3. Startup'ta tek rehydrate noktası olduğunu doğrula (zaten varsa tek noktada olduğunu göster; yoksa ekle).

**Kabul:** Hata yutma kalmadı (`let _ = .*rehydrate` eşleşmesi 0); test yeşil.

## MADDE 3 — TD-15: Production unwrap'ları ve yutulan commit hataları (Kapsam S-M)

**Problem:** `src-tauri/src/jobs/job_manager.rs:104,139,146,291` `Mutex::lock().unwrap()` (lock poison'da panic); `src-tauri/src/services/speaking_exam_service.rs:1156,1941,2467` `let _ = commit_snapshot_cas` (sessiz kalıcılık kaybı).

**Talimat:**
1. Kırmızı test: lock-poison birim testi — Mutex'i poison'la, çağrı panic yerine typed hata döndürmeli. commit-fail regresyonu — commit_snapshot_cas Err döndürdüğünde fonksiyon hata yaymalı (veya en azından log + audit'e işlenmeli; hayati yoldaysa yay).
2. `job_manager.rs` lock'larını `map_err` ile typed hata'ya çevir (poison durumunda `into_inner()` ile kurtarmak yerine hata yay veya poison'ı açıkça logla — hangisi daha güvenliyse; AMA panic asla).
3. `speaking_exam_service.rs`'teki üç `let _ = commit_snapshot_cas` noktasını: hayati (veri kaybı riski) yollarda hata yay; değilse `error!`/`warn!` log + audit olayı işle.
4. Mevcut testlerin geçtiğini doğrula (özellikle speaking_exam testleri).

**Kabul:** `unwrap()` production eşleşmesi 0 (test modülleri hariç); lock-poison + commit-fail testleri yeşil.

## MADDE 4 — TD-16: OCR duplicate canonical/read model (Kapsam M)

**Problem:** `student_answer_ocr_records` (flat) canonical; `resolved_active_ocr_records()` tanımlı ama hiç çağrılmıyor (dead code) — `src-tauri/src/domain/project.rs:48-52,139-151`, `student_answer_ocr_service.rs:1314-1319,1719-1727`, `scoring_service.rs:519`, `workflow_engine.rs:106-119`.

**Talimat:**
1. Kırmızı test yazmak yerine bu maddedeki doğrulama: hangi okuyucuların flat listeyi doğrudan kullandığını tespit et; `resolved_active_ocr_records` anlamsal olarak aynı sonucu üretmeli.
2. Okuyucuları `resolved_active_ocr_records` üzerinden geçir (workflow_engine readiness hesabı, scoring_service okumaları, student_answer_ocr_service okumaları). Yazımlar tek yerde kalsın (flat liste canonical yazım noktası).
3. Davranış koruyucu test: accept/reject sonrası active projection tutarlı (mevcut OCR testleri bunu örtüyorsa ek test gerekmez; örtmüyorsa bir senaryo ekle).
4. Dead code uyarısı kalktığını doğrula (clippy -D warnings geçmeli).

**Kabul:** `resolved_active_ocr_records()` en az 1 gerçek çağrı noktası; OCR testleri yeşil; clippy temiz.

## MADDE 5 — TD-18 + TD-29: Frontend job polling/event çoğalması (Kapsam M)

**Problem:** `src/app/AppLayout.tsx:192` (refetchInterval) ve `src/pages/ScoringPage.tsx:89` (refetchInterval) iki ayrı poller aynı job snapshot'ları için çalışıyor.

**Talimat:**
1. Job snapshot'ları için TEK merkezi query tanımla (mevcut AppLayout global işlem merkezini incele — oraya merkezileştir).
2. `ScoringPage`'deki sayfa bazlı poller'ı kaldır; global job query'ye bağla (staleTime/refetchInterval yalnız merkezde).
3. Kırmızı/yeşil: frontend testlerinde poller davranışını örten test varsa güncelle; yoksa merkezi query'nin varlığını doğrulayan bir test ekle (opsiyonel — mevcut 154 test yeşil kalmalı asıl kriter).
4. Grep ile `refetchInterval` kullanımının tek noktaya indiğini doğrula.

**Kabul:** `refetchInterval` yalnız merkezi job query'de; frontend testleri 154+ geçiyor; typecheck/lint temiz.

## MADDE 6 — TD-25: Correlation ID zinciri kırık (Kapsam M)

**Problem:** `ModelInvocationContract`'te correlation_id YOK (`src-tauri/src/domain/model.rs:284-296`); `MutationOptions.correlation_id` her çağrıda yeni (`project_store.rs:90-99`); `AuditEntryInput` yeni UUID (`audit_service.rs:853-886`); `scoring_service.rs:377-424` audit'e correlation chain yok.

**Talimat:**
1. Kırmızı test: bir command çağrısı (örn. performans approve veya publish) boyunca audit olayları + mutation'lar + model çağrıları AYNI correlation_id taşımalı.
2. `ModelInvocationContract`'e `correlation_id: Option<String>` (veya String) alanı ekle; gateway'e geç (mevcut prompt/yapılandırma akışına uygun).
3. `MutationOptions`'a correlation alanı ekle; `project_store` çağrılarında yeni üretmek yerine akıtılan değeri kullan (yoksa yeni üret).
4. `AuditEntryInput`'a correlation alanı ekle; audit_service kaydına yaz.
5. Komut katmanından tek id akıt: performans komutları + scoring komutlarında (komut girişinde yeni UUID → servise geçir).
6. Kontrat testi: audit kaydında command'in correlation_id'si görünür.

**Kabul:** Zincir testi yeşil; audit olayında correlation_id alanı var; mevcut testler bozulmadı (serileştirme şekli değişirse eski kayıtlarla uyumluluk korunmalı — alan Option ise güvenli).

## MADDE 7 — TD-27: Legacy prompt fallback fail-closed (Kapsam S)

**Problem:** `llama_server_gateway.rs:64-81` PromptContract yoksa `legacy_prompt_contract_with_data` fallback'ine düşüyor; `prompt_contract.rs:65-92` — dormant ama fail-closed değil.

**Talimat:**
1. Kırmızı test: `request_contract = None` geçildiğinde legacy fallback yerine typed hata döndürülmeli.
2. `request_contract` (veya eşdeğeri) `None`'ı `AppError` (typed, teacher-safe mesaj + diagnostic) olarak reddet.
3. Mevcut üretim çağrılarının hepsi `Some` geçiyor — bunlar değişmesin; yalnız None yolunu kapat.
4. Gateway ilgili testlerini güncelle (None davranışı testi ekle).

**Kabul:** None → hata testi yeşil; üretim çağrıları bozulmadı (mevcut gateway testleri geçiyor).

## DOĞRULAMA (tüm maddeler bitince — ajan kendisi koşacak)

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --lib performance   # 23+ (Tur 0/1/2 korunmalı)
cargo test --manifest-path src-tauri/Cargo.toml --lib scoring       # 64+
cargo test --manifest-path src-tauri/Cargo.toml --lib workflow      # 34+
cargo test --manifest-path src-tauri/Cargo.toml --lib job           # job_manager testleri
cargo test --manifest-path src-tauri/Cargo.toml --lib speaking      # speaking_exam testleri
cargo test --manifest-path src-tauri/Cargo.toml --lib audit         # audit/correlation testleri
npm test                                                             # 154+ (yeni dosyalar package.json test listesine eklenmeli)
npm run lint
npm run typecheck
npm run check:all                                                     # tam paket (lib + entegrasyonlar)
VITE_PORT=5175 npm run tauri:dev -- --smoke                          # smoke (port çakışması için VITE_PORT)
```

## SONUÇ RAPORU FORMATI (log sonuna yaz)

```
## STATUS
Tur 4 tamamlandı / tamamlanamadı. Her madde için: RED testi görüldü mü → düzeltme → GREEN testi.
## CHANGED_FILES
(her değişen dosya + neden)
## VALIDATION
(komut → sonuç tablosu)
## RISKS
(kalan riskler, davranış değişiklikleri, kapsam dışı notlar)
## NEXT_ACTION
(öneri: Tur 5 TD-19/20, Tur 6 TD-21/22/23, Tur 7 TD-24/28, Tur 8 TD-26/30/31 vb.)
```
