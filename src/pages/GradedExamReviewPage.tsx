import { useEffect, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ChevronLeft, ChevronRight, FileCheck2 } from 'lucide-react';
import { useSearchParams } from 'react-router-dom';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { ScoredExamReviewPanel } from '../components/scoring/ScoredExamReviewPanel';
import { getGradedExamReviewQueue } from '../components/scoring/gradedExamReviewUi';
import { useProjectContext } from '../state/useProjectContext';

export function GradedExamReviewPage() {
  const { projectId, projectPath, isResolving } = useProjectContext();
  const [searchParams, setSearchParams] = useSearchParams();
  const requestedSubmissionId = searchParams.get('submissionId') ?? '';

  const { data: project, error: projectError, isLoading: projectLoading } = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: !!projectId,
  });

  const queue = useMemo(() => project ? getGradedExamReviewQueue(project) : [], [project]);
  const selectedSubmissionId = queue.includes(requestedSubmissionId) ? requestedSubmissionId : (queue[0] ?? '');
  const selectedIndex = selectedSubmissionId ? queue.indexOf(selectedSubmissionId) : -1;

  useEffect(() => {
    if (!projectId || !selectedSubmissionId || requestedSubmissionId === selectedSubmissionId) return;
    setSearchParams({ projectId, submissionId: selectedSubmissionId }, { replace: true });
  }, [projectId, requestedSubmissionId, selectedSubmissionId, setSearchParams]);

  const { data: review, error: reviewError, isLoading: reviewLoading } = useQuery({
    queryKey: ['graded-exam-review', projectId, selectedSubmissionId, project?.latestScoringRunId],
    queryFn: () => commands.getGradedExamReview({ projectId, submissionId: selectedSubmissionId }),
    enabled: !!projectId && !!selectedSubmissionId,
    retry: false,
  });

  if (isResolving) return <ProjectContextState pageLabel="Kâğıt İnceleme" loading projectPath={projectPath} />;
  if (!projectId) return <ProjectContextState pageLabel="Kâğıt İnceleme" projectPath={projectPath} />;

  const goToStudent = (index: number) => {
    const submissionId = queue[index];
    if (!submissionId) return;
    setSearchParams({ projectId, submissionId });
  };

  const queryError = (projectError as AppError | null) || (reviewError as AppError | null);

  return (
    <div style={{ height: '100vh', minHeight: 0, display: 'grid', gridTemplateRows: 'auto minmax(0, 1fr)', background: '#e2e8f0' }}>
      <header style={{ padding: '0.75rem 1rem', background: 'white', borderBottom: '1px solid #dbe4ef', display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '1rem' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.7rem', minWidth: 0 }}>
          <div style={{ width: '2.25rem', height: '2.25rem', display: 'grid', placeItems: 'center', borderRadius: '0.65rem', background: '#fee2e2', color: '#b91c1c' }}><FileCheck2 size={19} /></div>
          <div>
            <h1 style={{ margin: 0, fontSize: '1rem', color: '#0f172a' }}>Kâğıt Üzerinde İnceleme</h1>
            <p style={{ margin: '0.15rem 0 0', color: '#64748b', fontSize: '0.75rem' }}>{project?.name ?? 'Proje'} · {queue.length} öğrenci</p>
          </div>
        </div>

        {queue.length > 0 && (
          <div style={{ display: 'flex', alignItems: 'center', gap: '0.45rem' }}>
            <button type="button" onClick={() => goToStudent(selectedIndex - 1)} disabled={selectedIndex <= 0} style={{ ...studentNavButtonStyle, opacity: selectedIndex <= 0 ? 0.4 : 1 }}><ChevronLeft size={17} /> Önceki öğrenci</button>
            <span style={{ minWidth: '4.5rem', textAlign: 'center', fontSize: '0.78rem', fontWeight: 800, color: '#334155' }}>{selectedIndex + 1} / {queue.length}</span>
            <button type="button" onClick={() => goToStudent(selectedIndex + 1)} disabled={selectedIndex >= queue.length - 1} style={{ ...studentNavButtonStyle, opacity: selectedIndex >= queue.length - 1 ? 0.4 : 1 }}>Sonraki öğrenci <ChevronRight size={17} /></button>
          </div>
        )}
      </header>

      <div style={{ minHeight: 0, position: 'relative' }}>
        {queryError && <div style={{ padding: '1rem' }}><ErrorBanner error={queryError} /></div>}
        {(projectLoading || reviewLoading) && <div style={{ height: '100%', display: 'grid', placeItems: 'center', color: '#475569' }}>Puanlı sınav kâğıdı hazırlanıyor…</div>}
        {!projectLoading && queue.length === 0 && !queryError && (
          <div style={{ height: '100%', display: 'grid', placeItems: 'center', padding: '2rem' }}>
            <div style={{ maxWidth: '28rem', textAlign: 'center', background: 'white', border: '1px solid #dbe4ef', borderRadius: '1rem', padding: '1.5rem', color: '#475569' }}>
              <strong style={{ display: 'block', color: '#0f172a', marginBottom: '0.4rem' }}>İncelenecek kâğıt henüz yok</strong>
              Önce Notlandırma modülünde öğrenci sonuçlarını oluşturun.
            </div>
          </div>
        )}
        {review && !reviewLoading && <ScoredExamReviewPanel review={review} />}
      </div>
    </div>
  );
}

const studentNavButtonStyle = {
  display: 'inline-flex', alignItems: 'center', gap: '0.35rem', padding: '0.5rem 0.7rem', borderRadius: '0.6rem', border: '1px solid #cbd5e1', background: 'white', color: '#334155', fontWeight: 750, fontSize: '0.78rem', cursor: 'pointer',
} as const;
