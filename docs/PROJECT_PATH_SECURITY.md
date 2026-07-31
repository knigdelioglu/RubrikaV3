# Project and Managed-File Path Security

Bu belge Faz 1 yol güvenliği sözleşmesini tanımlar. Amaç, kullanıcı tarafından açılan proje klasörünü tek güvenilir yazma kökü yapmak; `project.json` metadata'sını yetki kaynağı olmaktan çıkarmak; proje içi belge ve artefact yollarında traversal, symlink escape ve sessiz overwrite risklerini engellemektir.

## Trusted project root

`TrustedProjectRoot`, kullanıcının gerçekten seçtiği klasörden (veya o klasördeki `project.json` dosyasından) oluşturulur:

```text
selected path -> absolute -> canonicalize -> runtime trusted root
```

Yapı canonical absolute root, canonical `project.json`, managed path üretimi, read containment ve write containment yardımcılarını sağlar. `ProjectStore` açık proje oturumu için bu kökü tutar. `Project.root_path` hiçbir zaman save, import, preview, OCR, model input, backup, export veya log hedefi seçmez.

## Stored `root_path` metadata

`project.json.root_path` yalnız legacy/display metadata'dır. Runtime kökü değiştiremez. Stored değer açılan canonical kökten farklıysa proje açılır ve diagnostic warning üretilir; sonraki save açılan köke yapılır. Bu, taşınmış projelerin yeni konumundan güvenle açılmasını sağlar.

## Managed relative paths

`ManagedProjectPath` canonical serialized biçimde yalnız relative path kabul eder:

```text
documents/exam/questions.pdf
outputs/previews/document-1/page-001.png
```

Absolute Unix/macOS ve Windows drive/prefix yolları, root component, `..`, boş/yalnız `.` yolları, null byte ve backslash reddedilir. Domain document kaydı yeni importlarda yalnız relative `stored_path` saklar.

## Read and write containment

Okuma akışı path'i parse eder, trusted root ile birleştirir, canonical hedefi doğrular, path-component containment (`strip_prefix`) uygular ve regular file kontrolü yapar. Yazma akışı relative target'ı doğrular, eksik parent'ları trusted root altında tek tek oluşturur, canonical parent'ı kontrol eder, symlink parent/target'ı reddeder ve temp/staging/atomic rename hedeflerini de aynı resolver'dan geçirir.

`starts_with` string kontrolü güvenlik kararı olarak kullanılmaz. Mevcut atomic JSON write davranışı korunur; temp dosyası regular non-symlink ve trusted parent içinde olmak zorundadır.

## Symlink policy

Yönetilen dosya ve klasörlerde symlink politikası deny-by-default'tur. Proje içine işaret eden symlink dahil managed read/write akışında symlink component reddedilir; böylece platformlar arası politika açık ve deterministiktir. Dışarı işaret eden file veya parent symlink `ManagedPathSymlinkEscape` ile durdurulur.

## External import source vs managed document

Dosya seçiciyle açıkça seçilen import source dışarıda olabilir ve yalnız import sırasında okunur. Backend dosyayı proje içindeki `documents/...` managed hedefe `create_new` semantiğiyle kopyalar. Domain kaydı ve sonraki PDF/OCR/model akışları yalnız bu relative managed kopyayı kullanır. Export destination ayrı bir external destination sözleşmesidir; managed artefact yolu değildir.

## Legacy adaptation

Legacy absolute document path yalnız canonical hedef gerçekten trusted root içindeyse relative managed path'e çevrilebilir. Eski root prefix'i ancak containment ve regular-file/symlink kontrollerinden sonra kaldırılır. Proje dışındaki veya belirsiz yollar otomatik import edilmez; unresolved warning/diagnostic olarak kalır ve OCR/PDF/model girdisi olarak açılamaz. Adaptation load sırasında in-memory ve idempotenttir; proje yalnız açıldı diye otomatik rewrite edilmez.

## Tauri asset scope

Tauri asset protocol scope yalnız uygulamanın yönetilen varsayılan proje alanı olan `$HOME/Documents/RubrikaV3/Projects/**/*` ile sınırlandırılmıştır. Desktop altındaki genel RubriKa kapsamı kaldırılmıştır. Runtime dosya yolu bu scope dışında olduğunda backend resolver yine erişimi engeller; dış konumlar için güvenli backend file-serving/temporary asset çözümü ayrıca yapılmadan Tauri UI asset olarak varsayılmaz.

## Typed errors and doctor counters

Path failures `AppErrorCode`/frontend error DTO içinde typed'dir: `PROJECT_ROOT_MISMATCH`, `PROJECT_ALREADY_EXISTS`, `PROJECT_DIRECTORY_NOT_EMPTY`, `UNSAFE_MANAGED_PATH`, `MANAGED_PATH_OUTSIDE_PROJECT`, `MANAGED_PATH_SYMLINK_ESCAPE` ve `LEGACY_DOCUMENT_PATH_UNRESOLVED`. Teacher UI yalnız güvenli açıklama görür; absolute path teknik details/audit alanında kalır.

Doctor PII içermeyen sayaçlar üretir: `project_root_metadata_mismatch`, `unsafe_document_path_count`, `unresolved_legacy_document_path_count`, `external_managed_document_path_count`, `symlink_escape_count`.

## Security regression coverage

Rust tests malicious `root_path` redirect, target project JSON immutability, moved project warning, non-empty/existing-project rejection, nested relative paths, absolute/traversal rejection, read/write symlink escape, legacy adaptation, import copy/relative storage, atomic temp symlink protection and Tauri scope regressionını kapsar. Gerçek kullanıcı projesi test edilmez; fixture kopyası ve test-owned temporary directories kullanılır.
