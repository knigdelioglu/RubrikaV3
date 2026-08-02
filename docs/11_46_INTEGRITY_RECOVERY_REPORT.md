# 11_46 Integrity Recovery Report

Date: 2026-08-02  
Source: /Users/kadir/Documents/RubrikaV3/Projects/11_46  
Source policy: read-only; no recovery, migration, repair, cleanup, quarantine,
rename, delete, import, restore, lock metadata or project save was run against
the source.

## 1. Kaynak başlangıç manifesti

The external manifest is
/tmp/RubrikaV3-11_46-recovery/source-before.manifest.json.

- 393 regular files, 139 directories, 0 symlinks
- total regular bytes: 169,500,463
- project.json SHA-256:
  afd35dfcbbef0a2d944e004b741057fe1cd99ec36fb3e32f36eaacd5723334cf
- logs/audit.jsonl SHA-256:
  2db6dfe395797546057a25346ba6dbf52f81c4acc7354c7a2a95443e7441579a
- storageRevision/project revision: 2
- project fingerprint:
  afd35dfcbbef0a2d944e004b741057fe1cd99ec36fb3e32f36eaacd5723334cf
- full manifest hash at the initial observation:
  40a5d6a417383022122a3f73811a18fe5550d79c7e75954ec870b5f18d369077

## 2. Kaynağın sıfır-write doğrulaması

The source was checked for RubrikaV3, CLI and related writer processes before
backup. No active RubrikaV3 writer was present. The source before/after backup
byte entries were identical: 0 added, 0 removed, 0 changed; file count and
total bytes remained 393 and 169,500,463. The final external manifest exactly
matches the initial full manifest hash
40a5d6a417383022122a3f73811a18fe5550d79c7e75954ec870b5f18d369077. The
intermediate backup observation used a different directory-metadata encoding,
but its byte-bearing entries were identical. No source lock, temp, audit or
backup metadata file was created by this task.

## 3. Verified backup

Archive:
/Users/kadir/Documents/RubrikaV3/VerifiedBackups/11_46-pre-recovery-20260802T164052.054311Z.rbackup

- archive SHA-256:
  3450757a07f49a05314cff0e9d00e1e8839befe68eb6c88e4d1ec4c865172896
- manifest receipt:
  /Users/kadir/Documents/RubrikaV3/VerifiedBackups/11_46-pre-recovery-20260802T164052.054311Z.manifest.json
- verification receipt:
  /Users/kadir/Documents/RubrikaV3/VerifiedBackups/11_46-pre-recovery-20260802T164052.054311Z.sha256.json
- 393 entries, 169,500,463 bytes
- source manifest hash in archive:
  2e6f1c5fc0535179fbc867672c3ac52eacab808e9d270d939282755e6fe71042

## 4. Backup restore equality

Independent copies were created from the verified archive:

- source-restore:
  /tmp/RubrikaV3-11_46-recovery/source-restore
- forensic-copy:
  /tmp/RubrikaV3-11_46-recovery/forensic-copy
- repaired-candidate:
  /tmp/RubrikaV3-11_46-recovery/repaired-candidate

verify-restore PASS: archive verification, traversal/duplicate/absolute-path/
symlink checks, restored project parse/open, domain equality, artifact hash
equality and byte-bearing manifest equality. Proof receipt:
/tmp/RubrikaV3-11_46-recovery/backup-restore-equality.pass.

The equal byte-bearing manifest is
2260362b9be3cf37210fd8674aeecac3b6478d6434427006e95ced6b11e64e35.

## 5. Orphan audio forensic sonucu

The source orphan is:
artifacts/speaking-exams/b77116c4-b91f-4bb2-966f-0fa12d2a0940/audio-original.wav

- filename: audio-original.wav
- byte size: 574,784
- SHA-256: 0cbfcee9a2d5a1fc7e71445b55676be353ebe6357e61e2802a2e56a7c16379b0
- WAV validity: valid
- duration: 17.960625 seconds
- sample rate/channels: 16,000 Hz / mono
- mtime ns: 1785309403627538457
- probable speaking attempt/student/class: not established
- metadata/transcript/job/audit references: indirect text matches exist, but no
  verified canonical attempt relation
