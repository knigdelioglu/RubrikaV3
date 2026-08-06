# Final Technical Debt Closure — FAZ 2 (TD-15 Commit Semantiği ve Yutulan Commit Hataları)

## Kapsam

Proje kökü: `/Users/kadir/Desktop/RubriKa/RubrikaV3`

Bu görev, "**Final Technical Debt Closure**" kampanyasının ikinci uygulama aşamasıdır. FAZ 0+1 tamamlanmıştır (`docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` — tek otorite matris; bu dosyayı oku, FAZ 2 sonucuyla güncelle). Bu faz yalnız iş emrinin 2. bölümünü kapsar.

**Yetki ve yasaklar:**
- Production kodunda değişiklik yapmaya açık onay verilmiştir.
- Hiçbir gerçek kullanıcı projesinde migration, repair, cleanup veya write çalıştırma (yalnız tempdir/committed fixture).
- Git commit oluşturma. Kullanıcıya ait değişiklikleri silme, stash yapma, geri alma, `git reset/clean/checkout --/restore` kullanma.
- Çalışma ağacı FAZ 0+1 sonrası durumdadır (docs/FINAL_TECHNICAL_DEBT_CLOSURE.md dahil). Mevcut değişikliklerin üzerinde çalış.
- `stash@{0}` (tur0+tur1 WIP) varlığı korunur; dokunma.

## Bağlam

`docs/CURRENT_TECHNICAL_DEBT_AUDIT.md` bulguları (denetim snapshot'ı; satır numaraları güncel ağaçta yeniden doğrulanmalı):

- **TD-15 (PARTIAL):** `src-tauri/src/jobs/job_manager.rs` Mutex `unwrap()`'ları (denetimde :104,139,146,291) ve `src-tauri/src/services/speaking_exam_service.rs` içinde yutulan `let _ = commit_snapshot_cas` (denetimde :1156,1941,2467).
- **TD-14 (CONFIRMED):** `job_commands.rs:52` rehydrate hatası `let _ =` ile yutuluyor; startup'ta tek rehydrate noktası yok.
- İlgili mimari sözleşme: `docs/PROJECTSTORE_CONCURRENCY.md`, `docs/FILE_OWNERSHIP_MAP.md` (Faz 5 job ownership), `docs/FINAL_SECURITY_RELEASE_AUDIT.md`.

KURAL: Explore/keşif ajanları kullanma, dosyaları doğrudan oku. Plan sunma, onay isteme, soru sorma — doğrudan kod uygulamasına geç. Görev sonunda STATUS formatında rapor ver.

---

## 2. Son oturumdaki TD-15 kapanışını gerçek semantik olarak doğrula ve eksik kalanları kapat

`commit_snapshot_cas` veya kritik ProjectStore commit'i başarısız olduğunda yalnız audit/log yazmak yeterli değildir.

Zorunlu davranış:

```text
commit fail
→ typed error
→ command success dönmez
→ UI "kaydedildi" göstermez
→ memory state canonical sayılmaz
→ retry mümkündür
```

Uygulama adımları:

1. Güncel production call graph'ını incele: `commit_snapshot_cas` dönüş değerini `let _ =` / `let _ = result` / `if let Ok` / yutma desenleriyle bırakan tüm call-site'ları bul (özellikle `speaking_exam_service.rs`, `scoring_service.rs`, `student_answer_ocr_service.rs`, `analysis_service.rs`, performans ile ilgili yollar). Yutulan her call-site'ı typed error yayma veya açık `map_err` + görünür hata yüzeyine çevir; komut başarısını hiçbir koşulda yutulan commit üzerine kurma.
2. `job_manager.rs` Mutex lock `unwrap()`'larını typed hata/`map_err` desenine çevir (lock poison'da panic yerine AppError). Değişiklik davranışı bozmasın; yalnız panic-riskini kapat.
3. `rehydrate_jobs` hata yolunu typed hata veya görünür diagnostic'e çevir; startup'ta tek rehydrate noktası olduğunu doğrula.
4. **Speaking, performance, OCR ve scoring kritik mutation yollarında** regresyon testleri ekle: commit fail → typed error → success dönmez → memory state canonical sayılmaz → retry mümkün. Kırmızı→yeşil kanıtını raporda göster.
5. `docs/FINAL_TECHNICAL_DEBT_CLOSURE.md` matrisinde TD-14, TD-15 durumlarını güncelle.

