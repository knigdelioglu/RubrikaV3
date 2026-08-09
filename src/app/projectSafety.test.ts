/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import type { DataLossPreflightReport } from '../api/types.project.ts';
import {
  isProjectWriteBlocked,
  resolveWriteBlockReason,
  preflightReasonLabel,
} from './projectSafety.ts';

function makeReport(
  overrides: Partial<DataLossPreflightReport> = {},
): DataLossPreflightReport {
  return {
    projectPath: '/tmp/project-x',
    readOnly: true,
    readOnlyGuaranteeVerified: true,
    projectFileExists: true,
    projectParseOk: true,
    projectId: 'project-x',
    storageRevision: 3,
    projectRevision: 3,
    projectFingerprint: 'fp',
    sourceManifestHash: 'hash',
    sourceByteChanges: 0,
    pendingMigration: false,
    migrationBackupStatus: 'not_required',
    recursiveFileCount: 10,
    recursiveByteCount: 100,
    recursiveInventorySha256: 'inv',
    symlinkCount: 0,
    symlinkPaths: [],
    missingActivePointerCount: 0,
    missingReferencedArtifactCount: 0,
    brokenActivePointerCount: 0,
    orphanArtifactCount: 0,
    unknownOrphanCount: 0,
    orphanArtifacts: [],
    orphanRestoreStagingCount: 0,
    unsafeRestoreStagingCount: 0,
    unsafeImportStagingCount: 0,
    speakingAudioWithoutMetadataCount: 0,
    speakingMetadataWithoutAudioCount: 0,
    recoverableAudioOrphanCount: 0,
    staleGcPlanCount: 0,
    incompleteTransactionCount: 0,
    ambiguousTransactionCount: 0,
    auditProjectDivergenceCount: 0,
    activeRevisionDivergenceCount: 0,
    originalAuditStatus: 'VALID',
    activeAuditStatus: 'VALID',
    historicalRecoveryAnchorStatus: 'PASS',
    durabilityUncertainCount: 0,
    secondWriterDetected: false,
    initializationWriteAllowed: false,
    unverifiedWritesAllowed: true,
    audit: {
      recordCount: 0,
      chainValid: true,
      tamperCount: 0,
      reasons: [],
      projectRevisionDivergenceCount: 0,
      activeRevisionDivergenceCount: 0,
      firstInvalidLine: null,
      firstInvalidRecordId: null,
      firstInvalidPreviousHash: null,
      firstInvalidComputedHash: null,
      firstInvalidRecordedHash: null,
      lastValidRecordHash: 'x',
      lastAuditRevision: 0,
      duplicateRevisionCount: 0,
      missingRevisionCount: 0,
      originalAuditStatus: 'VALID',
      activeAuditStatus: 'VALID',
      historicalRecoveryAnchorStatus: 'PASS',
      classifications: [],
      recentRecords: [],
    },
    verifiedBackupCount: 1,
    failedBackupCount: 0,
    backupPaths: [],
    latestVerifiedBackupPath: null,
    verifiedBackupPath: null,
    verifiedBackupSha256: null,
    verifiedBackupRestoreStatus: 'PASS',
    latestVerifiedBackupAge: null,
    processKillProofsStatus: 'PASS',
    diskFaultProofsStatus: 'PASS',
    destructiveRaceProofsStatus: 'PASS',
    fullTestSuiteGreen: true,
    blockers: [],
    warnings: [],
    errors: [],
    decision: 'SAFE_TO_OPEN',
    safeToOpenForWriting: true,
    ...overrides,
  };
}

test('isProjectWriteBlocked gate matrix matches AppLayout runtime gate', () => {
  const idle = { isLoading: false, isError: false };

  assert.equal(
    isProjectWriteBlocked(undefined, { isLoading: true, isError: false }),
    true,
    'loading sırasında yazma engellenir',
  );
  assert.equal(
    isProjectWriteBlocked(undefined, { isLoading: false, isError: true }),
    true,
    'preflight hatasında yazma engellenir',
  );
  assert.equal(
    isProjectWriteBlocked(undefined, idle),
    true,
    'report yoksa yazma engellenir',
  );

  const doNotOpen = makeReport({ decision: 'DO_NOT_OPEN_FOR_WRITING', safeToOpenForWriting: false });
  assert.equal(
    isProjectWriteBlocked(doNotOpen, idle),
    true,
    'DO_NOT_OPEN_FOR_WRITING + !initializationWriteAllowed → blocked',
  );

  const initAllowed = makeReport({
    decision: 'DO_NOT_OPEN_FOR_WRITING',
    safeToOpenForWriting: false,
    initializationWriteAllowed: true,
  });
  assert.equal(
    isProjectWriteBlocked(initAllowed, idle),
    false,
    'DO_NOT_OPEN_FOR_WRITING + initializationWriteAllowed → yeni proje ilk kuruluma açık',
  );

  const safeWithBackup = makeReport({
    decision: 'SAFE_TO_OPEN_WITH_BACKUP',
    warnings: ['Bağımsız doğrulanmış backup bulunamadı.'],
  });
  assert.equal(
    isProjectWriteBlocked(safeWithBackup, idle),
    false,
    'SAFE_TO_OPEN_WITH_BACKUP → blocked değil',
  );

  const safe = makeReport({ decision: 'SAFE_TO_OPEN' });
  assert.equal(isProjectWriteBlocked(safe, idle), false, 'SAFE_TO_OPEN → blocked değil');
});