- project.json reference: present as an indirect textual reference
- backup manifest reference: false in the source artifact; the independent
  backup receipt references the project snapshot externally
- duplicate hash: none
- canonical naming: filename and UUID parent are syntactically canonical
- classification: UNKNOWN
- action: KEEP_UNTIL_MANUAL_REVIEW

The source file was not changed. In the repaired candidate only, its bytes and
hash are preserved under lost+found/audio with original relative path and a
quarantine audit event.

## 6. Audit zinciri ilk divergence

Source audit: 4 records, SHA-256
2db6dfe395797546057a25346ba6dbf52f81c4acc7354c7a2a95443e7441579a, size
1,839 bytes. First invalid line is 1:

- record ID: b916cfc3-5c3f-45f6-a930-9194769e3a48
- previous hash: genesis
- computed hash:
  8e902f89c357f6fea8fcab0b79db156c0912c4d5fdc7ad2807f6462cc5946cec
- recorded hash:
  a1d2ccd825cc0742d7d84d647259caecef0f174583310aff518703c91463ef67
- timestamp: 2026-07-31T20:13:53.438196+00:00
- operation: project_opened
- correlation ID: 2b14dd76-4eda-4c7b-ab61-fe52451eef42
- revisions/transaction ID: absent

All four records have legacy-format/hash-mismatch evidence. The source audit
was never rewritten or rehashed.

## 7. Audit/project revision divergence

Observed project revision is 2. The source audit has no valid revision record,
so the divergence count is 1 and the active audit status is
INVALID_UNRECOVERED. The repaired candidate carries the observed project
revision in its RecoveryAnchor and reports active revision divergence 0.

## 8. Transaction journal sonucu

Source and repaired candidate: incomplete transaction count 0 and ambiguous
transaction count 0. No historical transaction completion was invented.

## 9. Recovery anchor

The repaired candidate retains the original audit at:
/tmp/RubrikaV3-11_46-recovery/repaired-candidate/logs/recovery/historical/audit.jsonl

- original audit SHA-256:
  2db6dfe395797546057a25346ba6dbf52f81c4acc7354c7a2a95443e7441579a
- original audit size: 1,839
- first invalid line: 1
- project revision: 2
- project fingerprint:
  afd35dfcbbef0a2d944e004b741057fe1cd99ec36fb3e32f36eaacd5723334cf
- source backup SHA-256:
  3450757a07f49a05314cff0e9d00e1e8839befe68eb6c88e4d1ec4c865172896
- recovery manifest SHA-256:
  72440a73e2b7397b544fc90b0e379154c1f6663c89b1c916a7d06258f1236694
- recovery timestamp: 2026-08-02T16:50:00Z range
- active chain: VALID
- historical anchor: VALID

The old chain is historical evidence; the new chain is the only active append
target after recovery.

## 10. Repaired candidate

Path:
/tmp/RubrikaV3-11_46-recovery/repaired-candidate

Candidate preflight: project parses, missing referenced artifacts 0, unknown
orphans 0, incomplete/ambiguous transactions 0, active audit status VALID,
historical recovery anchor VALID, active revision divergence 0, verified restore
PASS, and all three destructive proof statuses PASS. The remaining speaking
audio-without-metadata count is reported as a non-orphan diagnostic count; no
student or attempt relationship was inferred. The candidate decision remains
DO_NOT_OPEN_FOR_WRITING because the full validation marker is NOT_VERIFIED.

## 11. Kaynak/repaired diff

Domain equality: PASS. Artifact hash equality: PASS. Byte identity is
intentionally false because the candidate has an active recovery audit,
historical-audit copy, recovery manifest and preserved orphan quarantine.
Source byte manifest: 2260362b9be3cf37210fd8674aeecac3b6478d6434427006e95ced6b11e64e35.
Candidate byte manifest:
1bf53e37eedf29780fcf0cbce2ddf30db7a4ea46a1425a091f5bb2f3a933d25f. The
original orphan hash is unchanged. Empty directory differences are explicitly
explained as restore-format metadata differences, not byte-bearing artifact
loss. There are no unexplained changes.

