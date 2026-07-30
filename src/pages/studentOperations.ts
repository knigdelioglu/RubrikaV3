import type {
  ProjectSnapshot,
  SchoolClass,
  SchoolClassOverview,
  Student,
  StudentScanBatch,
  StudentSubmission,
} from '../api/types';
import type { StudentOperationsTab } from '../app/projectRoutes';

export type StudentOperationsSelection = {
  classId: string;
  batchId: string;
};

export function normalizeStudentOperationsTab(rawTab: string | null): StudentOperationsTab {
  switch (rawTab) {
    case 'identity':
    case 'crops':
    case 'ocr':
    case 'issues':
      return rawTab;
    default:
      return 'grouping';
  }
}

export function resolveStudentOperationsSelection(
  classes: SchoolClass[],
  batches: StudentScanBatch[],
  requestedClassId: string | null,
  requestedBatchId: string | null,
): StudentOperationsSelection {
  const classId = requestedClassId && classes.some((item) => item.id === requestedClassId)
    ? requestedClassId
    : '';
  const requestedBatch = requestedBatchId
    ? batches.find((batch) => batch.id === requestedBatchId)
    : undefined;
  const batchId = requestedBatch && (!classId || requestedBatch.classId === classId)
    ? requestedBatch.id
    : '';
  return {
    classId: classId || requestedBatch?.classId || '',
    batchId,
  };
}

export function filterStudentSubmissions(
  submissions: StudentSubmission[],
  classId: string,
  batchId: string,
): StudentSubmission[] {
  return submissions.filter((submission) => {
    if (batchId && submission.scanBatchId !== batchId) return false;
    if (classId && submission.classId !== classId) return false;
    return true;
  });
}

export function getSubmissionSchoolClass(
  project: Pick<ProjectSnapshot, 'schoolClasses' | 'students'>,
  submission: Pick<StudentSubmission, 'studentId' | 'classId'>,
): SchoolClass | null {
  if (submission.classId) {
    const canonical = project.schoolClasses.find((item) => item.id === submission.classId);
    if (canonical) return canonical;
  }

  const legacyClassName = project.students.find((student) => student.id === submission.studentId)?.className;
  if (!legacyClassName?.trim()) return null;
  const normalizedLegacyName = normalizeClassNameForSuggestion(legacyClassName);
  return project.schoolClasses.find((item) => (
    normalizeClassNameForSuggestion(item.normalizedName || item.name) === normalizedLegacyName
  )) ?? null;
}

export function getSubmissionClassName(
  project: Pick<ProjectSnapshot, 'schoolClasses' | 'students'>,
  submission: Pick<StudentSubmission, 'studentId' | 'classId'>,
): string {
  const schoolClass = getSubmissionSchoolClass(project, submission);
  if (schoolClass) return schoolClass.name;
  const legacyName = project.students.find((student) => student.id === submission.studentId)?.className?.trim();
  return legacyName || 'Sınıfı belirlenmemiş';
}

export function getStudentTeacherLabel(student: Student | null | undefined, className: string): string {
  const name = student?.displayName?.trim();
  const number = student?.number?.trim();
  if (name) return [name, number ? `No ${number}` : null, className].filter(Boolean).join(' · ');
  if (number) return `Öğrenci ${number} · ${className}`;
  return className === 'Sınıfı belirlenmemiş'
    ? 'Kimliği doğrulanmamış öğrenci'
    : `Kimliği doğrulanmamış öğrenci · ${className}`;
}

export function hasIdentityClassMismatch(
  project: Pick<ProjectSnapshot, 'schoolClasses' | 'students'>,
  submission: Pick<StudentSubmission, 'studentId' | 'classId'>,
  detectedClassName: string | null | undefined,
): boolean {
  const detected = detectedClassName?.trim();
  const canonical = getSubmissionSchoolClass(project, submission);
  if (!detected || !canonical) return false;
  return normalizeClassNameForSuggestion(detected) !== normalizeClassNameForSuggestion(canonical.name);
}

export function normalizeClassNameForSuggestion(value: string): string {
  const withoutExtension = value.replace(/\.[^.]+$/, '');
  const match = withoutExtension.match(/(?:^|\D)(\d{1,2})\s*[-_. ]?\s*([a-zçğıöşü])(?:\b|\D)/i);
  if (match?.[1] && match[2]) return `${match[1]}-${match[2].toLocaleUpperCase('tr-TR')}`;
  return withoutExtension
    .trim()
    .toLocaleUpperCase('tr-TR')
    .replace(/[_.\s]+/g, '-')
    .replace(/-+/g, '-');
}

export function suggestSchoolClassFromFilename(
  fileName: string,
  classes: SchoolClass[],
): SchoolClass | null {
  const suggestion = normalizeClassNameForSuggestion(fileName);
  return classes.find((item) => (
    normalizeClassNameForSuggestion(item.normalizedName || item.name) === suggestion
  )) ?? null;
}

export function getStudentBatchImportDisabledReason(sourcePath: string | null, classId: string): string | undefined {
  if (!sourcePath) return 'Önce PDF seçin.';
  if (!classId) return 'Önce sınıf seçin.';
  return undefined;
}

export function aggregateClassOverview(items: SchoolClassOverview[]) {
  return items.reduce((summary, item) => ({
    scanBatchCount: summary.scanBatchCount + item.scanBatchCount,
    submissionCount: summary.submissionCount + item.submissionCount,
    identityVerifiedCount: summary.identityVerifiedCount + item.identityVerifiedCount,
    ocrCompleteCount: summary.ocrCompleteCount + item.ocrCompleteCount,
    scoringCompleteCount: summary.scoringCompleteCount + item.scoringCompleteCount,
    reviewRequiredCount: summary.reviewRequiredCount + item.reviewRequiredCount,
  }), {
    scanBatchCount: 0,
    submissionCount: 0,
    identityVerifiedCount: 0,
    ocrCompleteCount: 0,
    scoringCompleteCount: 0,
    reviewRequiredCount: 0,
  });
}
