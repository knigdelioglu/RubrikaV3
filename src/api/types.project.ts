import type { AssessmentActivity } from './types.assessment';
import type { Document } from './types.document';
import type { OcrGeneration, StudentAnswerCropTemplate, StudentAnswerOcrRecord, StudentIdentityCropTemplate } from './types.ocr';
import type { Question } from './types.question';
import type { ScoringAnchor, ScoringRecord } from './types.scoring';
import type { SchoolClass, StudentScanBatch, TeachingAssignment } from './types.schoolClass';
import type { SpeakingExam } from './types.speaking';
import type { PageGroupingMode, Section, Student, StudentSubmission } from './types.student';
import type { WorkflowSnapshot } from './types.workflow';

export type ExamPackageFreezeStatus = 'frozen' | 'invalidated';

export type ExamPackageFreeze = {
  examPackageVersion: number;
  freezeStatus: ExamPackageFreezeStatus;
  frozenAt: string;
  frozenBy?: string | null;
  sourceHash: string;
  rubricHash: string;
  questionTextHash: string;
  invalidatedAt?: string | null;
  invalidationReason?: string | null;
};

export type ProjectSnapshot = {
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  rootPath: string;
  /** Backend-owned persistence revision; callers must not manufacture it. */
  storageRevision?: number;
  academicYearId?: string | null;
  courseId?: string | null;
  courseName?: string | null;
  expectedQuestionCount?: number | null;
  examPackageFreeze?: ExamPackageFreeze | null;
  sections: Section[];
  schoolClasses: SchoolClass[];
  teachingAssignments: TeachingAssignment[];
  assessmentActivities: AssessmentActivity[];
  studentScanBatches: StudentScanBatch[];
  students: Student[];
  studentSubmissions: StudentSubmission[];
  studentAnswerOcrRecords: StudentAnswerOcrRecord[];
  studentAnswerOcrGenerations?: OcrGeneration[];
  scoringRecords: ScoringRecord[];
  scoringAnchors: ScoringAnchor[];
  speakingExams: SpeakingExam[];
  studentAnswerCropTemplate: StudentAnswerCropTemplate;
  studentIdentityCropTemplate?: StudentIdentityCropTemplate | null;
  studentScanDocumentId?: string | null;
  studentGroupingMode?: PageGroupingMode | null;
  studentPagesPerStudent?: number | null;
  studentGroupingCompleteAt?: string | null;
  latestScoringRunId?: string | null;
  documents: Document[];
  questions: Question[];
  workflow: WorkflowSnapshot;
};

export type ProjectListItem = {
  id: string;
  name: string;
  path: string;
  createdAt?: string;
  updatedAt?: string;
  questionCount?: number;
  documentRoles?: string[];
  statusSummary?: {
    hasExamSource: boolean;
    hasAnswerKeyOrRubric: boolean;
    hasStudentScan: boolean;
    questionTextCoverage?: string;
    rubricCoverage?: string;
  };
};

export type ListProjectsSkippedProject = {
  path: string;
  reason: string;
  technicalDetails?: string | null;
};

export type ListProjectsOutput = {
  projects: ProjectListItem[];
  warnings: string[];
  skippedProjects: ListProjectsSkippedProject[];
};

export type RemoveDocumentInput = {
  projectId: string;
  documentId: string;
};

export type CreateProjectInput = {
  name: string;
  rootPath: string;
  academicYearId: string;
  courseId: string;
  courseName: string;
};

export type CreateProjectOutput = {
  project: ProjectSnapshot;
  projectPath: string;
  warnings: string[];
};

export type ProjectOpenMode = 'inspectReadOnly' | 'openWithoutMigration' | 'migrateWithVerifiedBackup';

export type OpenProjectInput = {
  projectPath?: string;
  rootPath?: string;
  mode?: ProjectOpenMode;
};

export type OpenProjectOutput = {
  project: ProjectSnapshot;
  projectPath: string;
  warnings: string[];
};

