# Gerçek Proje Audit/Project Revision Divergence — Forensic Diagnosis Raporu (PERFORMANS ODAKLI)

**Proje Kökü (Kod):** `/Users/kadir/Desktop/RubriKa/RubrikaV3`  
**Sorun Yaşanan Proje (Gerçek Veri — SALT OKUNUR):** `/Users/kadir/Documents/RubrikaV3/Projects/11_sınıf_semeli_edebiyat_2026-2027`  
**Backup Dizini:** `/Users/kadir/Documents/RubrikaV3/VerifiedBackups/`  
**Tarih / Zaman:** 2026-08-08T22:06:35+03:00  
**Mod:** SALT OKUNUR FORENSIC DIAGNOSIS (HİÇBİR RECOVERY/MUTATION UYGULANMAMIŞTIR)

---

## 1. Current Integrity State

Uygulamanın `DataLossPreflightReport` ve canlı diagnostics motoru tarafından tespit edilen mevcut bütünlük durumu:

```
isProjectWriteBlocked = true
preflight.isLoading = false, isError = false, status = success, hasData = true
decision = DO_NOT_OPEN_FOR_WRITING
initializationWriteAllowed = false
unverifiedWritesAllowed = true
resolvedWriteBlockReason = "İşlem geçmişi doğrulanmadı."
```

### Canlı Backend Blocker'ları:
1. **audit chain geçersiz:** "İşlem geçmişi doğrulanamadı."
2. **audit/project revision divergence var:** "Proje ve işlem geçmişi aynı revision'da değil."
3. **active audit/project revision divergence var:** "Aktif işlem geçmişi mevcut proje durumu ile eşleşmiyor."

### Bütünlük Durumu Detayı:
- **Audit Hash Chain Fiziksel Bütünlüğü:** `/Users/kadir/Documents/RubrikaV3/Projects/11_sınıf_semeli_edebiyat_2026-2027/logs/audit.jsonl` dosyasındaki mevcut 6 kaydın TAMAMI SHA-256 canonical body hash doğrulamalarından ve `previous_record_hash` zincir bağlama testlerinden %100 BAŞARIYLA geçmiştir (`tamper_count = 0`). Fiziksel hash bozulması, karartma veya silinme tespit edilmemiştir.
- **Revizyon Sapması (Divergence):** Projenin `project.json` dosyasındaki `storageRevision` değeri **44**'tür. Buna karşın `logs/audit.jsonl` dosyasındaki son denetim kaydının `next_revision` değeri **7**'dir (`divergence = 37 revizyon`).
- **Yazma Engeli Mekanizması:** `AppLayout.tsx` ve `projectSafety.ts` bileşenleri `resolvedWriteBlockReason = "İşlem geçmişi doğrulanmadı."` ve `decision = "DO_NOT_OPEN_FOR_WRITING"` nedeniyle projeye yönelik tüm kullanıcı UI mutasyonlarını capture fazında `preventDefault()` ve `stopPropagation()` ile engellemektedir.

---

## 2. Project Revision

Proje durumunu temsil eden `project.json` dosyasının canonical revizyon parametreleri:

- **Dosya Yolu:** `/Users/kadir/Documents/RubrikaV3/Projects/11_sınıf_semeli_edebiyat_2026-2027/project.json`
- **Canonical Storage Revision:** `44`
- **Proje Kimliği (ID):** `ddf56fce-a848-4fb1-ace7-9be706e0729f`
- **Oluşturulma Tarihi (`createdAt`):** `2026-08-03T12:40:12.078661+00:00`
- **Son Güncellenme Tarihi (`updatedAt`):** `2026-08-08T12:12:06.075470+00:00`
- **SHA-256 Parmak İzi (Fingerprint):** `6990a2f46f5715d1639746b80e6f13b42139b16b79584eef85bf90f4975a046b`
- **Yapısal Geçerlilik:** %100 Geçerli (JSON şeması eksiksiz, 1 adet yazılı sınav aktivitesi ve 1 adet 11-C sınıfı tanımlı).

---

## 3. Audit Head

`logs/audit.jsonl` denetim kütüğünün en sonundaki kaydın (Head) teknik detayları:

