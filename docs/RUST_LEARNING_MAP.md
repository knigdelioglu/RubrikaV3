# RubrikaV3 - Rust Öğrenme Haritası

Bu doküman, RubrikaV3 projesinin Rust mimarisini incelerken Rust'ın temel kavramlarını öğrenmek isteyenler için hazırlanmıştır. Gerçek proje kodlarından örnekler barındırır.

## Yeni Başlayanlar İçin Okuma Sırası
Projeyi anlamak ve Rust mimarisine adapte olmak için kodları aşağıdaki sırayla okumanız tavsiye edilir:

1. **`src-tauri/src/domain/project.rs` ve `workflow.rs`**: Projenin veri yapısını (`struct` ve `enum`) ve katı kurallarını anlamak için (Katman: Domain).
2. **`src-tauri/src/domain/errors.rs`**: Projedeki tüm hata tiplerini ve Rust'taki error handling mantığını (`Result`, `?` operatörü) görmek için.
3. **`src-tauri/src/services/project_store.rs`**: Domain objesinin dosyaya nasıl kaydedildiğini (Serialization) ve dosyadan okunduğunu görmek için. Rust file I/O (File System) örneğidir.
4. **`src-tauri/src/services/workflow_engine.rs`**: Merkezi iş mantığının, `mut` kullanmadan sadece referans (`&Project`) ile durumu nasıl hesapladığını (Saf Fonksiyon mantığı) incelemek için.
5. **`src-tauri/src/commands/project_commands.rs`**: Tauri'nin (Controller/API) frontend'den gelen isteği alıp service katmanını nasıl çağırdığını görmek için.
6. **`src/api/commands.ts` (Frontend)**: Tauri komutlarının React tarafında (TypeScript ile) nasıl bir Promise zinciriyle sarmalandığını anlamak için.
7. **`src-tauri/src/services/llama_server_gateway.rs`**: Dış dünyayla iletişim (HTTP reqwest, JSON parse). Bağımlılıkların tersine çevrilmesi (Dependency Inversion / Trait) örneği.
8. **`src-tauri/src/services/student_answer_ocr_service.rs` & `jobs/job_manager.rs`**: Asenkron işlemler (tokio `spawn`), iş parçacıkları arası veri taşıma (`clone`, `move`) ve UI'a olay (`Event`) fırlatma yapısını incelemek için (Katman: Async/Job).

---

## A. Rust Dosya/Modül Sistemi

RubrikaV3'te modül yapısı katmanlı mimariye (Domain, Service, Commands) göre kurulmuştur.

*   **`mod.rs` ne işe yarıyor?**
    Bir klasörün Rust için bir modül olduğunu belirtir. Örneğin `src-tauri/src/domain/mod.rs`, `domain` klasöründeki dosyaları toplar ve dışa aktarır.

*   **`pub mod` nedir?**
    `mod.rs` içinde yazılan `pub mod project;` ifadesi, "bu dizindeki `project.rs` dosyasını bul, derle ve içindeki public (`pub`) olan şeyleri dış dünyaya aç" demektir.

