# Assessment Organization

Bu checkout TeacherOS `crates/teacheros-*` ve SQL migration katmanını içermediği için aynı domain kararı RubrikaV3 eşdeğerleriyle uygulanır: `SchoolClass`, `SchoolClassService`, `ProjectStore` ve `AssessmentActivity`.

## Ownership and navigation

`/project/:projectId/classes` canonical proje kurulumu workspace’idir. Proje bilgileri, `SchoolClass` ve `TeachingAssignment` kayıtları burada yönetilir. `/project/:projectId/activities` yalnız mevcut sınavları listeler/yönetir ve kullanıcı aksiyonuyla açılan create mode üzerinden `AssessmentActivity` oluşturur; görevlendirme formu bu sayfada bulunmaz.

Öğretmen akışı: proje kurulumu → sınıflar ve görevlendirmeler → sınav organizasyonu → belgeler → sınav paketi → öğrenci işlemleri → notlandırma.

## Merkezi sınıf servisi

`SchoolClass`/`SchoolClassService` projenin tek merkezi sınıf kaynağıdır. `name` ve `displayName` sınıf etiketini, `academicYearId` eğitim yılını, `gradeLevel` ve `section` sınıf kimliğinin parçalarını, `status` aktif/arşiv durumunu taşır. Normal akışta hard-delete yoktur. `list_assessment_classes`, aktif eğitim yılı + ders + sınıf düzeyi + aktif `TeachingAssignment` kesişiminden seçim döndürür.

## Tek ana sınav ve sınıf uygulaması

`AssessmentActivity` tek ortak sınav kaydıdır. Ana tekillik anahtarı:

`academicYearId + courseId + gradeLevel + term + assessmentType + sequenceNumber`

Create mode eğitim yılı ve ders bilgisini aktif `TeachingAssignment` kayıtlarından, sınıf düzeyini seçili aktif sınıflardan türetir. Öğretmen ders kodu, ders adı veya sınıf düzeyi için tekrar input doldurmaz; duplicate key, görevlendirme uygunluğu ve farklı düzey kontrolü backend’de son kez doğrulanır.

Şube bu anahtarın parçası değildir. Her seçilen sınıf için aynı activity altında bir `ClassApplication` bulunur; aynı `activityId + schoolClassId` yalnız bir kez olabilir. `Project.assessmentActivities[*].classApplications` speaking dahil tüm sınav–sınıf ilişkisinin canonical kaynağıdır.

Activity seviyesinde ortak başlık, görev, süre, rubrik/scoring policy snapshot’ı ve ortak belgeler tutulur. ClassApplication seviyesinde `schoolClassId`, öğrenci kapsamı, sınıfa özel belgeler ve ilerleme tutulur. Ortak görev/rubrik sınıf başına kopyalanmaz.

## Speaking yürütmesi ve attempt

`SpeechExamPage` yalnız `assessmentActivityId` ile açılan activity’nin aktif `classApplications` listesini gösterir. Sınıf değişimi `classApplicationId` taşır; öğrenci listesi merkezi `SchoolClass` roster’ından backend tarafından çözülür. Yeni attempt en az `assessmentActivityId`, `classApplicationId`, `schoolClassId`, `studentId` ve `speakingConfigSnapshot` taşır. Backend activity/application sahipliğini, sınıf aktifliğini ve öğrenci üyeliğini doğrular.

Konuşma motorunun mevcut runtime aggregate’ı (`SpeakingExam`) yalnız compatibility projection’dır. `ProjectStore.save_project` attempt’leri activity altındaki class application’a yazar ve linked runtime attempt listesini JSON’dan çıkarır; açılışta read model olarak yeniden hydrate eder. Bu nedenle üretim kaydında bağımsız bir speaking sınıf listesi doğruluk kaynağı değildir.

## Sınav türleri ve workflow family

- `written` → `written`
- `listening` → `written`
- `speaking` → `speaking`

Dinleme ayrı görünen tür ve bağımsız sıra alanıdır; yazılı soru/belge workflow altyapısını yeniden kullanır. Dinleme metadata’sı ses belgesi, metin, dinletme sayısı, süre ve yönergeyle sınırlıdır; medya oynatıcı, soru çıkarma ve puanlama bu organizasyon değişikliğinin kapsamında değildir.

