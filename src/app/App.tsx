import { QueryClientProvider } from '@tanstack/react-query';
import { BrowserRouter, Navigate, Routes, Route, useLocation } from 'react-router-dom';
import { lazy, Suspense } from 'react';
import { queryClient } from './queryClient';
import { AppErrorBoundary } from '../components/common/AppErrorBoundary';
import { AppLayout } from './AppLayout';
import { resolveLegacyProjectDestination } from './projectRoutes';
import { useProjectContext } from '../state/useProjectContext';
import { StartupRedirect } from './StartupRedirect';

const ProjectCreatePage = lazy(() => import('../pages/ProjectCreatePage').then((module) => ({ default: module.ProjectCreatePage })));
const DocumentsPage = lazy(() => import('../pages/DocumentsPage').then((module) => ({ default: module.DocumentsPage })));
const WorkflowPage = lazy(() => import('../pages/WorkflowPage').then((module) => ({ default: module.WorkflowPage })));
const ModelStatusPage = lazy(() => import('../pages/ModelStatusPage').then((module) => ({ default: module.ModelStatusPage })));
const PdfPreviewPage = lazy(() => import('../pages/PdfPreviewPage').then((module) => ({ default: module.PdfPreviewPage })));
const ExamPackageCompatibilityRedirect = lazy(() => import('../pages/ExamPackageWorkspacePage').then((module) => ({ default: module.ExamPackageCompatibilityRedirect })));
const ExamPackageWorkspacePage = lazy(() => import('../pages/ExamPackageWorkspacePage').then((module) => ({ default: module.ExamPackageWorkspacePage })));
const StudentOperationsCompatibilityRedirect = lazy(() => import('../pages/StudentOperationsWorkspacePage').then((module) => ({ default: module.StudentOperationsCompatibilityRedirect })));
const StudentOperationsWorkspacePage = lazy(() => import('../pages/StudentOperationsWorkspacePage').then((module) => ({ default: module.StudentOperationsWorkspacePage })));
const ClassesPage = lazy(() => import('../pages/ClassesPage').then((module) => ({ default: module.ClassesPage })));
const ScoringPage = lazy(() => import('../pages/ScoringPage').then((module) => ({ default: module.ScoringPage })));
const GradedExamReviewPage = lazy(() => import('../pages/GradedExamReviewPage').then((module) => ({ default: module.GradedExamReviewPage })));
const SpeechExamPage = lazy(() => import('../pages/SpeechExamPage').then((module) => ({ default: module.SpeechExamPage })));
const AnalysisPage = lazy(() => import('../pages/AnalysisPage').then((module) => ({ default: module.AnalysisPage })));

function LegacyProjectRedirect({ pathname }: { pathname: string }) {
  const { projectId, isResolving } = useProjectContext();
  const location = useLocation();
  if (isResolving) return <div style={{ padding: '2rem' }}>Proje bağlamı yükleniyor…</div>;
  const destination = projectId
    ? resolveLegacyProjectDestination(pathname, projectId, location.search)
    : null;
  if (!destination) return <Navigate to="/projects" replace />;
  return <Navigate to={destination} replace />;
}

function AppRoutes() {
  const location = useLocation();

  return (
    <AppErrorBoundary key={location.key}>
      <AppLayout>
        <Suspense fallback={<div style={{ padding: '2rem', color: '#64748b' }}>Sayfa yükleniyor…</div>}>
          <Routes>
          <Route path="/" element={<StartupRedirect />} />
          <Route path="/projects" element={<StartupRedirect />} />
          <Route path="/projects/new" element={<ProjectCreatePage />} />
          <Route path="/project-create" element={<Navigate to="/projects/new" replace />} />

          <Route path="/project/:projectId" element={<WorkflowPage />} />
          <Route path="/project/:projectId/overview" element={<WorkflowPage />} />
          <Route path="/project/:projectId/exam" element={<DocumentsPage />} />
          <Route path="/project/:projectId/exam/documents" element={<DocumentsPage />} />
          <Route path="/project/:projectId/exam/preview" element={<PdfPreviewPage />} />
          <Route path="/project/:projectId/exam/questions" element={<ExamPackageCompatibilityRedirect tab="question" />} />
          <Route path="/project/:projectId/exam/rubrics" element={<ExamPackageCompatibilityRedirect tab="rubric" />} />
          <Route path="/project/:projectId/exam/package-review" element={<ExamPackageCompatibilityRedirect tab="freeze" />} />
          <Route path="/project/:projectId/exam/package" element={<ExamPackageWorkspacePage />} />
          <Route path="/project/:projectId/classes" element={<ClassesPage />} />
          <Route path="/project/:projectId/students" element={<StudentOperationsWorkspacePage />} />
          <Route path="/project/:projectId/students/grouping" element={<StudentOperationsCompatibilityRedirect tab="grouping" />} />
          <Route path="/project/:projectId/students/crops" element={<StudentOperationsCompatibilityRedirect tab="crops" />} />
          <Route path="/project/:projectId/students/identities" element={<StudentOperationsCompatibilityRedirect tab="identity" />} />
          <Route path="/project/:projectId/ocr" element={<StudentOperationsCompatibilityRedirect tab="ocr" />} />
          <Route path="/project/:projectId/ocr/review" element={<StudentOperationsCompatibilityRedirect tab="issues" />} />
          <Route path="/project/:projectId/ocr/issues" element={<StudentOperationsCompatibilityRedirect tab="issues" />} />
          <Route path="/project/:projectId/grading" element={<ScoringPage />} />
          <Route path="/project/:projectId/grading/students" element={<ScoringPage />} />
          <Route path="/project/:projectId/grading/papers" element={<GradedExamReviewPage />} />
          <Route path="/project/:projectId/analysis" element={<AnalysisPage kind="written" />} />
          <Route path="/project/:projectId/settings" element={<ModelStatusPage />} />
          <Route path="/project/:projectId/settings/model" element={<ModelStatusPage />} />
          <Route path="/project/:projectId/settings/diagnostics" element={<ModelStatusPage />} />
          <Route path="/project/:projectId/speaking" element={<SpeechExamPage />} />
          <Route path="/project/:projectId/speaking/analysis" element={<AnalysisPage kind="speaking" />} />

          {[
            '/documents',
            '/student-scans',
            '/workflow',
            '/pdf-preview',
            '/student-grouping',
            '/student-identity',
            '/crop-template',
            '/student-answer-ocr',
            '/student-answer-ocr-issues',
            '/scoring',
            '/graded-exam-review',
            '/question-text',
            '/rubric-preparation',
            '/exam-package-review',
            '/model-status',
          ].map((pathname) => (
            <Route key={pathname} path={pathname} element={<LegacyProjectRedirect pathname={pathname} />} />
          ))}
          </Routes>
        </Suspense>
      </AppLayout>
    </AppErrorBoundary>
  );
}

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <AppRoutes />
      </BrowserRouter>
    </QueryClientProvider>
  );
}