*   **`use crate::...` ne demek?**
    Projenin (crate'in) kökünden (bu durumda `src-tauri/src/lib.rs` veya `main.rs` hizasından) başlayarak bir modülü içeri aktarmaktır.
    *Örnek:* `use crate::domain::project::Project;` (`project.rs` içindeki `Project` struct'ını kullan)

*   **`crate`, `super`, `self` kavramları:**
    *   `crate`: Projenin kök dizini.
    *   `super`: Bir üst modül (parent directory).
    *   `self`: Mevcut modülün kendisi.

*   **Katman Ayrımı (Domain, Services, Commands):**
    *   `domain`: Asla Tauri'yi (`tauri::State`, `tauri::AppHandle`) veya dış servisleri (veritabanı, reqwest) `use` etmez. Sadece veri (struct, enum) barındırır.
    *   `services`: Domain'i kullanır, iş yapar. (Örn: `ProjectStore` dosyaya yazar).
    *   `commands`: Tauri API'sidir. Sadece Frontend'den gelen istekleri yakalar ve `services`'i çağırır.

---

## B. Struct / Enum / Impl

### Domain Struct Örneği
```rust
// Dosya: src-tauri/src/domain/project.rs
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    // ...
    pub workflow: WorkflowSnapshot,
}
```
*   **Nedir?** `struct` diğer dillerdeki sınıflara (class) veya veri kayıtlarına benzer.
*   **Neden böyle kullanılmış?** JSON'a dönüştürülmek (Serialize) ve okunmak (Deserialize) için `serde` makroları (`#[derive(...)]`) eklenmiştir. `camelCase` kuralı TypeScript tarafıyla tam uyum sağlar.

### Enum Status Örneği
```rust
// Dosya: src-tauri/src/domain/workflow.rs
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStage {
    DocumentsMissing,
    PdfPreviewMissing,
    // ...
    ScoringDone,
}
```
*   **Nedir?** Sadece belirli bir küme (set) değeri alabilen veri tipidir.
*   **Neden böyle kullanılmış?** `String` yerine `enum` kullanmak, geçersiz bir iş akışı durumunun var olmasını imkansız hale getirir (Derleyici hatası verir).

### Impl (Methodlar) Örneği
```rust
// Dosya: src-tauri/src/domain/project.rs
impl Project {
    pub fn invalidate_exam_package_if_frozen(&mut self, reason: &str) {
        // self referansı mut (değiştirilebilir) olarak alınır.
        if let Some(freeze) = self.exam_package_freeze.as_mut() {
            freeze.freeze_status = ExamPackageFreezeStatus::Invalidated;
        }
    }
}
```
*   **Nedir?** `struct` veya `enum`'lara method eklemek için kullanılır.
*   **Nasıl taklit etmeliyim?** Struct'ın kendi iç verisini değiştiren kurallar (`&mut self`) Domain içinde `impl` bloğuna konulmalıdır. İş mantığı servislere taşınmadan (anemik modelden kaçınarak) struct'ın kendisine ait olmalıdır.

---

## C. Result / Error Handling

RubrikaV3, özel bir hata yapısı (`AppError`) kullanır. Kullanıcı (öğretmen) asla ham teknik hatalar (örn: `std::io::Error`) görmez.

*   **`AppError` ve `AppErrorCode`:**
    ```rust
    // Dosya: src-tauri/src/domain/errors.rs
    pub struct AppError {
        pub code: AppErrorCode,
        pub message: String,
        pub recoverable: bool,
        pub suggested_action: Option<String>,
        pub technical_details: Option<String>,
        pub correlation_id: String,
    }
    ```
    Hatalar `AppErrorCode` enum'ı ile (Örn: `ModelTimeout`, `OcrFailed`) sınıflandırılır. Öğretmen için Türkçe `message` ve `suggested_action` bulunur. Geliştirici için `technical_details` eklenir.

*   **Result Dönüşü (İyi ve Kötü Örnekler):**
    ```rust
    // KÖTÜ: unwrap() kullanmak panik (çökme) yaratır.
    let proje = load_project("id").unwrap();

    // İYİ: Hata dönüşümü (map_err)
    // Dosya: src-tauri/src/services/llama_server_gateway.rs örneğinden esinlenildi:
    let image_bytes = std::fs::read(&path).map_err(|error| {
        AppError {
            code: AppErrorCode::PdfRenderFailed,
            message: "Görsel okunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("PDF'i tekrar yükleyin.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        }
    })?;
    // Sona eklenen `?` operatörü, hata varsa hemen fonksiyondan Err() dönmesini sağlar.
    ```

---

## D. Async / Jobs / Long-Running Task

Uzun süren işler (OCR, PDF oluşturma) arayüzü dondurmamalıdır. Tauri command'leri hızlıca geri dönmelidir.

*   **Job Nedir?**
    RubrikaV3'te uzun bir işlem (Örn: OCR) `JobManager` kullanılarak başlatılır. Backend'de tokio thread'inde asenkron olarak çalışır ve Frontend'e Tauri Events üzerinden "ilerleme" bildirir.
*   **Nasıl Yönetiliyor?**
    1.  Tauri command `StudentAnswerOcrService::start`'ı çağırır.
    2.  Servis, `job_manager.start_job(...)` ile yeni bir iş kaydeder ve durumunu `Queued` yapar.
    3.  `tauri::async_runtime::spawn` ile asıl iş (model istekleri) arka plana itilir.
    4.  Command hemen Frontend'e `JobId`'yi döner. Frontend bu `JobId`'yi ekranda gösterir.
    5.  Arka plandaki `spawn` edilen task, her model dönüşünde `job_manager.update_progress` çağırarak `last_message` ve `current` sayacını günceller.
    6.  UI, Query polling veya Tauri dinleyicileri (listeners) sayesinde ilerlemeyi okur (Frontend: `StudentAnswerOcrPage.tsx`).

---

## E. Trait / Gateway Mantığı

Neden UI doğrudan llama-server'a istek atmıyor?
Neden OCR servisi doğrudan process'i çalıştırmıyor?

*   **Cevap:** Bağımlılıkların Tersine Çevrilmesi (Dependency Inversion).
*   **`ModelGateway` (Trait):** Modelin "nasıl" çalıştırıldığını (HTTP mi, native mi) gizleyen bir arayüzdür (`src-tauri/src/services/model_gateway.rs`).
*   **`LlamaServerGateway` (Impl):** Bu arayüzü uygulayan (`impl ModelGateway for LlamaServerGateway`), llama.cpp'nin JSON API'sine HTTP `reqwest` ile bağlanan kısımdır.
*   **Neden Merkezi ModelRuntimeService var?**
    Model sunucusu (llama-server) ağır bir süreçtir. Port çakışmalarını önlemek, çökerse yeniden başlatmak, hazır olmadan istek atmayı engellemek için merkezi bir süreç yöneticisine (`ModelProcessManager` & `ModelRuntimeService`) ihtiyaç vardır. Böylece OCR servisi portun dolu olup olmadığını düşünmek zorunda kalmaz.

---

## F. Borrowing / Ownership Nerede Önemli?

Rust'ın en zor ama en güvenli kısmı: Sahiplik (Ownership).

*   **`clone` Nerede Kullanılmış?**
    `StudentAnswerOcrService` içinde, arka planda bir thread'e geçerken (`tauri::async_runtime::spawn(async move { ... })`) servisin kendisi (`service.clone()`) ve proje ID'si (`project_id_for_run = project_id.clone()`) kopyalanır. Thread'in yaşam süresi belirsiz olduğu için referans (borrow) verilemez, değişkenin sahipliği veya klonu verilmelidir.
*   **Mutable (`&mut`) Proje Güncelleme:**
    `workflow_engine::evaluate_workflow` çağrıldıktan sonra dönen yeni workflow'u projeye atamak için:
    ```rust
    let mut project = self.project_store.get_project_snapshot(...)?;
    project.workflow = workflow_engine::evaluate_workflow(&project);
    self.project_store.save_project(&project)?;
    ```
    Burada `get_project_snapshot` projeyi (owned) döner. Onu `mut` yaparız ki değiştirebilelim.
*   **İncelenecek Satırlar:**
    *   `src-tauri/src/services/student_answer_ocr_service.rs` (Satır ~120-140): `spawn` öncesi kopyalama.
    *   `src-tauri/src/domain/workflow.rs` (Satır ~12: `evaluate_workflow` fonksiyonu): Referans alan (`&Project`) ve veriyi hiç değiştirmeden saf snapshot dönen güzel bir örnek.