Sıra numarası yıllık toplam değildir. Dönem, tür ve sınıf düzeyi kapsamında ilerler; UI varsayılan yuvaları önerir, üçüncü yazılı gerçek ihtiyaç olduğunda ayrıca kaydedilir.

## Belgeler

Activity-level belgeler `commonDocumentIds` altında ortak sınava bağlanır. Class-application-level belgeler `ClassApplication.documentIds` altında ilgili uygulamaya bağlanır. UI yalnız bu activity’ye bağlı sınıf uygulamalarını sunar; bağlı olmayan sınıf için belge ilişkisi uydurulmaz.

## Legacy assignedClassIds ve migration

Eski `SpeakingExam.assignedClassIds`/`classId` alanları deserialize edilebilir ancak yeni production write yolunda yazılmaz. `ProjectStore` açılışında yalnız unambiguous speaking kayıtları uygun `AssessmentActivity` ve `SchoolClass` ile bir defa `ClassApplication`’a dönüştürülür; duplicate application üretilmez. Birden fazla uygun activity, eksik sınıf veya eksik kimlik varsa ilişki unresolved bırakılır ve migration warning üretilir; yanlış activity seçilmez. Migration idempotenttir, atomik JSON yazımı ve timestamp’li backup kullanır.

Bu checkout’ta SQL migration veya veritabanı foreign key’i yoktur. Integrity kuralları `AssessmentOrganizationService`, `SpeakingExamService` ve `ProjectStore` seviyesinde merkezi olarak uygulanır: activity/application tekilliği, workflow uyumu, geçerli sınıf, attempt üyeliği ve attempt içeren uygulamanın silinememesi.

## Performans değerlendirme (TYMM) — KALDIRILDI (REMOVED, 2026-08-08)

TYMM Performans Değerlendirme modülü RubrikaV3'ten tamamen kaldırılmıştır.
`AssessmentType::Performance`, `PerformanceService`, `PerformanceDetails` ve
`ClassApplication.performanceAssessments` artık aktif workflow'da yoktur.
Eski test projelerinin açılışı için `AssessmentType::LegacyPerformance`
tombstone variantı (serde `alias = "performance"`) tutulur; bu tür aktiviteler
yalnız deserialize edilir ve hiçbir ekran bu türü aktif bir iş akışı olarak açmaz.

## RubrikaV3 sınırı

Bu model yalnız sınav etkinliği, merkezi sınıf, ders–sınıf görevlendirmesi, sınıf uygulaması, belge bağlama, tarih/durum ve öğretmen iş akışını yönetir. Soru çıkarma, cevap anahtarı/rubrik üretme, öğrenci cevabı, otomatik notlandırma ve sınav başarı analizi RubrikaV3 kapsamına eklenmemiştir.


## 3-Adımlı Ders Alanı Kurulumu (Canonical Flow)

`/project/:projectId/classes` üzerindeki `Sınıflar ve Görevlendirmeler` sekmesi 3 açık adıma ayrılmıştır:

1. **Ders bilgileri:** Ders kodu (`courseId`), ders adı (`courseName`), ve eğitim yılı (`academicYearId`). Ders bilgisi `update_course_info` komutu ile kaydedilir.
2. **Sınıflar:** Merkezi `SchoolClass` kayıtlarının oluşturulması ve yönetimi (`create_school_class`).
3. **Ders–sınıf görevlendirmeleri:** Ders ve sınıflar hazır olduğunda sınıfların tek/çoklu eylemle derse görevlendirilmesi (`batch_create_teaching_assignments`).

Sınavlar sayfasından eksik kurula doğrudan yönlendirme query parametreleri:
- `/project/:projectId/classes?setup=course` (1. Adım: Ders bilgileri)
- `/project/:projectId/classes?setup=classes` (2. Adım: Sınıflar)
- `/project/:projectId/classes?setup=assignments` (3. Adım: Görevlendirmeler)

Kurulum tamamlanmadan Sınav Organizasyonu sayfasında yeni sınav oluşturma modalı açılamaz.
