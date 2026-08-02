# Final Pre-Use Data Loss & Destructive Operations Audit

Tarih: 2026-08-02
Kapsam: `/Users/kadir/Desktop/RubriKa/RubrikaV3`
Gerçek proje: `/Users/kadir/Documents/RubrikaV3/Projects/11_46`
Karar: `DO_NOT_OPEN_FOR_WRITING`

Bu rapor gerçek projeye yazma yetkisi vermez. Gerçek proje üzerinde yalnızca
salt-okunur preflight ve hash envanteri çalıştırılmıştır; migration, repair,
import, delete, GC, restore veya backup çalıştırılmamıştır.

## Sonuç özeti

Kod, frontend, Rust, smoke ve release paketleme kapıları yeşildir. Buna rağmen
gerçek projenin mevcut durumu yazma için güvenli değildir:

- doğrulanmış bağımsız backup yok;
- bir adet sınıflandırılamayan orphan speaking audio var;
- audit hash zinciri geçersiz ve audit/project revision divergence var;
- bu nedenle preflight kararı `DO_NOT_OPEN_FOR_WRITING` ve
  `safeToOpenForWriting=false` döner.

Bu karar, kalite kapılarının yeşil olmasından bağımsızdır; gerçek proje
bulguları çözülmeden yazma açılmamalıdır.

## Gerçek proje salt-okunur preflight

Komut:

```text
src-tauri/target/debug/rubrika --json preflight \
  /Users/kadir/Documents/RubrikaV3/Projects/11_46
```

Komutun `DO_NOT_OPEN_FOR_WRITING` nedeniyle exit code 3 döndürmesi beklenen
davranıştır. Raporlanan değerler:

| Alan | Sonuç |
| --- | --- |
| `readOnly` / garanti | `true` / `true` |
| `project.json` parse | başarılı |
| project id | `58721783-b149-4dfb-902c-714410a38b41` |
| storage revision | `2` |
| regular file / byte | `393` / `169,500,463` |
| symlink | `0` |
| missing active pointer | `0` |
| missing referenced artifact | `0` |
| orphan artifact | `1` |
| unknown orphan | `1` |
| speaking audio without metadata | `16` |
| incomplete / uncertain transaction | `0` / `0` |
| second writer detected | `false` |
| verified backup | `0` |
| pending migration | `false` |

Unknown orphan:

```text
artifacts/speaking-exams/b77116c4-b91f-4bb2-966f-0fa12d2a0940/audio-original.wav
```

Dosya 574,784 byte’tır. Canonical speaking attempt pointer’ı bulunamadığı
için sınıflandırma `UNKNOWN`, olası öğretmen içeriği nedeniyle öneri
`KEEP_UNTIL_MANUAL_REVIEW`’dır. Bu dosya silinmemiştir.

Audit bulgusu: 4 kaydın hash eşleşmesi bozuk ve zincir geçersizdir; ayrıca
project revision 2 için eşleşen audit revision kaydı bulunmadığından
`auditProjectDivergenceCount=1` raporlanır. Bu durum mevcut proje state’inin
önce bağımsız backup ve manuel audit incelemesi gerektirdiğini gösterir.

## Salt-okunur değişmezlik kanıtı

Gerçek proje için başlangıç, final-before ve final-after-marker manifestleri
aynı 393 regular file listesini ve aynı SHA-256 manifest özetini verdi:

```text
98448b4b3e36ee34445c60594ef636f58ffdd7af412e02b582afddd8f8b15e6b
```

`cmp` sonucu `0`’dır. Preflight çağrıları ve marker oluşturulması arasında
gerçek proje byte’ı değişmemiştir. Manifestler `/tmp` altında tutulmuştur;
gerçek proje üzerinde hiçbir yazma-capable komut çalıştırılmamıştır.

## Uygulanan veri kaybı kontrolleri

- `ProjectOpenMode` ile inspect/read-only, migration’sız normal open ve
  doğrulanmış backup gerektiren explicit migration ayrıştırıldı.
- `ProjectStore` canonical `project.json` için tek yazardır; atomic write,
  file/directory durability, revision/fingerprint ve write lease kullanır.
- Transaction journal intent/terminal state ile incomplete transaction’lar
  preflight’ta görünür hale getirildi.
- Audit append hash-chain, revision ve divergence doğrulamasıyla typed sonuç
  üretir; sahte başarı yerine hata görünürdür.
- Verified backup/restore manifest, hash, staging ve atomic activation ile
  korunur; source proje backup sırasında değiştirilmez.
