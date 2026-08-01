import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type { WorkflowAction } from '../../api/types';
import { commands } from '../../api/commands';
import type { AppError } from '../../api/errors';
import { ErrorBanner } from '../common/ErrorBanner';
import { LoadingButton } from '../common/LoadingButton';
import { useQueryClient } from '@tanstack/react-query';
import { QuestionCountDialog } from '../common/QuestionCountDialog';
import { useProjectContext } from '../../state/useProjectContext';
import { getExamPackageActionPath, getStudentOperationsActionPath } from '../../app/projectRoutes';

export function NextActions({ actions }: { actions: WorkflowAction[] }) {
  const navigate = useNavigate();
  const { projectId } = useProjectContext();
  const queryClient = useQueryClient();

  const [loadingAction, setLoadingAction] = useState<string | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);
  const [questionCountDialogOpen, setQuestionCountDialogOpen] = useState(false);
  const [questionCountAction, setQuestionCountAction] = useState<string | null>(null);

  if (!actions || actions.length === 0) return null;

  const handleActionClick = async (action: WorkflowAction) => {
    if (!projectId || !action.enabled) return;

    setError(null);
    setSuccessMsg(null);
    setLoadingAction(action.code);

    try {
      const commandCode = action.command ?? action.code;
      const examPackageDestination = getExamPackageActionPath(projectId, commandCode);
      if (examPackageDestination) {
        navigate(examPackageDestination);
        return;
      }
      const studentOperationsDestination = getStudentOperationsActionPath(projectId, commandCode);
      if (studentOperationsDestination) {
        navigate(studentOperationsDestination);
        return;
      }
      switch (commandCode) {
        case 'import_exam_source_pdf':
          navigate(`/project/${encodeURIComponent(projectId)}/exam/documents`);
          break;

        case 'start_pdf_preview_render': {
          const project = await commands.getProjectSnapshot(projectId);
          const examSource = project.documents.find((document) => document.role === 'exam_source');
          if (!examSource) {
            throw {
              code: 'PDF_DOCUMENT_NOT_FOUND',
              safeMessage: 'Sınav PDF’i bulunamadı.',
              recoveryAction: 'Önce sınav PDF’ini içe aktarın.',
              retryable: true,
              correlationId: 'unknown',
              detailsAvailable: false,
            } as AppError;
          }
          await commands.startPdfPreviewRender({ projectId, documentId: examSource.id });
          setSuccessMsg('PDF önizleme işi başlatıldı.');
          queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
          break;
        }

        case 'open_pdf_preview_page': {
          const project = await commands.getProjectSnapshot(projectId);
          const examSource = project.documents.find((document) => document.role === 'exam_source');
          if (!examSource) {
            throw {
              code: 'PDF_DOCUMENT_NOT_FOUND',
              safeMessage: 'Sınav PDF’i bulunamadı.',
              recoveryAction: 'Önce sınav PDF’ini içe aktarın.',
              retryable: true,
              correlationId: 'unknown',
              detailsAvailable: false,
            } as AppError;
          }
          navigate(`/project/${encodeURIComponent(projectId)}/exam/documents?documentId=${encodeURIComponent(examSource.id)}&documentType=exam`);
          break;
        }

        case 'start_question_text_extraction': {
          const project = await commands.getProjectSnapshot(projectId);
          const examSource = project.documents.find((document) => document.role === 'exam_source');
          if (!examSource) {
            throw {
              code: 'PDF_DOCUMENT_NOT_FOUND',
              safeMessage: 'Sınav PDF’i bulunamadı.',
              recoveryAction: 'Önce sınav PDF’ini içe aktarın.',
              retryable: true,
              correlationId: 'unknown',
              detailsAvailable: false,
            } as AppError;
          }
          await commands.startQuestionTextExtraction({
            projectId,
            documentId: examSource.id,
          });
          setSuccessMsg('PDF metni tarama işi başlatıldı.');
          queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
          break;
        }

        case 'start_exam_package_build':
          setQuestionCountAction('start_exam_package_build');
          setQuestionCountDialogOpen(true);
          break;

        case 'start_model_server':
          await commands.startModelServer();
          setSuccessMsg('Model sunucusu başlatıldı.');
          queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['model-status'] });
          break;

        case 'set_managed_mode':
          await commands.setModelMode({ profileId: undefined, mode: 'managed' });
          setSuccessMsg('Profil yönetilen moda alındı.');
          queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['model-status'] });
          break;

        case 'switch_to_managed_and_start':
          await commands.setModelMode({ profileId: undefined, mode: 'managed' });
          await commands.startModelServer();
          setSuccessMsg('Yönetilen moda geçildi ve model başlatıldı.');
          queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['model-status'] });
          break;

        case 'open_model_status_page':
          navigate(`/project/${encodeURIComponent(projectId)}/settings/model`);
          break;

        case 'confirm_all_question_texts':
          await commands.confirmAllQuestionTexts({ projectId });
          setSuccessMsg('Soru metni önerileri onaylandı.');
          queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
          break;

        case 'import_student_scan_pdf':
          navigate(`/project/${encodeURIComponent(projectId)}/exam/documents`);
          break;

        case 'open_scoring_page':
          navigate(`/project/${encodeURIComponent(projectId)}/grading`);
          break;

        case 'start_student_answer_ocr':
          await commands.startStudentAnswerOcr({
            projectId,
            forceRerun: action.code === 'rerun_student_answer_ocr',
          });
          setSuccessMsg('Öğrenci cevap OCR işi başlatıldı.');
          queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
          break;

        case 'validate_rubrics':
          await commands.validateRubrics({ projectId });
          setSuccessMsg('Rubrikler doğrulandı.');
          queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['rubric-state', projectId] });
          queryClient.invalidateQueries({ queryKey: ['rubric-items', projectId] });
          break;

        case 'start_scoring_job':
          await commands.startScoringJob({ projectId, forceRerun: false });
          setSuccessMsg('Notlandırma işi başlatıldı.');
          queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
          break;

        case 'start_rubric_pdf_import':
          setQuestionCountAction('start_rubric_pdf_import');
          setQuestionCountDialogOpen(true);
          break;

        case 'start_student_scan_preview_render': {
          const project = await commands.getProjectSnapshot(projectId);
          const studentScan =
            (project.studentScanDocumentId
              ? project.documents.find((document) => document.id === project.studentScanDocumentId)
              : null) ??
            project.documents.find((document) => document.role === 'student_scan');
          if (!studentScan) {
            throw {
              code: 'STUDENT_SCAN_NOT_FOUND',
              safeMessage: 'Öğrenci cevap PDF’i bulunamadı.',
              recoveryAction: 'Önce öğrenci cevap PDF’ini içe aktarın.',
              retryable: true,
              correlationId: 'unknown',
              detailsAvailable: false,
            } as AppError;
          }
          await commands.startStudentScanPreviewRender({ projectId, documentId: studentScan.id });
          setSuccessMsg('Öğrenci PDF önizleme işi başlatıldı.');
          queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['student-scan-documents', projectId] });
          queryClient.invalidateQueries({ queryKey: ['ocr-readiness', projectId] });
          break;
        }

        case 'mark_student_grouping_complete':
          await commands.markStudentGroupingComplete({ projectId });
          setSuccessMsg('Öğrenci gruplaması tamamlandı.');
          queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
          queryClient.invalidateQueries({ queryKey: ['student-submissions', projectId] });
          queryClient.invalidateQueries({ queryKey: ['ocr-readiness', projectId] });
          break;

        default:
          setError({
            code: 'UNKNOWN_ERROR',
            safeMessage: `Bu aksiyon henüz desteklenmiyor: ${action.label}`,
            retryable: true,
            correlationId: 'unknown',
            detailsAvailable: false,
          } as AppError);
          break;
      }
    } catch (err) {
      setError(err as AppError);
    } finally {
      setLoadingAction(null);
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      {error && <div style={{ marginBottom: '0.5rem' }}><ErrorBanner error={error} /></div>}
      {successMsg && (
        <div style={{ padding: '0.75rem', background: '#dcfce7', color: '#166534', borderRadius: '0.5rem', fontSize: '0.875rem' }}>
          {successMsg}
        </div>
      )}

      <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap' }}>
        {actions.map((a, i) => {
          const isPrimary = i === 0;
          return (
            <LoadingButton 
              key={i} 
              onClick={() => handleActionClick(a)}
              loading={loadingAction === a.code}
              disabledReason={!a.enabled ? (a.disabledReason || 'Bu aksiyon şu an kullanılamaz.') : undefined}
              style={{ 
                padding: '0.5rem 1rem', 
                background: a.enabled ? (isPrimary ? '#4f46e5' : '#ffffff') : '#f8fafc', 
                color: a.enabled ? (isPrimary ? 'white' : '#0f172a') : '#94a3b8', 
                border: a.enabled && !isPrimary ? '1px solid #e2e8f0' : (a.enabled ? 'none' : '1px solid #cbd5e1'), 
                borderRadius: '0.75rem',
                fontWeight: 600,
                fontSize: '0.875rem',
                boxShadow: a.enabled && isPrimary ? '0 4px 6px -1px rgba(79, 70, 229, 0.2)' : 'none',
                cursor: a.enabled ? 'pointer' : 'not-allowed',
                transition: 'all 0.2s'
              }}
            >
              {a.label}
            </LoadingButton>
          );
        })}
      </div>

      <QuestionCountDialog
        open={questionCountDialogOpen}
        title="Soru sayısını girin"
        description="Bu sınavda kaç soru olduğunu girin. Sistem her soru için ayrı prompt gönderecek."
        confirmLabel="Devam Et"
        onCancel={() => {
          setQuestionCountDialogOpen(false);
          setQuestionCountAction(null);
        }}
        onConfirm={async (expectedQuestionCount) => {
          if (!questionCountAction) return false;
          try {
            if (questionCountAction === 'start_exam_package_build') {
              await commands.startExamPackageBuild({ projectId, expectedQuestionCount });
              setSuccessMsg('Sınav paketi oluşturma işi başlatıldı, işlem devam ediyor...');
              queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
              queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
              queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
            } else if (questionCountAction === 'start_rubric_pdf_import') {
              await commands.startRubricPdfImport({ projectId, expectedQuestionCount });
              setSuccessMsg('Rubrik PDF çıkarma işi başlatıldı, işlem devam ediyor...');
              queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
              queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
              queryClient.invalidateQueries({ queryKey: ['rubric-state', projectId] });
              queryClient.invalidateQueries({ queryKey: ['rubric-items', projectId] });
              queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
            }
            return true;
          } catch (err) {
            setError(err as AppError);
            return false;
          } finally {
            setQuestionCountAction(null);
          }
        }}
      />
    </div>
  );
}
