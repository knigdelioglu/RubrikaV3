# Tur 0 — Kırmızı regresyon testleri (RubrikaV3 performans değerlendirme)

Proje: /Users/kadir/Desktop/RubriKa/RubrikaV3 (branch: performans_degerlendirme, HEAD 26a41ba)
Referans: `docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` — denetim raporu; test senaryolarının kaynağıdır (TD-02, TD-04, TD-05, TD-06, TD-07, TD-08, TD-09).

Bu görev **YALNIZCA regresyon testleri yazar**. Hiçbir production düzeltmesi yapılmaz. Testler, aşağıdaki 7 hatayı **mevcut kod üzerinde yakalamalıdır (kırmızı kalmalıdır)**. Düzeltmeler bir sonraki turda yapılacaktır; testlerin kırmızı kalması görevin başarı ölçütüdür.

## Yapılacak testler

### Backend A — `src-tauri/src/services/performance_service.rs` (mevcut `#[cfg(test)] mod tests` bloğuna ekle; mevcut yardımcıları kullan: `temp_project`, `setup_environment`, `service`, `create_task`)

1. **TD-02 (P0) — Onaylı kaydın durumu, ID verilmeden değiştirilemez**
   - Akış: bir öğrenci için değerlendirme kaydı oluştur, tüm ölçütleri puanla, `approve_performance_assessment` ile onayla (kayıt `Approved`).
   - Ardından `set_performance_assessment_status` çağrısını `assessment_id: None`, `student_id` ile yap.
   - Beklenen: `Err` (typed AppError). Mevcut kod onaylı kaydı bulup puanlarını/notunu silip `Missing`'e çekiyor (rapor: performance_service.rs:778-843) → test KIRMIZI.

2. **TD-05 — Yabancı (başka uygulamaya ait) assessment_id duplicate kayıt oluşturamaz**
   - Akış: uygulama A'da bir değerlendirme kaydı oluştur (id'sini sakla). Uygulama B'de (farklı class application) ayrı bir kayıt oluştur.
   - `save_performance_assessment`'ı uygulama B'nin `class_application_id`'si + uygulama A'nın `assessment_id`'si + B'deki bir öğrenciyle çağır.
   - Beklenen: `Err` (typed AppError) ve B'de yeni/ikinci kayıt oluşmamış olmalı. Mevcut kod eşleşme bulamayınca sessizce YENİ kayıt oluşturuyor (rapor: performance_service.rs:568-575) → test KIRMIZI.

3. **TD-04 — Yeni rubrik sürümü mevcut taslağın sürümünü ve toplamını sessizce değiştiremez**
   - Akış: rubrik v1 yayınla → taslağı v1 ile kaydet (puanlar dolu, `provisional_total` v1 düzey puanlarıyla hesaplanır) → rubrik v2 yayınla (düzey puanlarını değiştirerek) → **aynı taslağı yeniden kaydet**.
   - Beklenen: kaydın `rubric_id`/`rubric_version` değerleri DEĞİŞMEZ ve `provisional_total` v1 puanlarıyla aynı kalır.
   - Mevcut kod existing dalında kaydı en yeni yayınlanmış rubriğe re-pin ediyor ve toplamı yeniden hesaplıyor (rapor: performance_service.rs:584-588, 552-553) → test KIRMIZI.