- Document import staging + hash doğrulama + trusted rename sırasını,
  deletion dependency scan’i, metadata-first generation GC ve pointer
  recheck’i kullanır.
- Speaking audio/transcript, scoring rerun ve teacher override yollarında
  başarısız iş normal başarı veya sıfır puan olarak kaydedilmez.
- Frontend yazma eylemleri preflight safety banner/guard ve başarısız save’de
  draft koruması ile bağlandı.
- `rubrika preflight` repair/migration yapmadan karar, blocker, warning ve
  read-only garanti bilgisini raporlar.

## Proof 32–58 durumu

Aşağıdaki exact-name proof sembollerinin tamamı test suite içinde bulundu ve
yeşil geçti:

```text
proof_32_reference_project_is_never_modified_by_audit
proof_33_failed_atomic_write_preserves_previous_project
proof_34_process_kill_cannot_leave_partial_canonical_json
proof_35_stale_job_cannot_overwrite_teacher_change
proof_36_failed_replacement_preserves_old_document
proof_37_delete_dependency_scan_prevents_history_loss
proof_38_scoring_rerun_preserves_teacher_override
proof_39_speaking_crash_preserves_teacher_and_audio_state
proof_40_backup_restore_is_semantically_and_byte_equivalent
proof_41_restore_crash_never_activates_partial_project
proof_42_gc_rechecks_references_before_delete
proof_43_disk_full_never_reports_success
proof_44_second_process_cannot_run_destructive_operation
proof_45_frontend_failed_save_preserves_teacher_draft
proof_46_preflight_detects_missing_referenced_artifact
proof_47_audit_and_project_revision_cannot_silently_diverge
proof_48_final_data_loss_negative_repository_scan
proof_49_read_only_project_open_changes_zero_bytes
proof_50_migration_requires_verified_backup
proof_51_parent_sync_failure_never_reports_saved
proof_52_import_kill_preserves_old_active_document
proof_53_speaking_finalize_kill_never_creates_fake_completed
proof_54_delete_rechecks_dependencies_inside_transaction
proof_55_gc_service_cannot_bypass_reference_recheck
proof_56_verified_backup_creation_changes_zero_source_bytes
proof_57_unknown_orphan_blocks_safe_to_open
proof_58_incomplete_audit_transaction_blocks_safe_to_open
```

Bu isimlerin bir bölümü gerçek process-kill/disk-full/race çalıştırmak yerine
regresyon veya contract proxy’sidir. Bu nedenle exact-name suite’in yeşil
olması gerçek proje için yazma izni anlamına gelmez; gerçek proje preflight
blocker’ları ayrıca geçerlidir.

## Kalite ve release doğrulaması

- `npm run check:all`: PASS
- `npm run typecheck`: PASS
- `npm run lint`: PASS
- `npm test`: 131/131 PASS
- `cargo fmt --check`: PASS
- `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`: PASS
- `cargo test`: 398 PASS, 0 FAIL, 4 ignored
- `npm run tauri:dev -- --smoke`: PASS
- `npm run tauri:build`: PASS

Üretilen release çıktıları:

- `src-tauri/target/release/bundle/macos/RubrikaV3.app`
- `src-tauri/target/release/bundle/dmg/RubrikaV3_0.1.0_aarch64.dmg`
- DMG SHA-256: `9a4b96dabe0636c2e23d136d73aaa5de7ab6c85557b42ad51266eb02fcd6ea6b`

Bu sonuçlar imza/notarization veya gerçek proje üzerinde destructive işlem
kabulü anlamına gelmez.

## Kalan gerçek riskler

- Gerçek projede doğrulanmış bağımsız backup yoktur; bu turda scope gereği
  backup alınmamıştır.
- Unknown orphan ve invalid audit chain çözülmeden veri silme/repair yapılması
  geçmişi geri döndürülemez biçimde etkileyebilir.
- Exact-name proof’ların proxy olanları canlı process kill, disk-full veya
  gerçek iki-process race’in yerine geçmez.
- Manuel canlı UI kabulü ve imza/notarization bu denetimin parçası değildir.

## Final karar

## 11_46 integrity recovery closure (2026-08-02)

This section supersedes the earlier no-verified-backup and proxy-proof status
for the controlled recovery artifacts. It does not authorize opening the real
source project for writing.