## 12. Process-kill proof’ları

final_data_loss_proofs target: 11 passed, 0 failed, 1 ignored child fixture.
The real child fixture kills processes with SIGKILL after canonical staging,
import staging, speaking audio staging and restore staging boundaries. The
existing production atomic-write unit child proof also passes.

## 13. Disk/filesystem fault proof’ları

disk_fault filter: 1 passed, 0 failed, 0 ignored. The matrix covers real
read-only directory rejection, rename/temp conflict, short-write WriteZero
behavior and durability-uncertain classification. The proof marker is
accepted only when it contains status=PASS.

## 14. İki-process destructive race proof’ları

destructive_race filter: 1 passed, 0 failed, 0 ignored. Process A holds the
OS lease while process B attempts backup and restore; B is rejected, no
partial destination is activated, and the destination remains without
project.json. The proof marker is status=PASS.

## 15. DataLossPreflightReport

Source report decision: DO_NOT_OPEN_FOR_WRITING.

- sourceByteChanges: 0
- verified backup restore status: PASS
- unknown orphan count: 1
- active revision divergence: 1
- original/active audit status: INVALID_UNRECOVERED
- historical anchor status: NOT_PRESENT
- incomplete/ambiguous transaction count: 0/0
- process/disk/race proof status: PASS/PASS/PASS

Repaired candidate report: `DO_NOT_OPEN_FOR_WRITING`; its only remaining
blocker is `full validation marker yok`. This is intentionally not promoted to
SAFE_TO_OPEN because the required full Cargo suite and `check:all` did not
pass.

## 16. Frontend safety guard’ları

AppLayout blocks project-page button actions while preflight is loading,
unavailable or DO_NOT_OPEN_FOR_WRITING. LoadingButton marks project writes
explicitly; backup is explicitly read-only; repair/recovery is exposed as a
job contract and cannot target the source path. The banner shows friendly
backup, orphan, audit, revision and proof messages without raw technical
codes.

## 17. Değiştirilen dosyalar

Core changes include:

- src-tauri/src/services/integrity_recovery_service.rs
- src-tauri/src/services/backup_service.rs
- src-tauri/src/services/audit_service.rs
- src-tauri/src/services/transaction_journal.rs
- src-tauri/src/diagnostics.rs
- src-tauri/src/platform/project_write_lease.rs
- src-tauri/src/commands/backup_commands.rs
- src-tauri/src/bin/rubrika.rs
- src-tauri/src/domain/job.rs
- src-tauri/tests/final_data_loss_proofs.rs
- src/app/AppLayout.tsx
- src/app/globalJobs.ts
- src/api/commands.ts
- src/api/types.ts
- requested audit/map/contract/flow/ownership documents

## 18. Frontend testleri

- `npm run typecheck`: EXIT 0.
- `npm run lint`: EXIT 0.
- `npm test`: EXIT 0; 132 passed, 0 failed, 0 skipped, duration 0.335 s.
- `npm run build`: EXIT 0; Vite production build passed.

## 19. Rust testleri

- `final_data_loss_proofs`: EXIT 0; 11 passed, 0 failed, 1 ignored, 0
  filtered, 9.18 s.
- `process_kill`: EXIT 0; real child-process fixture passed; 1 child entry
  intentionally ignored and 403 lib tests filtered in the lib target.
- `disk_fault`: EXIT 0; 1 passed, 0 failed, 0 ignored, 11 filtered in the
  integration target.
- `destructive_race`: EXIT 0; 1 passed, 0 failed, 0 ignored, 11 filtered in
  the integration target.
- `cargo fmt --check`: EXIT 0.
- `cargo check --all-targets`: EXIT 0.
- `cargo clippy --all-targets --all-features -- -D warnings`: EXIT 0.
- `cargo test`: EXIT 101; 394 passed, 6 failed, 4 ignored, 0 filtered,
  21.49 s. The six failures are loopback-listener permission failures in
  existing model-runtime fixtures.