- **Dosya Yolu:** `/Users/kadir/Documents/RubrikaV3/Projects/11_sınıf_semeli_edebiyat_2026-2027/logs/audit.jsonl` (Line 6)
- **Kayıt Sayısı:** 6 adet satır / kayıt
- **Audit Head Revision (`nextRevision`):** `7` (`previousRevision`: 6)
- **Event ID:** `8dd0ba8e-1fcf-4397-b8da-dbdb0063728e`
- **İşlem (`operation`):** `student_scan_batch_imported`
- **Zaman Damgası (`timestamp`):** `2026-08-03T15:22:28.606823+00:00`
- **Transaction ID:** `f7acdf92-10fc-42c8-8e3f-962599265df0`
- **Previous Record Hash:** `c04fb08eb79c723167e2f74bbd3934d3727ea43d5b8389f97b707ac2dc61e011`
- **Record Hash:** `9d1f2199fa662502551bd1996abb086b570b25ddb54de8d5c5645d0748a0d40c`
- **Doğrulama Durumu:** Kendi içinde hash ve zincir bağı geçerli; fakat `project.storage_revision` (44) ile 37 revizyon uyumsuzdur.

---

## 4. Transaction Journal Head

`logs/transactions/` dizinindeki 53 adet işlem günlüğünün en sonundaki kaydın (Head) teknik detayları:

- **Dosya Yolu:** `/Users/kadir/Documents/RubrikaV3/Projects/11_sınıf_semeli_edebiyat_2026-2027/logs/transactions/d405cef6-28a2-4c8e-84c9-a146591d27bd.json`
- **Toplam Transaction Dosyası:** 53 adet
- **Transaction ID:** `d405cef6-28a2-4c8e-84c9-a146591d27bd`
- **İşlem (`operation`):** `legacy_snapshot_merge`
- **Beklenen Revizyon (`expectedRevision`):** `43`
- **Hedef Revizyon (`targetRevision`):** `44`
- **Durum (`status`):** `complete`
- **Oluşturulma Zamanı:** `2026-08-08T12:12:06.066703+00:00`
- **Son Güncelleme Zamanı:** `2026-08-08T12:12:06.076395+00:00`
- **Correlation ID:** `7a2b2037-684d-444a-83e2-aa5853a2cba0`

---

## 5. First Divergence

Denetim kütüğü ile proje revizyonu arasındaki ilk bozulma ve ayrışma noktası:

- **İlk Ayrışma Revizyonu:** Revizyon `4 -> 5` (2026-08-03T14:59:32)
- **Ayrışmaya Neden Olmayan İşlem:** `logs/audit.jsonl` Line 5 (`assessment_activity_created`) revizyon `3 -> 4` geçişini başarıyla kaydetmiştir.
- **İlk Atlanan İşlem:** `2026-08-03T14:59:32.039435+00:00` tarihinde gerçekleşen `legacy_snapshot_merge` (Tx: `5592cf3d-8841-40f4-b5dc-f21d6595ab90.json`, Rev: 4 -> 5, `sınav soruları.pdf` eklendi). Bu işlem `project.json` revizyonunu 5 yapmış ve transaction journal dosyasına `complete` olarak kaydetmiş, ancak `audit.jsonl` kütüğüne HİÇBİR DENETİM KAYDI YAZILMAMIŞTIR.
- **İkinci Atlanan İşlem:** `2026-08-03T15:10:27.604670+00:00` tarihinde gerçekleşen `legacy_snapshot_merge` (Tx: `fa2def19-14c6-4d58-a9c4-1259fdd0298d.json`, Rev: 5 -> 6, `rubrik.pdf` eklendi).
- **Atlamalı Audit Kaydı (Line 6):** `2026-08-03T15:22:28.606823+00:00` tarihinde `student_scan_batch_imported` işlemi `previousRevision: 6`, `nextRevision: 7` olarak audit kütüğüne yazılmış; böylece audit zincirinde revizyon 4'ten 6'ya atlama (gap) oluşmuştur.
- **Ana Ayrışma Zinciri:** Revizyon `7 -> 44` arasındaki 37 mutasyon (`queue_preview_generation`, `activate_preview_generation`, `legacy_snapshot_merge`) transaction journal'da `complete` olarak sonlanmış fakat audit kütüğüne hiç yazılmamıştır.

---

## 6. Timeline

Projede gerçekleşen tüm işlemlerin kronolojik akış özeti:

