# Hızlı geliştirme ve test döngüsü

Bu proje Rust/Tauri backend'i ile Node tabanlı frontend testlerini ayrı
çalıştırır. Günlük geliştirmede önce dar kontrolü çalıştırın:

```bash
npm run check:fast
```

Bu komut frontend typecheck ile `cargo check --workspace`
çalıştırır; test binary'lerini koşturmaz.

## Rust testleri

`npm run cargo:test` nextest kuruluysa `cargo nextest run` kullanır. Nextest
kurulu değilse komut bunu açıkça bildirerek `cargo test` ile devam eder:

```bash
cargo install cargo-nextest --locked
npm run cargo:test
```

Belirli bir çalıştırıcıyı zorlamak için:

```bash
RUBRIKA_TEST_RUNNER=nextest npm run cargo:test
RUBRIKA_TEST_RUNNER=legacy npm run cargo:test
```

Nextest testleri varsayılan olarak izole süreçlerde paralel çalıştırır. Paylaşılan
kaynağa bağlı bir test grubu varsa o grup için uygun nextest profili veya test
izolasyonu ayrıca tanımlanmalıdır; bu ayar testlerin sonucunu gizlemek için
kullanılmamalıdır.

## macOS linker

`cargo:check`, `cargo:clippy` ve bunların derleme yapan alt komutları
`scripts/run-cargo.mjs` üzerinden çalışır. Apple Silicon macOS'ta PATH içinde
`ld64.lld`, `lld`, `ld.lld` veya `llvm-lld` bulunursa `-fuse-ld` otomatik eklenir.
Bulunamazsa varsayılan Apple linker kullanılır; bu durum başarısız bir derleme
olarak gizlenmez.

Hızlı linker'ı zorlamak için:

```bash
RUBRIKA_FAST_LINKER=lld npm run cargo:check
```

Bu komut linker bulunamazsa başarısız olur. Kalıcı kullanıcı ayarı tercih
edilirse, linker kurulumundan sonra `~/.cargo/config.toml` içine aşağıdaki
hedef ayarı eklenebilir:

```toml
[target.aarch64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

Bu ayar yalnızca gerçekten kurulu olan bir `lld` ile kullanılmalıdır. Projenin
çalışma ağacına zorunlu linker ayarı eklenmemesinin nedeni, kurulu olmayan bir
linker'ın tüm geliştirici derlemelerini çalışmaz hale getirmesidir.

## Güvenlik taraması

Antivirüs veya EDR kullanılıyorsa, kurum politikasına uygun biçimde yalnızca
`src-tauri/target/` derleme çıktısı dizininin tarama davranışını gözden geçirin.
Bu istisna uygulama tarafından otomatik oluşturulmaz; güvenlik politikasını
sessizce değiştirmemek için makine yöneticisi tarafından yapılmalıdır.