4. **TD-07 — InProgress değerlendirme, final rapora final puan olarak giremez**
   - Akış: biri `Approved`, diğeri `InProgress` iki öğrencili bir sınıf uygulaması kur; `get_performance_report` çağır.
   - Beklenen iddia: InProgress satırın geçici toplamı, raporun final/onaylı toplamı olarak dönemez (InProgress için `total` `None`/ayrı işaretli olmalı ya da DTO'da provisional/final ayrımı talep eden bir iddia).
   - Önce mevcut DTO'yu ve `get_performance_report` çıktısını incele (rapor: performance_service.rs:1055-1063). Testi mevcut yapıya göre "InProgress toplamı final toplam olarak döner" davranışını yakalayacak şekilde yaz; **DTO değişikliği YAPMA** — sadece test. Mevcut kod InProgress toplamını `total` olarak basıyor → test KIRMIZI.

### Backend B — class application silme

5. **TD-06 — Performans değerlendirmesi bulunan class application silinemez**
   - `src-tauri/src/services/assessment_organization_service.rs` içine (test modülü yoksa yeni `#[cfg(test)] mod tests` ekle; mevcut proje fixture/yardımcı desenlerini takip et — gerekiyorsa performance_service.rs test modülündeki `temp_project`/`setup_environment` desenini kullan).
   - Akış: performans değerlendirmesi (tercihen onaylı) içeren bir class application'ı, ilgili silme komutunun servis yoluyla silmeyi dene (`remove_class_application` veya çağıran komut yolu).
   - Beklenen: `Err` (typed AppError, in-use benzeri). Mevcut kod yalnız speaking attempt'leri tarıyor; performans kayıtlarını kontrolsüz siliyor (rapor: assessment_organization_service.rs:650-663) → test KIRMIZI.

### Frontend — yeni test dosyaları (node --test deseni, mevcut `*.test.ts` dosyalarının stilini takip et)

6. **TD-08 — CSV formula injection engellenir** → yeni `src/pages/performanceReportUi.test.ts`
   - `src/pages/performanceReportUi.ts`'teki CSV üretim fonksiyonunu test et.
   - Öğrenci adı `=HYPERLINK("http://evil","tık")`, `+SUM(1,1)`, `-1+2`, `@cmd` gibi değerlerle üretilen CSV'de hücreler formül olarak yorumlanamayacak şekilde kaçışlanmış olmalı (tırnaklama veya önek).
   - Mevcut kod yalnız `;`/`"`/yeni satır kaçışlıyor; `=`/`+`/`-`/`@` önekleri korumasız (rapor: performanceReportUi.ts:38-44) → test KIRMIZI.

7. **TD-09 — Save devam ederken approve/status çağrısı yapılamaz** → yeni `src/pages/performanceScoringUi.ts` + `src/pages/performanceScoringUi.test.ts`
   - Önce `src/pages/PerformanceScoringPage.tsx`'teki kullanılabilirlik mantığını incele: `canApprove` koşulları (~satır 282-288) ve status/revert butonlarının disabled koşulları (~satır 612-659).
   - Bu mantığı **davranış değiştirmeden** yeni bir saf, export edilmiş fonksiyona taşı: `src/pages/performanceScoringUi.ts` içinde `derivePerformanceActionAvailability(...)` (parametreler: mevcut koşulların tüm girdileri + `savePending`/`approvePending`/`statusPending` bayrakları; dönüş: `{ canApprove, canChangeStatus, canRevert, reason?: string }`). **Şu an pending bayraklarını kontrol ETME** — mevcut davranışı birebir koru.
   - Test: `savePending: true` iken approve ve status değişikliği kullanılamaz olmalı (`canApprove === false`, `canChangeStatus === false`). Mevcut davranışta pending kontrolü yok → fonksiyon `true` döner → test KIRMIZI.
   - **Bileşeni bu fonksiyona bağlama** — bağlama bir sonraki turda yapılacak.

## Zorunlu paket güncellemesi
- `package.json` `test` script'i açık dosya listesi kullanıyor. Yeni test dosyalarını (`src/pages/performanceReportUi.test.ts`, `src/pages/performanceScoringUi.test.ts`) listeye ekle. Başka script'i değiştirme.

## Dokunulmayacaklar
- Production davranış değişikliği YOK (tek istisna: TD-09 için davranış koruyan saf fonksiyon çıkarma; bileşen bağlanmaz).
- `docs/`, `src-tauri/tests/` (mevcut integration testler), migration kodu, domain tipleri, `performance_service.rs`'in production kodu (test modülü dışı).
- Mevcut testleri DEĞİŞTİRME; yalnız ekleme yap.

## Doğrulama (çalıştırılacaklar — bu görevde test koşturmak ZORUNLUDUR)
1. `cargo test --manifest-path src-tauri/Cargo.toml performance` — yeni backend testleri derlenmeli ve KIRMIZI olmalı; mevcut 11 performance testi yeşil kalmalı.
2. `cargo test --manifest-path src-tauri/Cargo.toml assessment_organization` — TD-06 testi KIRMIZI.
3. `npm test` — typecheck + tüm frontend testleri: yeni testler KIRMIZI, mevcut testler yeşil.
4. `npm run lint` — yeni dosyalar lint hatasız.

Not: Kırmızı testlerin FAIL çıkması BEKLENENDİR ve görevin başarısıdır. VALIDATION bölümünde her testin kırmızı kalma nedenini tek satırla belirt.

## Sonuç formatı
ÇALIŞMA SÖZLEŞMESİ'nin sonunda belirtilen formatta çıkış yap (STATUS/SUMMARY/CHANGED_FILES/VALIDATION/RISKS/NEXT_ACTION).

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
