# GÖREV: TYMM Performans Değerlendirme İş Akışı — PLANLAMA (kod yazma!)

RubrikaV3 projesine TYMM uyumlu performans değerlendirme iş akışı eklenecek. Senin görevin YALNIZCA uygulanabilir bir uygulama planı üretmek. KESİNLİKLE hiçbir kaynak kod dosyasını değiştirme, hiçbir dosya oluşturma (plan çıktısı hariç), test çalıştırma.

## Girdiler

1. Rapor: `docs/TYMM_PERFORMANCE_SCALE_REPORT.md` (tam içeriğini oku) — TYMM yaklaşımı, TDE performans görevleri, Rubrika entegrasyon önerisi.
2. Kod tabanı: React+TS frontend (`src/`) + Tauri/Rust backend (`src-tauri/src/`).

## Planlamadan önce incelemen gerekenler (dokümantasyon + kod)

- `docs/ASSESSMENT_ORGANIZATION.md` — mevcut sınav organizasyonu (SchoolClass, TeachingAssignment, AssessmentActivity, ClassApplication)
- `docs/API_CONTRACTS.md` — Tauri komut sözleşmeleri
- `docs/FILE_OWNERSHIP_MAP.md` ve `docs/SYMBOL_MAP.md` — dosya/sembol sahipliği
- `src-tauri/src/domain/` — `school_class.rs`, `assessment.rs`, `student.rs`, `project.rs`, `rubric.rs`, `scoring.rs` (ScoringRecord'a DOKUNULMAYACAK — raporda yasak)
- `src-tauri/src/commands/` — komut katmanı örüntüleri (özellikle `assessment_organization_commands.rs`, `school_class_commands.rs`, `scoring_commands.rs`, `speaking_exam_commands.rs`)
- `src-tauri/src/services/` — servis örüntüleri ve `ProjectStore` tek-yazar deseni
- `src/pages/` — UI örüntüleri (`AssessmentOrganizationPage.tsx`, `ClassesPage.tsx`, `SpeechExamPage.tsx`, `ScoringPage.tsx`)
- `src/api/commands.ts`, `src/api/types.ts` — frontend Tauri client deseni
- `src/state/`, `src/app/` — routing ve state desenleri

## Raporun ana gereksinimleri (plan bunları karşılamalı)

1. Performans görevi yazılı sınav akışından (PDF/OCR/QEP) BAĞIMSIZ ayrı bir akış; mevcut ders/sınıf/öğrenci organizasyonunu yeniden kullanır.
2. Beş parçalı yapı: Görev → Rubrik/Ölçek → Kanıt/Ürün → Öğrenci değerlendirmesi (öz/akran) → Öğretmen kararı (nihai puan öğretmende, AI yetkisiz).
3. Mevcut `ScoringRecord` kullanılmaz; yeni domain tipleri (görev, rubrik şablonu, öğrenci değerlendirmesi, öğretmen onayı).
4. Rubrik: 3-6 ölçüt, 3 veya 5 düzey, gözlenebilir tanımlar, sürümleme; ölçüt puanı + gerekçe + geri bildirim; eksik değerlendirme ≠ sıfır.
5. Öğretmen akışı 10 adım (raporda §6) — sınıf/tema/öğrenme çıktısı seçimi, görev şablonu, rubrik düzenleme, öğrenci görevlendirme (bireysel/grup), kanıt toplama, formlar, ölçüt işaretleme, geçici toplam + eksik uyarıları, onay, rapor.
6. İlk aşama kapsamı (raporda §9): 9. sınıf TDE yazılı + sözlü görev şablonları, öğretmen rubrik oluşturma, sınıf listesi üzerinden ölçek doldurma, öğretmen notu + geri bildirim, PDF/Excel raporu, rubrik sürümleme ve onay. İkinci aşama (ürün dosyası, ses/video kanıtı, öz/akran, grup takibi) ve üçüncü aşama plana ayrı faz olarak yazılır.
7. Yapılmaması gerekenler (rapor §8) plana değişmez kural olarak girer.

## Plan formatı (docs/TYMM_PERFORMANCE_PLAN.md'ye yaz)

1. **Amaç, kapsam ve değişmezler** — rapordaki yasaklar dahil
2. **Mevcut mimariyle eşleştirme** — hangi mevcut tip/servis/komut/sayfa yeniden kullanılacak (dosya:satır referanslarıyla)
3. **Yeni veri modeli tasarımı** — Rust domain tipleri (isim, alanlar, varyantlar), ProjectStore şema etkisi, migration gereksinimi
4. **Fazlı uygulama planı** — her faz tek başına merge edilebilir; faz başına: kapsam, dokunulacak dosyalar, yeni dosyalar, Tauri komutları, frontend sayfaları, testler, doğrulama komutları
5. **Test stratejisi** — AGENTS.md §5-7 test seviyelerine uygun; Rust birim + komut kontrat + frontend bileşen testleri
6. **Riskler ve açık sorular**

## Kısıtlar

- AGENTS.md kurallarına uy (küçük doğru değişiklik, tek yazar ProjectStore, typed komutlar, geri bildirim kuralları).
- Plan Türkçe yazılacak.
- Kod dosyası değiştirme, komut çalıştırma YOK. Yalnızca `docs/TYMM_PERFORMANCE_PLAN.md` yazılacak.
