# Tur 1 — Veri ve karar doğruluğu düzeltmeleri (RubrikaV3 performans değerlendirme)

Proje: /Users/kadir/Desktop/RubriKa/RubrikaV3 (branch: performans_degerlendirme)
Referans: `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` (denetim raporu) ve `.audit_cache/tur0_run.log` (Tur 0'da yazılan regresyon testlerinin durumu).

Ön koşul: Tur 0'da 7 kırmızı regresyon testi yazıldı. Bu turda o testler **yeşile dönmelidir** — testler düzeltmenin doğrulama kanıtıdır. Aşağıdaki düzeltmeleri yap ve her biri için ilgili regresyon testinin geçtiğini göster.

## Yapılacak düzeltmeler

### 1. TD-02 — Onaylı kayıt her command dalında immutable (P0)
- `src-tauri/src/services/performance_service.rs` `set_performance_assessment_status`: `assessment_id: None` dalında öğrenci ID'siyle bulunan kayıt `Approved` ise typed hata döndür (rapor: satır 778-843). Aynı kuralı bu servisteki diğer durum değiştiren mutation'larda da tutarlı uygula (save existing dalı zaten koruyor; doğrula).
- Tur 0 testi 1 yeşil olmalı.

### 2. TD-06 — Performans kaydı bulunan class application silinmesin
- `src-tauri/src/services/assessment_organization_service.rs` `remove_class_application`: dependency taramasına `!application.performance_assessments.is_empty()` ekle; varsa typed `AssessmentClassApplicationInUse` benzeri hata döndür (rapor: satır 650-663).
- Tur 0 testi 5 yeşil olmalı.

### 3. TD-05 — Yabancı veya uyuşmayan assessment ID typed error üretsin
- `save_performance_assessment`: `assessment_id: Some` ise kaydın bu application içinde VE student_id ile eşleştiğini doğrula; eşleşmiyorsa typed hata (örn. `AssessmentInvalidInput`), asla yeni kayıt oluşturma (rapor: satır 568-575).
- Tur 0 testi 2 yeşil olmalı.

### 4. TD-04 — Mevcut taslak başladığı rubrik sürümünde sabit kalsın
- `save_performance_assessment` existing dalı: kaydın `rubric_id`/`rubric_version`'ını DEĞİŞTİRME; validate_ratings ve toplam hesabını kaydın sabitlediği sürümle yap. Yeni kayıt hâlâ en yeni yayınlanmış sürümü sabitler (rapor: satır 552-553, 584-588).
- Tur 0 testi 3 yeşil olmalı.

### 5. TD-07 — Provisional ve final toplam ayrımı
- `get_performance_report` DTO'su: InProgress kayıtların geçici toplamı final toplam olarak dönmemeli. Satır bazında `isApproved` (veya `status`) ile ayrım + toplam sütununda yalnız onaylı satırlarda final değer (rapor: satır 1055-1063, 989). DTO'ya `provisionalTotal`/`finalTotal` gibi additive alan eklenebilir.
- Frontend rapor görünümü (rapor tablosu/CSV/PDF): onaysız satırı açıkça işaretle (örn. "Taslak" rozeti, toplamda `—` veya ayrı sütun).
- Tur 0 testi 4 yeşil olmalı; frontend rapor testleri varsa güncelle.

### 6. TD-08 — CSV injection kapatılsın
- `src/pages/performanceReportUi.ts` `escapeCell`: `=`, `+`, `-`, `@`, `\t`, `\r` ile başlayan hücreleri tırnakla/önekle kaçışla. UTF-8 BOM korunmalı.
- Tur 0 testi 6 yeşil olmalı.

### 7. TD-09 — Frontend mutation yarışı ve draft kaybı kapatılsın
- `src/pages/PerformanceScoringPage.tsx`:
  - Save in-flight iken approve/status/revert butonlarını devre dışı bırak (Tur 0'da çıkarılan `derivePerformanceActionAvailability` fonksiyonunu bileşene bağla; `savePending`/`approvePending`/`statusPending` koşullarını uygula).
  - `refreshAssessments` sonrası draft state'in (ratingDrafts/feedback) sunucu snapshot'ıyla sıfırlanmasını yalnızca pending yokken yap; save onSuccess'inde yalnız ilgili assessment'ı tazele (rapor: satır 149-162, 282-288, 612-659).
- Tur 0 testi 7 yeşil olmalı.

### 8. Teknik enum/UUID gösterimleri öğretmen ekranından çıkarılsın (TD-17)
- `src/pages/PerformanceScoringPage.tsx:492-493`: Missing/NotPerformed İngilizce enum adlarını Türkçe etiketlerle değiştir (örn. "Eksik", "Yapılmadı" — mevcut etiket sistemini kullan).
- `src/pages/PerformanceScoringPage.tsx:966-968`: `report.teacherId` UUID'si yerine öğretmen adı veya anonim etiket göster.
- `src/components/workflow/BlockingReasons.tsx:11`: etiketi olmayan ham engel kodu yerine Türkçe genel mesaj göster.
- `src/pages/performanceReportUi.ts` CSV/PDF'teki benzer teknik değerleri gözden geçir.

### 9. All-target Clippy ve smoke + tam doğrulama
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` temizlenmeli: 5 test-kodu lint'i (deterministic_scoring_service.rs:888 `field_reassign_with_default`; scoring_anchor_service.rs:626/636/647; scoring_cache_service.rs:392 `cloned_ref_to_slice_refs`) düzelt. Yalnız lint'i düzelt, test davranışını değiştirme.
- Smoke testi: `vite.config.ts` `server.port` değerini `process.env.VITE_PORT` ile override edilebilir yap (varsayılan 5173 korunur, strictPort korunur), sonra `VITE_PORT=5175 npm run tauri:dev -- --smoke` ile koş. Port 5175'in boş olduğunu önce doğrula. Smoke app'in exit(0) ile kapanması beklenir.
- Tam doğrulama çalıştır: `npm run check:all` (build+typecheck+lint+test+cargo:fmt+cargo:clippy+cargo:test).

## Dokunulmayacaklar
- TD-01 (yazılı çoklu sınav scope'u — Tur 3), TD-12 (legacy default migration — kullanıcı onayı gerektirir), TD-03/TD-10/TD-11 (workflow/readiness — ikinci tur).
- `docs/` içeriği, migration davranışı, mevcut testlerin iddiaları (yalnız Tur 0'da eklenen testlerin yeşile dönmesi hedeflenir; gerekirse Tur 0 testlerinde düzeltme kapsamına uygun küçük düzeltmeler yapılabilir).
- Port 5173'te çalışan canlı vite süreçlerine dokunma (kullanıcının kendi süreçleri).

## Doğrulama (ZORUNLU)
1. `cargo test --manifest-path src-tauri/Cargo.toml performance` ve `... assessment_organization` — Tur 0 testleri dahil tümü yeşil.
2. `npm test` — tüm frontend testleri yeşil (Tur 0 testleri dahil).
3. `npm run lint` ve `npm run typecheck` — temiz.
4. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` — exit 0.
5. `VITE_PORT=5175 npm run tauri:dev -- --smoke` — smoke başarılı.
6. `npm run check:all` — tam doğrulama.
Süre maliyeti biliniyor (~8 dk check:all); bu görevde açıkça istendiği için çalıştır.

## Sonuç formatı
ÇALIŞMA SÖZLEŞMESİ'nin sonunda belirtilen formatta çıkış yap (STATUS/SUMMARY/CHANGED_FILES/VALIDATION/RISKS/NEXT_ACTION). Her TD maddesi için hangi dosyada ne değiştiğini ve hangi testin yeşile döndüğünü SUMMARY'de belirt.

ÇALIŞMA SÖZLEŞMESİ

- Önce mevcut projeyi ve ilgili dosyaları incele.
- Görev kapsamı dışındaki dosyaları değiştirme.
- Mevcut kullanıcı değişikliklerini silme veya geri alma.
- `git reset`, `git clean`, `git checkout --`, `git restore`, force push,
  rebase veya geçmiş değiştiren Git komutlarını kullanma.
- Hiçbir koşulda Git commit, branch, tag veya pull request oluşturma —
  değişiklik ne kadar büyük olursa olsun, kullanıcı onayı olsa bile.
- Kullanıcı açıkça istemedikçe bağımlılık sürümlerini topluca yükseltme.
- Kullanıcı açıkça istemedikçe dosya silme.
- Gizli anahtarları, tokenleri, kullanıcı verilerini veya proje içeriğini
  dış servislere gönderme.
- Gereksiz biçimlendirme ve kapsam dışı refactor yapma.
- Uygulamadan önce ilgili mimariyi ve mevcut davranışı doğrula.
- Değişiklikleri küçük ve denetlenebilir tut.
- Çalıştırılan testler başarısız olursa saklama; hata mesajlarını kısa ve
  doğru biçimde raporla.
- Çalışma sonunda yalnızca aşağıdaki formatta sonuç ver:

STATUS: COMPLETED | BLOCKED | APPROVAL_REQUIRED | FAILED
SUMMARY: En fazla 10 satırlık sonuç özeti
CHANGED_FILES: Değiştirilen dosya yolları
VALIDATION: Çalıştırılan testler ve sonuçları
RISKS: Kalan riskler veya "none"
NEXT_ACTION: Gerekli sonraki işlem veya "none"

Onay gerektiren, geri döndürülemez, kapsamı genişleten ya da güvenlik açısından
riskli bir işlemle karşılaşırsan işlemi gerçekleştirme. Şu formatta çıkış yap:

STATUS: APPROVAL_REQUIRED
APPROVAL_REQUEST: Yapılmak istenen işlem
REASON: Neden gerekli olduğu
IMPACT: Hangi dosya, veri veya sistemi etkileyeceği
ALTERNATIVES: Varsa daha güvenli seçenekler