export type OrphanArtifactReport = {
  relativePath: string;
  size: number;
  sha256?: string | null;
  fileType: string;
  probableSubsystem: string;
  probableEntityOrGeneration?: string | null;
  reason: string;
  indirectReferencePossible: boolean;
  teacherContentPossible: boolean;
  classification: string;
  recommendedAction: string;
};

export type DataLossPreflightReport = {
  projectPath: string;
  readOnly: boolean;
  readOnlyGuaranteeVerified: boolean;
  projectFileExists: boolean;
  projectParseOk: boolean;
  projectId?: string | null;
  storageRevision?: number | null;
  projectRevision?: number | null;
  projectFingerprint?: string | null;
  sourceManifestHash: string;
  sourceByteChanges: number;
  pendingMigration: boolean;
  migrationBackupStatus: string;
  recursiveFileCount: number;
  recursiveByteCount: number;
  recursiveInventorySha256: string;
  symlinkCount: number;
  symlinkPaths: string[];
  missingActivePointerCount: number;
  missingReferencedArtifactCount: number;
  brokenActivePointerCount: number;
  orphanArtifactCount: number;
  unknownOrphanCount: number;
  orphanArtifacts: OrphanArtifactReport[];
  orphanRestoreStagingCount: number;
  unsafeRestoreStagingCount: number;
  unsafeImportStagingCount: number;
  speakingAudioWithoutMetadataCount: number;
  speakingMetadataWithoutAudioCount: number;
  recoverableAudioOrphanCount: number;
  staleGcPlanCount: number;
  incompleteTransactionCount: number;
  ambiguousTransactionCount: number;
  auditProjectDivergenceCount: number;
  activeRevisionDivergenceCount: number;
  originalAuditStatus: string;
  activeAuditStatus: string;
  historicalRecoveryAnchorStatus: string;
  durabilityUncertainCount: number;
  secondWriterDetected: boolean;
  initializationWriteAllowed: boolean;
  unverifiedWritesAllowed: boolean;
  audit: {
    recordCount: number;
    chainValid: boolean;
    tamperCount: number;
    reasons: string[];
    projectRevisionDivergenceCount: number;
    activeRevisionDivergenceCount: number;
    firstInvalidLine?: number | null;
    firstInvalidRecordId?: string | null;
    firstInvalidPreviousHash?: string | null;
    firstInvalidComputedHash?: string | null;
    firstInvalidRecordedHash?: string | null;
    lastValidRecordHash: string;
    lastAuditRevision?: number | null;
    duplicateRevisionCount: number;
    missingRevisionCount: number;
    originalAuditStatus: string;
    activeAuditStatus: string;
    historicalRecoveryAnchorStatus: string;
    classifications: string[];
  };
  verifiedBackupCount: number;
  failedBackupCount: number;
  backupPaths: string[];
  latestVerifiedBackupPath?: string | null;
  verifiedBackupPath?: string | null;
  verifiedBackupSha256?: string | null;
  verifiedBackupRestoreStatus: string;
  latestVerifiedBackupAge?: string | null;
  processKillProofsStatus: string;
  diskFaultProofsStatus: string;
  destructiveRaceProofsStatus: string;
  fullTestSuiteGreen: boolean;
  blockers: string[];
  warnings: string[];
  errors: string[];
  decision: 'SAFE_TO_OPEN' | 'SAFE_TO_OPEN_WITH_BACKUP' | 'DO_NOT_OPEN_FOR_WRITING' | string;
  safeToOpenForWriting: boolean;
};

export type GcReport = {
  dryRun: boolean;
  protectedGenerations: number;
  cleanupCandidates: number;
  deletedGenerations: number;
  deferredCleanup: number;
  orphanStagingDirs: number;
};

export type BackupSummary = {
  archivePath: string;
  verificationPath?: string;
  sourceProjectPath?: string;
  entryCount: number;
  totalSize: number;
  sha256: string;
  createdAt: string;
};

export type RestoreSummary = {
  destination: string;
  entryCount: number;
  restoredProjectId: string;
};

export type StartBackupJobOutput = {
  jobId: string;
  status: string;
};

export type StartRestoreJobOutput = {
  jobId: string;
  status: string;
};

export type StartRecoveryCopyJobOutput = {
  jobId: string;
  status: string;
};
