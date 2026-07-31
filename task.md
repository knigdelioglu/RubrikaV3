# Görev kaydı

## Ortak sınıf servisi ve çoklu sınav organizasyonu

- Merkezi sınıf okuması mevcut `SchoolClassService` üzerinde tutuldu.
- Aktif ders–sınıf görevlendirmeleri `TeachingAssignment` olarak eklendi; sınav sınıf seçimleri bu kaynaktan filtreleniyor.
- `AssessmentActivity` ortak sınav kaydı ve `AssessmentClassApplication` sınıf uygulaması olarak ayrıldı.
- Yazılı, dinleme ve konuşma türleri ile `written`/`speaking` workflow family eşlemesi eklendi.

## AssessmentActivity/ClassApplication canonical speaking kaydı

- Konuşma setup ve execution akışı `AssessmentActivity.classApplications` kaynağına bağlandı; yeni speaking write yolunda bağımsız `assignedClassIds` kullanılmıyor.
- `SpeakingAttempt` activity/application/school-class referansları ve config snapshot’ı taşıyor; `ProjectStore` canonical persistence ve runtime compatibility projection’ını yönetiyor.
- Legacy `assignedClassIds` unambiguous migration, unresolved warning, idempotence ve ProjectStore reload testleri eklendi.
- Organization command sözleşmeleri, doctor canonical integrity sayaçları ve ilgili akış haritaları güncellendi.
- Ortak ve sınıfa özel belge ID bağlama seviyeleri eklendi.
- JSON ProjectStore açılış migration’ı eski sınıf alanlarını backfill ediyor; belirsiz eski konuşma sınavları unresolved bırakılıyor.
- UI organizasyon sayfası ve Kurulum → Sınıflar bağlantısı eklendi.

Doğrulama sonucu ve gerçek UI smoke sonucu teslim raporunda güncellenecektir.
