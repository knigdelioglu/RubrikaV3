# Faz B — TYMM Performans Değerlendirme: Frontend İş Akışı

## Amaç
Faz A'da kurulan backend'i (PerformanceDetails, sürümlü rubrik, PerformanceAssessment, 10 komut) kullanıcıya sunan frontend iş akışını kur. UI smoke bu fazın sonunda kullanıcı tarafından yapılacaktır.

## Kapsam (yalnız bunlar)

### 1. Performans türü yönlendirmesi (opencode'un Faz A raporundaki notu)
- `src/.../examWorkspace.ts` (veya mevcut adım tanımı dosyası): `AssessmentType.Performance` için adım tanımı ekle.
- `src/.../assessmentMode.ts` (veya tür yönlendirmesi yapılan yer): performans türünü organizasyon + değerlendirme akışına yönlendir.
- Mevcut yazılı/konuşma akışlarını bozma.

### 2. Performans görevi organizasyon ekranı (mevcut AssessmentOrganizationPage deseninde)
- Görev oluşturma/düzenleme: PerformanceDetails formu (tema, öğrenme çıktıları, beceri alanı, yönerge, bireysel/grup, teslim tarihi, kanıt türleri).
- Rubrik düzenleyici: 3-6 ölçüt, 3 veya 5 düzey, her düzey için gözlenebilir tanım; taslak (sürüm 0) düzenlenebilir, yayın akışı (PublishPerformanceRubricInput ile yeni sürüm); yayınlanmış rubrik kilidi (düzenlenemez, yalnız görüntülenir).
- `PerformanceDetails.rubric_versions` listesinden beslenir (Faz A notu: sürüm 0 = yayınlanmamış taslak, ≥1 = yayınlanmış).

### 3. Değerlendirme ekranı
- Sınıftaki öğrenci listesi; öğrenci başına düzey seçimleri (rubrik ölçütleri).
- Geçici toplam hesabı (yalnız seçili düzeylerden; otomatik dönem notu YOK).
- Eksik ölçüt ve eksik öğrenci uyarıları görünür; `Missing`/`NotPerformed` işaretleme — görsel olarak sıfırdan AYRI (ör. farklı rozet/renk, puan kolonunda boşluk).
- Onay akışı: zorunlu ölçütler tamamlanmadan onay butonu kapalı; onay sonrası düzenleme reddedilir (backend zaten reddediyor — UI'da buton devre dışı bırak).
- Backend komutlarını kullan: activity create/update, rubric publish/history, assessment save/approve/status, list (komut adlarını `src-tauri/src/commands/performance_commands.rs`'ten ve `src/api/types.ts`'teki typed sözleşmelerden al).

### 4. Frontend tipleri
- Faz A'da eklenen typed API sözleşmelerini UI'da kullan; eksik kalan tip bağlantılarını tamamla (mevcut desene uygun, optional alanları koru).

## Kısıtlar (değişmez kurallar — ihlal etme)
1. `ScoringRecord` ve yazılı sınav puanlama akışına DOKUNMA.
2. Tek sayı girişi yok (yalnız rubrik düzeyi seçimi).
3. Otomatik dönem notu yok.
4. Akran/öz değerlendirme ortalamasıyla puan değişimi yok.
5. Grup üyelerine otomatik eşit puan yok.
6. Sessiz yeniden hesaplama yok.
7. `Missing` ≠ `NotPerformed` ≠ sıfır puan; raporda ayrı gösterilir.
8. AI/otomasyon puan vermez; puanı öğretmen seçer.
9. Türkçe arayüz metinleri; mevcut kodun stil/dil desenine uy.

## Test — YOK (kullanıcı kuralı)
Kullanıcı politikası: testler yalnızca büyük işlerde (mimari değişiklik/büyük refactor) yazılır ve çalıştırılır. Faz B orta ölçekli iştir → **yeni test yazma, mevcut testleri çalıştırma**. `npm test` ÇALIŞTIRMA.

## Doğrulama (dar)
1. `npm run build` — PASS olmalı (TS derlemesi + vite).
2. `npm run typecheck` — PASS olmalı.
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` — UI ile Rust tarafında değişiklik yaptıysan PASS; yapmadıysan çalıştırma.
4. Her komutun süresini raporla.
5. `npm test`, `cargo test`, lint tam suite ÇALIŞTIRMA (kural).

## Rapor (yanıtının sonunda)
- Yapılan ekranlar/akışlar (dosya yollarıyla).
- Doğrulama tablosu (komut, sonuç, süre).
- Kullanıcı için UI smoke kontrol listesi (elle test edilecek adımlar).
- Faz C için notlar (şablon/rapor entegrasyon noktaları).

## Git
git add/commit/push YAPMA. Çalışma ağacını olduğu gibi bırak.