- A separate escalated retry of one loopback fixture exceeded 60 seconds and
  was interrupted; it was not counted as a pass.

## 20. Ignored testler

Normal targeted proof execution has one intentional child entry point ignored:
`process_kill_real_child_fixture_child`; the parent invokes it with the fixture
environment. The explicit `cargo test -- --ignored --nocapture` command exited
101: 4 model-runtime ignored tests passed, then the standalone child entry
point failed because its required fixture environment was absent. No ignored
test is being counted as a successful full-suite substitute.

## 21. check:all

`npm run check:all`: EXIT 101. Build, typecheck, lint, frontend tests, fmt and
clippy completed successfully; the final `cargo test` step reproduced the six
loopback-listener model-runtime failures above. Therefore no full-validation
PASS marker was created.

## 22. Tauri smoke

`npm run tauri:dev -- --smoke`: EXIT 0. Vite started at
`http://127.0.0.1:5173/`, `target/debug/app` started and the smoke process
exited successfully. No smoke process remained afterward.

## 23. .app ve DMG

`npm run tauri:build`: EXIT 0; both bundles were produced.

- `.app`: `src-tauri/target/release/bundle/macos/RubrikaV3.app` (~45 MiB).
- DMG: `src-tauri/target/release/bundle/dmg/RubrikaV3_0.1.0_aarch64.dmg`,
  19,213,500 bytes, SHA-256
  `5ecf016dce5c6e2b3f7a820c84a18e42c541c62775e0d68d88c698345f49b407`.
- App executable SHA-256:
  `79337f5101a7c7f104d87c41195ae7bf0f28a1b8ba04fca87df315eaba976e13`.

## 24. Kaynak final manifesti

The final source manifest was generated after the final source preflight and
is stored outside the source at
/tmp/RubrikaV3-11_46-recovery/source-final.manifest.json. Its full metadata
manifest hash is
40a5d6a417383022122a3f73811a18fe5550d79c7e75954ec870b5f18d369077. Summary:
393 regular files, 139 directories, 0 symlinks, 169,500,463 bytes; project and
audit hashes are unchanged.

## 25. Kaynak byte değişikliği

Final comparison: 0 added, 0 removed and 0 changed byte entries; 393 byte
entries; project.json SHA-256 unchanged at
afd35dfcbbef0a2d944e004b741057fe1cd99ec36fb3e32f36eaacd5723334cf;
logs/audit.jsonl SHA-256 unchanged at
2db6dfe395797546057a25346ba6dbf52f81c4acc7354c7a2a95443e7441579a; symlinks
unchanged at 0; no new source lock/temp/audit/backup metadata.

## 26. Repaired candidate kararı

RECOVERED_COPY_NOT_SAFE. The isolated candidate has PASS for backup restore,
domain/artifact equality, active audit, anchor, revision divergence,
transactions, orphan handling and destructive proofs, but the required full
Cargo/check:all validation is not green. This is not a decision for the real
source project.

## 27. Kalan gerçek riskler

The source unknown orphan and historical invalid audit remain unresolved on
purpose. The full Rust suite/check:all are blocked by six loopback-listener
model-runtime fixture failures in this environment; a separate escalated
single-fixture retry also exceeded 60 seconds and was not counted as pass. The
standalone ignored child fixture is not valid without its parent environment.
The orphan relationship must be manually established or rejected before any
source cleanup. The historical chain cannot be treated as verified history.

## 28. Gerçek projeye uygulanacak recovery planı

1. Keep the verified backup immutable and retain its receipts.
2. Review the orphan report and approve or reject its speaking relationship.
3. Re-run source preflight and compare the source final manifest.
4. Obtain explicit user approval for a separate apply task.
5. In that task, run recover-copy dry-run first, then apply only to a new
   destination; never mutate the original source in place.
6. Reopen the recovered destination read-only, verify active audit/revision,
   transaction journal, speaking metadata and full preflight.
7. Only then choose whether to migrate or replace the original project.

## 29. Uygulama kullanım kararı

Uygulama kullanım kararı: DO_NOT_OPEN_FOR_WRITING