- Source: /Users/kadir/Documents/RubrikaV3/Projects/11_46
- Initial source byte manifest: 393 regular files, 169,500,463 bytes,
  project.json SHA-256 afd35dfcbbef0a2d944e004b741057fe1cd99ec36fb3e32f36eaacd5723334cf,
  logs/audit.jsonl SHA-256 2db6dfe395797546057a25346ba6dbf52f81c4acc7354c7a2a95443e7441579a,
  storage revision 2, project fingerprint afd35dfcbbef0a2d944e004b741057fe1cd99ec36fb3e32f36eaacd5723334cf.
- Backup:
  /Users/kadir/Documents/RubrikaV3/VerifiedBackups/11_46-pre-recovery-20260802T164052.054311Z.rbackup
  SHA-256 3450757a07f49a05314cff0e9d00e1e8839befe68eb6c88e4d1ec4c865172896;
  393 entries and 169,500,463 bytes.
- Independent restore equality: PASS. Archive traversal/checksum validation,
  restored project parse/open, byte-bearing manifest, domain equality and
  artifact hash equality all pass. The byte-bearing manifest is
  2260362b9be3cf37210fd8674aeecac3b6478d6434427006e95ced6b11e64e35.
- Source manifest before/after backup has identical byte entries (0 added,
  0 removed, 0 changed). The full metadata manifest hash may differ because
  mtime is observational; the byte manifest is the source safety criterion.
- The orphan
  artifacts/speaking-exams/b77116c4-b91f-4bb2-966f-0fa12d2a0940/audio-original.wav
  is a valid mono 16 kHz WAV, 17.960625 seconds, 574,784 bytes,
  SHA-256 0cbfcee9a2d5a1fc7e71445b55676be353ebe6357e61e2802a2e56a7c16379b0.
  It has no verified canonical attempt/student relationship and remains
  UNKNOWN on the source. It was not changed. The recovery candidate keeps
  the same bytes under lost+found/audio/ with a quarantine record.
- The source audit first diverges at line 1, record
  b916cfc3-5c3f-45f6-a930-9194769e3a48: computed hash
  8e902f89c357f6fea8fcab0b79db156c0912c4d5fdc7ad2807f6462cc5946cec,
  recorded hash
  a1d2ccd825cc0742d7d84d647259caecef0f174583310aff518703c91463ef67.
  The legacy audit has four hash mismatches and no revision evidence for
  project revision 2. It was not rewritten.
- Repaired candidate:
  /tmp/RubrikaV3-11_46-recovery/repaired-candidate.
  Historical audit is preserved byte-for-byte and anchored by a new
  RecoveryAnchor; active chain is VALID, historical anchor is VALID,
  active revision divergence is 0, incomplete/ambiguous transactions are
  0, and unknown orphans are 0.
- Real child-process, filesystem-fault and two-process lease fixtures now
  pass. Their proof markers are generated only after the fixture assertions
  pass; stale empty markers are not accepted by preflight.

The source preflight remains DO_NOT_OPEN_FOR_WRITING because the source still
contains the unknown orphan and unrecovered historical audit chain. The
repaired candidate preflight is SAFE_TO_OPEN for the isolated copy only.
See docs/11_46_INTEGRITY_RECOVERY_REPORT.md for the complete evidence and
final test matrix.

Gerçek proje için güvenli eylem yalnızca salt-okunur inceleme, bağımsız
backup hazırlığı ve manuel audit/orphan değerlendirmesidir. Proje yazma,
migration, import, delete, GC, restore veya repair için açılmamalıdır.

`DO_NOT_OPEN_FOR_WRITING`

## Superseding integrity-recovery result (2026-08-02 final)

The verified backup now exists at
`/Users/kadir/Documents/RubrikaV3/VerifiedBackups/11_46-pre-recovery-20260802T164052.054311Z.rbackup`
with SHA-256
`3450757a07f49a05314cff0e9d00e1e8839befe68eb6c88e4d1ec4c865172896`.
Restore equality is PASS, the source byte manifest is unchanged, and the
source final external manifest matches the initial byte-bearing entries.

The source still has one `UNKNOWN` orphan audio and an
`INVALID_UNRECOVERED` audit chain with active revision divergence 1. The
repaired candidate has a valid active chain and valid historical anchor, but
its decision is `RECOVERED_COPY_NOT_SAFE` until the full Cargo/check:all
validation is green. The full suite currently reports 394 passed, 6 failed,
4 ignored because six model-runtime fixtures cannot bind loopback in this
environment; `npm run check:all` therefore exits 101. No full-validation PASS
marker was created.

The source decision remains `DO_NOT_OPEN_FOR_WRITING`.