| Tarih / Zaman | İşlem (`operation`) | Transaction ID | Revizyon Geçişi | Audit Kaydı Durumu |
|---|---|---|---|---|
| `2026-08-03T12:40:12` | `project_created` | `698aeba8-a865-433f-9e21-bcd40acac177` | None -> 0 | **VAR** (Audit Line 1) |
| `2026-08-03T13:11:47` | `legacy_snapshot_merge` (Sınıf 11-C eklendi) | `e0464c8a-6b79-477c-b197-56c52b30c7f3` | 0 -> 1 | Atlandı (Sonradan repair edildi) |
| `2026-08-03T13:55:05` | `audit_revision_repaired` | `140bd230-4f34-4573-a4a8-75f48b2e8ed7` | 0 -> 1 | **VAR** (Audit Line 2) |
| `2026-08-03T13:57:14` | `class_student_created` (Öğrenci Kadir Niğdelioğlu) | `7ad9fb6c-e416-44fd-aba0-648827f53fc2` | 1 -> 2 | **VAR** (Audit Line 3) |
| `2026-08-03T13:58:04` | `legacy_snapshot_merge` (Ders bilgileri) | `a58db113-6f30-4232-8695-e9e0610f5a03` | 2 -> 3 | Atlandı (Sonradan repair edildi) |
| `2026-08-03T14:06:29` | `audit_revision_repaired` | `79241817-376a-4bfb-a012-7c04bc434fbc` | 2 -> 3 | **VAR** (Audit Line 4) |
| `2026-08-03T14:58:58` | `assessment_activity_created` (Yazılı sınav) | `c9f33be5-4f10-491a-8b2b-3e0137777d03` | 3 -> 4 | **VAR** (Audit Line 5) |
| `2026-08-03T14:59:32` | `legacy_snapshot_merge` (`sınav soruları.pdf`) | `5592cf3d-8841-40f4-b5dc-f21d6595ab90` | 4 -> 5 | **YOK (İLK DIVERGENCE)** |
| `2026-08-03T15:10:27` | `legacy_snapshot_merge` (`rubrik.pdf`) | `fa2def19-14c6-4d58-a9c4-1259fdd0298d` | 5 -> 6 | **YOK** |
| `2026-08-03T15:22:28` | `student_scan_batch_imported` (`öğrenci cevabı.pdf`) | `f7acdf92-10fc-42c8-8e3f-962599265df0` | 6 -> 7 | **VAR** (Audit Line 6, `prevRev: 6`) |
| `2026-08-03T15:22:28` — `2026-08-03T16:30:27` | PDF Önizleme Mutasyonları (34 adet `ProjectStore::mutate`) | Çeşitli Tx ID'ler (`88139ee0...`, `ece1d798...` vb.) | 7 -> 41 | **YOK** (Tüm Tx `complete`, audit atlandı) |
| `2026-08-08T12:11:51` — `2026-08-08T12:12:06` | Snapshot güncellemeleri (3 adet `legacy_snapshot_merge`) | `6ae1ed5a...`, `0c4a53cd...`, `d405cef6...` | 41 -> 44 | **YOK** (Tüm Tx `complete`, audit atlandı) |

---

## 7. Probe-Created Activity Analysis (Performans Odaklı İnceleme)

Önceki hata ayıklama turunda bir ajanın doğrudan `PerformanceService::create_performance_task` / `ProjectStore` çağrısı yaparak `activity_id = 4ee99596-...` ile revizyon **45** oluşturduğu iddiası incelenmiştir:

- **Şüpheli İşlem:** `create_performance_task`, Activity ID: `4ee99596-...`, Raporlanan Revizyon: `45`.
- **Fiziksel Veri İnceleme Çıktıları (SALT OKUNUR):**
  1. **`project.json` Dosyası:** `assessmentActivities` dizisinde **YALNIZCA 1 ADET** etkinlik bulunmaktadır (`id: 58e52736-7a92-4956-a03b-a411f6fe4d54`, title: "1. Dönem 1. Yazılı Sınav", `assessmentType: "written"`). Probe tarafından oluşturulduğu iddia edilen `4ee99596-...` ID'li performans görevi **GERÇEK PROJEDE MEVCUT DEĞİLDİR**.
  2. **Storage Revision:** `project.json` içindeki `storageRevision` değeri **44**'tür. Revizyon **45 değildir**.
  3. **Transaction Journal (`logs/transactions/`):** 53 adet transaction dosyasının en sonuncusu `d405cef6-28a2-4c8e-84c9-a146591d27bd.json` olup revizyon **43 -> 44** geçişine aittir. Revizyon 45 veya `create_performance_task` için oluşturulmuş hiçbir transaction dosyası **MEVCUT DEĞİLDİR**.
  4. **Audit Log (`logs/audit.jsonl`):** Revizyon 45 veya `create_performance_task` işlemine ait hiçbir denetim kaydı **BULUNMAMAKTADIR**.