test('dev-mode (unverifiedWritesAllowed) + yalnız backup/release eksikliği → writes allowed', () => {
  // Rust tarafı dev modda sadece relaxable blocker'ları süzer (filter_preflight_blockers).
  // Yalnız backup/proof eksikliği varsa decision SAFE kalır → gate false.
  const devOnlyBackupMissing = makeReport({
    unverifiedWritesAllowed: true,
    decision: 'SAFE_TO_OPEN_WITH_BACKUP',
    verifiedBackupCount: 0,
    processKillProofsStatus: 'NOT_VERIFIED',
    fullTestSuiteGreen: false,
    warnings: ['Bağımsız doğrulanmış backup bulunamadı.'],
  });
  assert.equal(
    isProjectWriteBlocked(devOnlyBackupMissing, { isLoading: false, isError: false }),
    false,
    'deneme modunda yalnız backup/release eksikliği yazmayı engellememeli',
  );
  assert.equal(
    resolveWriteBlockReason({
      report: devOnlyBackupMissing,
      state: { isLoading: false, isError: false },
    }),
    null,
  );
});

test('dev-mode (unverifiedWritesAllowed) + GERÇEK integrity blocker → writes blocked', () => {
  const realIntegrityBlock = makeReport({
    unverifiedWritesAllowed: true,
    decision: 'DO_NOT_OPEN_FOR_WRITING',
    safeToOpenForWriting: false,
    blockers: ['ikinci writer aktif'],
    secondWriterDetected: true,
  });
  assert.equal(
    isProjectWriteBlocked(realIntegrityBlock, { isLoading: false, isError: false }),
    true,
    'deneme modu gerçek bütünlük blockerlarını geçmemeli',
  );
  assert.equal(
    resolveWriteBlockReason({
      report: realIntegrityBlock,
      state: { isLoading: false, isError: false },
    }),
    'Proje başka bir yazıcı işlem tarafından kullanılıyor.',
  );
});

test('resolveWriteBlockReason: teacher-safe nedenler', () => {
  assert.match(
    resolveWriteBlockReason({
      report: undefined,
      state: { isLoading: true, isError: false },
    }) ?? '',
    /ön kontrolü henüz tamamlanmadı/,
  );
  assert.match(
    resolveWriteBlockReason({
      report: undefined,
      state: { isLoading: false, isError: true },
    }) ?? '',
    /ön kontrolü alınamadı/,
  );
  assert.match(
    resolveWriteBlockReason({ report: undefined, state: { isLoading: false, isError: false } }) ?? '',
    /ön kontrolü alınamadı/,
  );

  const pendingMigration = makeReport({
    decision: 'DO_NOT_OPEN_FOR_WRITING',
    safeToOpenForWriting: false,
    pendingMigration: true,
    blockers: ['pending migration var'],
  });
  assert.equal(
    resolveWriteBlockReason({
      report: pendingMigration,
      state: { isLoading: false, isError: false },
    }),
    'Proje için açıkça onaylanmış göç gerekiyor.',
  );

  const noBlockerList = makeReport({
    decision: 'DO_NOT_OPEN_FOR_WRITING',
    safeToOpenForWriting: false,
    blockers: [],
  });
  assert.equal(
    resolveWriteBlockReason({
      report: noBlockerList,
      state: { isLoading: false, isError: false },
    }),
    'Proje için veri güvenliği ön koşulu sağlanmadı.',
  );
});

test('preflightReasonLabel tüm backend blocker anahtarlarını kapsar', () => {
  const backendKeys = [
    'verified backup yok',
    'failed/unverified backup var',
    'unknown orphan var',
    'missing referenced artifact var',
    'pending migration var',
    'incomplete transaction var',
    'ambiguous transaction var',
    'audit chain geçersiz',
    'audit/project revision divergence var',
    'active audit/project revision divergence var',
    'active audit chain invalid',
    'verified backup restore doğrulanmadı',
    'process-kill proof failure',
    'disk fault proof failure',
    'destructive race proof failure',
    'source byte manifest changed',
    'speaking metadata/audio mismatch var',
    'read-only hash guarantee doğrulanmadı',
    'full validation marker yok',
    'symlink bulundu',
    'unsafe import staging var',
    'unsafe restore staging var',
    'ikinci writer aktif',
  ];
  for (const key of backendKeys) {
    const label = preflightReasonLabel(key);
    assert.ok(label.length > 0, `anahtar için label üretilmeli: ${key}`);
    assert.notEqual(label, 'Veri güvenliği ön koşulu sağlanmadı.', `bilinmeyen anahtar: ${key}`);
  }
});
