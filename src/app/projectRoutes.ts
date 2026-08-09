export type ProjectArea =
  | 'overview'
  | 'activities'
  | 'classes'
  | 'analysis'
  | 'settings'
  | 'projects'
  | 'documents'
  | 'exam'
  | 'students'
  | 'grading';
export type ExamPackageTab = 'documents' | 'question' | 'rubric' | 'freeze';
export type StudentOperationsTab = 'roster' | 'grouping' | 'identity' | 'crops' | 'ocr' | 'issues';

export type ProjectNavigationItem = {
  area: ProjectArea;
  label: string;
  description: string;
  path: (projectId: string) => string;
};

export const projectNavigation: ProjectNavigationItem[] = [
  {
    area: 'overview',
    label: 'Ana Sayfa',
    description: 'Sınav takibi ve genel ilerleme',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/overview`,
  },
  {
    area: 'activities',
    label: 'Sınavlar',
    description: 'Ortak sınavlar ve sınıf uygulamaları',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/activities`,
  },
  {
    area: 'classes',
    label: 'Sınıflar ve Öğrenciler',
    description: 'Sınıflar, öğrenciler ve görevlendirmeler',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/classes`,
  },
  {
    area: 'analysis',
    label: 'Raporlar',
    description: 'Grafikler ve başarı raporları',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/analysis`,
  },
  {
    area: 'settings',
    label: 'Ayarlar',
    description: 'Model durumu ve sistem ayarları',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/settings/model`,
  },
];

const legacyProjectRouteAreas: Record<string, ProjectArea> = {
  '/workflow': 'overview',
  '/documents': 'activities',
  '/pdf-preview': 'activities',
  '/question-text': 'activities',
  '/rubric-preparation': 'activities',
  '/exam-package-review': 'activities',
  '/student-scans': 'activities',
  '/student-grouping': 'activities',
  '/student-identity': 'activities',
  '/crop-template': 'activities',
  '/student-answer-ocr': 'activities',
  '/student-answer-ocr-issues': 'activities',
  '/scoring': 'activities',
  '/graded-exam-review': 'activities',
  '/model-status': 'settings',
};

const legacyProjectRouteSuffixes: Record<string, string> = {
  '/workflow': 'overview',
  '/documents': 'exam/documents',
  '/pdf-preview': 'exam/documents',
  '/question-text': 'exam/package',
  '/rubric-preparation': 'exam/package',
  '/exam-package-review': 'exam/package',
  '/student-scans': 'students',
  '/student-grouping': 'students',
  '/student-identity': 'students',
  '/crop-template': 'students',
  '/student-answer-ocr': 'students',
  '/student-answer-ocr-issues': 'students',
  '/scoring': 'grading',
  '/graded-exam-review': 'grading/papers',
  '/model-status': 'settings/model',
};

export function getProjectArea(pathname: string): ProjectArea | null {
  if (/^\/project\/[^/]+\/overview(?:\/|$)/.test(pathname)) return 'overview';
  if (/^\/project\/[^/]+\/activities(?:\/|$)/.test(pathname)) return 'activities';
  if (/^\/project\/[^/]+\/classes(?:\/|$)/.test(pathname)) return 'classes';
  if (/^\/project\/[^/]+\/analysis(?:\/|$)/.test(pathname)) return 'analysis';
  if (/^\/project\/[^/]+\/settings(?:\/|$)/.test(pathname)) return 'settings';
  if (/^\/project\/[^/]+\/(?:exam|grading|speaking|ocr|students)(?:\/|$)/.test(pathname)) return 'activities';
  return legacyProjectRouteAreas[pathname] ?? null;
}

export function getProjectIdFromPathname(pathname: string): string {
  const encodedProjectId = pathname.match(/^\/project\/([^/]+)(?:\/|$)/)?.[1];
  if (!encodedProjectId) return '';
  try {
    return decodeURIComponent(encodedProjectId);
  } catch {
    return '';
  }
}

export function resolveLegacyProjectPath(pathname: string, projectId: string): string | null {
  const suffix = legacyProjectRouteSuffixes[pathname];
  return suffix ? `/project/${encodeURIComponent(projectId)}/${suffix}` : null;
}

function appendPreservedSearch(destination: string, rawSearch: string): string {
  const search = new URLSearchParams(rawSearch);
  search.delete('projectId');
  search.delete('projectPath');
  const query = search.toString();
  return `${destination}${query ? `?${query}` : ''}`;
}

export function resolveLegacyProjectDestination(
  pathname: string,
  projectId: string,
  rawSearch = '',
): string | null {
  const searchParams = new URLSearchParams(rawSearch);
  const activityId = searchParams.get('assessmentActivityId');

  if (activityId) {
    const activityStepMap: Record<string, string> = {
      '/documents': 'prep',
      '/pdf-preview': 'prep',
      '/question-text': 'prep',
      '/rubric-preparation': 'prep',
      '/exam-package-review': 'prep',
      '/student-scans': 'students',
      '/student-grouping': 'students',
      '/student-identity': 'students',
      '/crop-template': 'students',
      '/student-answer-ocr': 'ocr',
      '/student-answer-ocr-issues': 'ocr',
      '/scoring': 'scoring',
      '/graded-exam-review': 'scoring',
      '/speaking': 'transcript',
    };

    const step = activityStepMap[pathname];
    if (step) {
      return appendPreservedSearch(
        `/project/${encodeURIComponent(projectId)}/activities/${encodeURIComponent(activityId)}/${step}`,
        rawSearch,
      );
    }
  }

  const destination = resolveLegacyProjectPath(pathname, projectId);
  if (!destination) return null;
  const defaultTab: Record<string, ExamPackageTab | undefined> = {
    '/question-text': 'question',
    '/rubric-preparation': 'rubric',
    '/exam-package-review': 'freeze',
  };
  const studentDefaultTab: Record<string, StudentOperationsTab | undefined> = {
    '/student-scans': 'grouping',
    '/student-grouping': 'grouping',
    '/student-identity': 'identity',
    '/crop-template': 'crops',
    '/student-answer-ocr': 'ocr',
    '/student-answer-ocr-issues': 'issues',
  };
  const withExamTab = withDefaultExamPackageTab(rawSearch, defaultTab[pathname]);
  return appendPreservedSearch(destination, withDefaultStudentOperationsTab(withExamTab, studentDefaultTab[pathname]));
}

function withDefaultExamPackageTab(rawSearch: string, tab: ExamPackageTab | undefined): string {
  if (!tab) return rawSearch;
  const search = new URLSearchParams(rawSearch);
  if (!search.has('tab')) search.set('tab', tab);
  return search.toString();
}

function withDefaultStudentOperationsTab(
  rawSearch: string,
  tab: StudentOperationsTab | undefined,
): string {
  if (!tab) return rawSearch;
  const search = new URLSearchParams(rawSearch);
  if (!search.has('tab')) search.set('tab', tab);
  return search.toString();
}

export function projectDocumentsPath(projectId: string, rawSearch = ''): string {
  return appendPreservedSearch(
    `/project/${encodeURIComponent(projectId)}/exam/documents`,
    rawSearch,
  );
}

export function projectExamPackagePath(
  projectId: string,
  tab: ExamPackageTab = 'question',
  rawSearch = '',
): string {
  const search = new URLSearchParams(rawSearch);
  search.delete('projectId');
  search.delete('projectPath');
  search.set('tab', tab);
  return appendPreservedSearch(
    `/project/${encodeURIComponent(projectId)}/exam/package`,
    search.toString(),
  );
}

export function getExamPackageActionPath(projectId: string, actionCode: string): string | null {
  switch (actionCode) {
    case 'open_question_text_page':
      return projectExamPackagePath(projectId, 'question');
    case 'open_rubric_preparation_page':
    case 'prepare_rubric':
      return projectExamPackagePath(projectId, 'rubric');
    case 'open_exam_package_review_page':
    case 'confirm_all_rubrics':
      return projectExamPackagePath(projectId, 'freeze');
    default:
      return null;
  }
}

export function projectStudentOperationsPath(
  projectId: string,
  tab: StudentOperationsTab = 'grouping',
  rawSearch = '',
): string {
  const search = new URLSearchParams(rawSearch);
  search.delete('projectId');
  search.delete('projectPath');
  search.set('tab', tab);
  return appendPreservedSearch(
    `/project/${encodeURIComponent(projectId)}/students`,
    search.toString(),
  );
}

export function getStudentOperationsActionPath(projectId: string, actionCode: string): string | null {
  switch (actionCode) {
    case 'open_student_scans_page':
    case 'open_student_grouping_page':
    case 'create_student_page_groups':
      return projectStudentOperationsPath(projectId, 'grouping');
    case 'open_student_identity_page':
      return projectStudentOperationsPath(projectId, 'identity');
    case 'open_crop_template_page':
      return projectStudentOperationsPath(projectId, 'crops');
    case 'open_student_answer_ocr_page':
      return projectStudentOperationsPath(projectId, 'ocr');
    case 'open_student_answer_ocr_issue_review_page':
      return projectStudentOperationsPath(projectId, 'issues');
    default:
      return null;
  }
}

export function projectOverviewPath(projectId: string): string {
  return projectNavigation.find((item) => item.area === 'overview')?.path(projectId)
    ?? `/project/${encodeURIComponent(projectId)}/overview`;
}