- **Kesinleşmiş Kanıt:** Önceki ajanın yaptığı probe işlemi bellek içi (in-memory) bir nesne üzerinde çalışmış veya hata alıp rollback olmuş; disk üzerindeki gerçek proje dosyalarına (`project.json`, `audit.jsonl`, `logs/transactions/`) **FİZİKSEL OLARAK HİÇBİR YAZMA YAPMAMIŞTIR**. Probe işlemi revizyon sapmasına neden OLMAMIŞTIR.

---

## 8. Backup Inventory

- **Verified Backups Dizin Durumu:**  
  Yol: `/Users/kadir/Documents/RubrikaV3/VerifiedBackups/`  
  Durum: Dizin fiziksel olarak diskte henüz mevcut değildir (`0` adet doğrulanmış yedek).
- **Proje Klasörü İçi Migration Yedekleri:**  
  Yol: `/Users/kadir/Documents/RubrikaV3/Projects/11_sınıf_semeli_edebiyat_2026-2027/`  
  Durum: Klasör içinde `.bak` veya `project.json.migration.*` yedeği bulunmamaktadır.
- **Harici Yedeğe Erişim:** Bulunmamaktadır.

---

## 9. Root Cause (Performans Mimarisi ve Audit Kapsama Eksikliği)

Forensic araştırmanın ortaya çıkardığı **KESİN KÖK NEDEN (ROOT CAUSE)**:

### 1. Performans Servis Katmanı Mimarisi (`PerformanceService`):
- `src-tauri/src/services/performance_service.rs` dosyası incelendiğinde, `PerformanceService` yapısının `project_store: ProjectStore` ve `assessment_organization_service: Arc<AssessmentOrganizationService>` alanlarına sahip olduğu, fakat **`audit_service: Arc<AuditService>` ALANININ BULUNMADIĞI** kanıtlanmıştır.
- `create_performance_task`, `update_performance_task`, `publish_performance_rubric`, `save_performance_assessment`, `approve_performance_assessment`, `set_performance_assessment_status` fonksiyonlarının TAMAMI mutasyonlarını `ProjectStore::mutate(...)` üzerinden gerçekleştirir.

### 2. `ProjectStore::mutate` Mimarisi:
- `src-tauri/src/services/project_store.rs` içindeki `mutate` metodu:
  1. Projenin storage revizyonunu otomatik artırır (`storage_revision += 1`).
  2. Transaction journal dosyası oluşturur (`transaction_journal::begin`).
  3. `project.json` dosyasını diske atomik yazar.
  4. Transaction durumunu `complete` yapar (`transaction_journal::update`).
  5. **ANCAK `ProjectStore::mutate` METODU `AuditService::append` FONKSİYONUNU ÇAĞIRMAZ.** Denetim kaydı yazımı tamamen çağıran servis katmanının sorumluluğuna bırakılmıştır.

### 3. Sistemik Ayrışma Mekanizması:
- `ScoringAnchorService`, `SpeakingExamService` ve `AssessmentOrganizationService` gibi servisler bazı kritik işlemlerde `AuditService::append` çağırırken; `PerformanceService` mimarisinde denetim servisi hiç tanımlanmamış, PDF önizleme render kuyrukları ve snapshot birleştirmeleri de denetim kütüğüne kaydolmamıştır.
- Sonuç olarak `2026-08-03` tarihinden itibaren 37 adet mutasyon `storage_revision` değerini 7'den 44'e ilerletmiş, transaction journal'da 53 işlemin tamamı `complete` olmuş, ancak `audit.jsonl` 7. revizyonda kalmıştır. `DataLossPreflightReport` bu 37 revizyonluk boşluğu `audit/project revision divergence var` olarak tespit etmiş ve projeyi `DO_NOT_OPEN_FOR_WRITING` moduna almıştır.

---

## 10. Safe Recovery Alternatives

Kanıta dayalı ve veri bütünlüğünü %100 koruyan kurtarma seçenekleri:

