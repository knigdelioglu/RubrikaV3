# Proje Kilidi ve Taşınabilir Asset Erişimi

## App single-instance

`AppInstanceLease`, per-user app-support dizininde OS `flock` tutar. İkinci
instance writer başlatmaz ve temiz çıkar; ilk instance çökerse OS kilidi
otomatik serbest kalır. Resmî Tauri single-instance plugin'i bu ortamın
offline registry'sinde çözülemediği için belgelenmiş güvenli alternatif
kullanılmıştır (pencere öne getirme davranışı yoktur; kalan sınırlama).

## ProjectWriteLease

- Gerçek OS file lock: `flock(LOCK_EX | LOCK_NB)` proje kökündeki
  `.rubrika.lock` dosyasında.
- Aynı process içindeki birden fazla store aynı lease'i paylaşır
  (`acquire_or_share`); ikinci OS process `ProjectAlreadyOpen` alır.
- `ProjectStore` yazar yolları (`open_project`, `create_project`,
  `mutate`) lease olmadan açılmaz.
- PID dosyasına güven yok; crash'te OS release eder.

Kanıt: `second_process_cannot_write_locked_project` gerçek child process
fixture'ı ile A→B red, A çıkışı→C başarı.

## Taşınabilir asset serving

`managed-asset://localhost/<project_id>/<relative-path>` custom Tauri
protokolü:
- frontend mutlak path gönderemez; DTO'lar göreli managed path döner
- `ManagedProjectPath` traversal/backslash/drive/NUL reddeder
- `TrustedProjectRoot::resolve_existing_file` symlink escape reddeder
- okuma `MAX_MANAGED_ASSET_BYTES` (32 MiB) ile sınırlıdır; MIME uzantıdan
  belirlenir
- asset scope genişletilmemiştir; taşınan proje canonical root üzerinden
  açılır (proof_25)
