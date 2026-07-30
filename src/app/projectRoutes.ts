export type ProjectArea =
  | 'projects'
  | 'overview'
  | 'documents'
  | 'exam'
  | 'classes'
  | 'students'
  | 'grading'
  | 'analysis'
  | 'settings';
export type ExamPackageTab = 'question' | 'rubric' | 'freeze';
export type StudentOperationsTab = 'grouping' | 'identity' | 'crops' | 'ocr' | 'issues';

export type ProjectNavigationItem = {
  area: ProjectArea;
  label: string;
  description: string;
  path: (projectId: string) => string;
};

export const projectNavigation: ProjectNavigationItem[] = [
  {
    area: 'projects',
    label: 'Yeni Proje',
    description: 'Yeni çalışma alanı oluştur',
    path: () => '/projects/new',
  },
  {
    area: 'overview',
    label: 'İş Akışı',
    description: 'İlerleme ve sonraki adım',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/overview`,
  },
  {
    area: 'documents',
    label: 'Belgeler',
    description: 'Sınav ve öğrenci PDF’leri',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/exam/documents`,
  },
  {
    area: 'exam',
    label: 'Sınav Paketi',
    description: 'Sorular, rubrikler ve dondurma',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/exam/package`,
  },
  {
    area: 'classes',
    label: 'Sınıflar',
    description: 'Sınıf ve PDF paketleri',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/classes`,
  },
  {
    area: 'students',
    label: 'Öğrenci İşlemleri',
    description: 'Gruplama, kimlik ve OCR',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/students?tab=grouping`,
  },
  {
    area: 'grading',
    label: 'Notlandırma',
    description: 'Sonuçlar ve kâğıt inceleme',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/grading`,
  },
  {
    area: 'analysis',
    label: 'Analiz',
    description: 'Grafikler ve Gemma raporu',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/analysis`,
  },
  {
    area: 'settings',
    label: 'Model Durumu',
    description: 'Yerel model ve sağlık durumu',
    path: (projectId) => `/project/${encodeURIComponent(projectId)}/settings/model`,
  },
];

const legacyProjectRouteAreas: Record<string, ProjectArea> = {
  '/workflow': 'overview',
  '/documents': 'documents',
  '/pdf-preview': 'documents',
  '/question-text': 'exam',
  '/rubric-preparation': 'exam',
  '/exam-package-review': 'exam',
  '/student-scans': 'students',
  '/student-grouping': 'students',
  '/student-identity': 'students',
  '/crop-template': 'students',
  '/student-answer-ocr': 'students',
  '/student-answer-ocr-issues': 'students',
  '/scoring': 'grading',
  '/graded-exam-review': 'grading',
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
  if (/^\/project\/[^/]+\/exam\/(?:documents|preview)(?:\/|$)/.test(pathname)) return 'documents';
  if (/^\/project\/[^/]+\/(?:students|ocr)(?:\/|$)/.test(pathname)) return 'students';
  const match = pathname.match(/^\/project\/[^/]+\/(overview|exam|classes|grading|analysis|settings)(?:\/|$)/);
  return (match?.[1] as ProjectArea | undefined) ?? legacyProjectRouteAreas[pathname] ?? null;
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