### Seçenek A (TAVSİYE EDİLEN - Canonical Re-anchoring via Verified Transaction Journal):
- **Mantık:** `logs/transactions` dizininde 0'dan 44'e kadar 53 işlemin TAMAMI mevcuttur ve durumları `complete` olarak doğrulanmıştır. `project.json` verisi revizyon 44 itibarıyla %100 tutarlı ve aktiftir. Sahte/uydurma öğretmen denetim kaydı eklemek yerine, `AuditService::verify_chain_against_project` (Satır 413-446) tarafından yerleşik olarak desteklenen kanonik `recovery_anchor` kaydı `audit.jsonl` kütüğünün sonuna eklenir.
- **Teknik İşlem:** `logs/audit.jsonl` dosyasının en sonuna `previous_record_hash: 9d1f2199fa662502551bd1996abb086b570b25ddb54de8d5c5645d0748a0d40c` ile bağlanan, `operation: "recovery_anchor"`, `previous_revision: 7`, `next_revision: 44` ve `projectFingerprint: 6990a2f46f5715d1639746b80e6f13b42139b16b79584eef85bf90f4975a046b` içeren 1 adet doğrulanmış kanonik hizalama kaydı yazılır.
- **Risk:** SIFIR. Proje verisine (`project.json`) ve geçmiş 6 denetim kaydına dokunulmaz.

### Seçenek B (Transaction Journal Replay into Audit Log):
- **Mantık:** Revizyon 7 ile 44 arasındaki 37 adet tamamlanmış transaction journal kaydı taranarak, her biri için `audit.jsonl` kütüğüne karşılık gelen `audit_transaction_reconciled` sentetik kayıtları eklenir.
- **Risk:** DÜŞÜK. Ancak geçmişe dönük 37 adet sentetik audit nesnesi üretilmesi gerekir.

### Seçenek C (Manuel Project Storage Revision Rollback - KESİNLİKLE REDDEDİLDİ):
- **Mantık:** `project.json` içindeki `storageRevision` değerini 44'ten 7'ye düşürmek.
- **Reddedilme Nedeni:** `project.json` içinde revizyon 7'den sonra eklenen belgeler, PDF önizleme nesneleri ve workflow durumları bulunmaktadır. Revizyonu geriye çekmek veri kaybı riski oluşturur.

---

## 11. Recommended Recovery

**Seçenek A (Canonical Re-anchoring)** uygulanmalıdır.

### Kurtarma Sonrası Alınacak Kod Önlemi (Gelecek Turlar İçin):
- `PerformanceService` yapısına `audit_service: Arc<AuditService>` eklenmeli ve performans mutasyonlarının (`create_performance_task`, `publish_performance_rubric`, `save_performance_assessment`, `approve_performance_assessment`) `append_transactionally` üzerinden audit kütüğüne yazılması sağlanmalıdır.

---

## 12. Exact Files / State That Would Change

Recovery onaylandığı takdirde değişecek KESİN dosyalar ve durumlar:

### Değişecek Dosya (YALNIZCA 1 ADET):
1. `/Users/kadir/Documents/RubrikaV3/Projects/11_sınıf_semeli_edebiyat_2026-2027/logs/audit.jsonl`  
   *(Dosyanın sonuna 1 adet kanonik `recovery_anchor` kaydı eklenecektir).*

### DOKUNULMAYACAK / DEĞİŞMEYECEK DOSYALAR:
- `project.json` (Değiştirilmeyecek, revizyon 44 olarak aynen korunacak)
- `logs/transactions/*.json` (53 transaction dosyası aynen korunacak)
- `documents/*` (Aynen korunacak)
- `cache/*`, `crops/*`, `outputs/*` (Aynen korunacak)
- Herhangi bir uygulama kodu veya test dosyası (Bu turda değiştirilmeyecektir).

---

## 13. Rollback Plan

Kurtarma işlemi (Seçenek A) gelecekte uygulanmadan önce:

1. Proje klasörünün tam bir doğrulanmış yedeği `/tmp/rubrika_recovery_backup_11_sinif/` altına alınacaktır.
2. Herhangi bir uyumsuzluk durumunda `/Users/kadir/Documents/RubrikaV3/Projects/11_sınıf_semeli_edebiyat_2026-2027/logs/audit.jsonl` dosyasının orijinal 6 satırlı kopyası geri yüklenecektir.

---

## 14. Acceptance Criteria

Kurtarma işlemi uygulandığında aşağıdaki kriterlerin TAMAMI sağlanmalıdır:

1. `auditChainValid == true`
2. `projectRevision == auditProjectRevision` (`44 == 44`)
3. `activeAuditRevision == projectRevision` (`44 == 44`)
4. `pendingMigration == false`
5. `secondWriterDetected == false`
6. `incompleteTransactionCount == 0`
7. `ambiguousTransactionCount == 0`
8. `preflight.decision != DO_NOT_OPEN_FOR_WRITING` (Yazma korumasının tam olarak kalkması)

---

## Final Karar Formatı

```
STATUS: ROOT_CAUSE_CONFIRMED_PREEXISTING_DIVERGENCE
RECOVERY_STATE: RECOVERY_READY_FOR_APPROVAL
```