Kapsam dışı: workflow otoritesi (Faz 4), correlation zinciri (Faz 9), unwrap olmayan diğer dosyalar. Yalnız yukarıdaki call-site sınıfına dokun.

---

## ÇALIŞMA SÖZLEŞMESİ

- Önce mevcut projeyi ve ilgili dosyaları incele.
- Görev kapsamı dışındaki dosyaları değiştirme.
- Mevcut kullanıcı değişikliklerini silme veya geri alma.
- `git reset`, `git clean`, `git checkout --`, `git restore`, force push, rebase veya geçmiş değiştiren Git komutlarını kullanma.
- Hiçbir koşulda Git commit, branch, tag veya pull request oluşturma.
- Kullanıcı açıkça istemedikçe bağımlılık sürümlerini topluca yükseltme; dosya silme.
- Gizli anahtarları, tokenleri, kullanıcı verilerini veya proje içeriğini dış servislere gönderme.
- Gereksiz biçimlendirme ve kapsam dışı refactor yapma.
- Uygulamadan önce ilgili mimariyi ve mevcut davranışı doğrula.
- Değişiklikleri küçük ve denetlenebilir tut.
- Çalıştırılan testler başarısız olursa saklama; hata mesajlarını kısa ve doğru biçimde raporla.
- Çalışma sonunda yalnızca aşağıdaki formatta sonuç ver:

```text
STATUS: COMPLETED | BLOCKED | APPROVAL_REQUIRED | FAILED
SUMMARY: En fazla 10 satırlık sonuç özeti
CHANGED_FILES: Değiştirilen dosya yolları
VALIDATION: Çalıştırılan testler ve sonuçları (exit code + passed/failed + süre)
RISKS: Kalan riskler veya "none"
NEXT_ACTION: Gerekli sonraki işlem veya "none"
```

Onay gerektiren, geri döndürülemez, kapsamı genişleten ya da güvenlik açısından riskli bir işlemle karşılaşırsan işlemi gerçekleştirme; `STATUS: APPROVAL_REQUIRED` formatında çıkış yap (APPROVAL_REQUEST, REASON, IMPACT, ALTERNATIVES).

## Doğrulama (bu fazın kapsamı — AGENTS.md seviye D/E)

- `cargo fmt --manifest-path src-tauri/Cargo.toml --check` (bozuksa yalnız değiştirdiğin dosyaları formatla)
- Hedefli Rust testleri (yeni regresyon testleri dahil, kırmızı→yeşil kanıtıyla):
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib job_manager`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib speaking_exam`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib performance`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib project_store`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib student_answer_ocr`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` (değiştirdiğin dosyalardaki hataları düzelt; kapsam dışı mevcut hata raporla, düzeltme)
- `npm run typecheck`, `npm run lint` (yalnız değişen dosya hataları)
- `npm test -- --run` (yalnız etkilenen frontend testleri — bu fazda frontend değişikliği beklenmiyor; yoksa atla ve belirt)
- `git diff --check`

Tam suite (check:all, smoke, tauri:build, full cargo test) **Faz 11'e** aittir — bu fazda çalıştırma. Çalıştırdığın her komutun süresini raporla (komut, exit code, passed/failed/ignored, elapsed).

## Kabul kriterleri (bu faz için)

- Hiçbir kritik mutation yolunda yutulan commit yok (grep ile negatif tarama: `let _ = .*commit_snapshot_cas` ve benzeri desenler 0 sonuç).
- Commit fail senaryosunda komut typed error döner; success DTO dönmez; retry mümkündür (test kanıtı).
- job_manager production path'inde `unwrap()`/`expect()`/`panic!` yok (grep kanıtı).
- Rehydrate hatası görünür/typed (startup tek nokta).
- Matris güncellendi; yeni git commit yok; kullanıcı değişikliklerine dokunulmamış.
