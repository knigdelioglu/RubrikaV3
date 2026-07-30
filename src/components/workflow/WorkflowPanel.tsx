import { Link } from 'react-router-dom';
import type { JobSnapshot, ProjectSnapshot, WorkflowSnapshot } from '../../api/types';
import { projectNavigation } from '../../app/projectRoutes';
import { useProjectContext } from '../../state/useProjectContext';
import { BlockingReasons } from './BlockingReasons';
import { NextActions } from './NextActions';
import { getPrimaryWorkflowAction } from './workflowUi';
import { getWorkflowSummaryText } from './workflowSummary';
import { summarizeWorkflowAreas, type OverviewStatus } from './projectOverview';

type WorkflowPanelProps = {
  workflow?: WorkflowSnapshot;
  project?: ProjectSnapshot;
  jobs?: JobSnapshot[];
};

const statusLabels: Record<OverviewStatus, string> = {
  pending: 'Henüz başlanmadı',
  running: 'İşlem devam ediyor',
  succeeded: 'Tamamlandı',
  failed: 'Başarısız',
  partial: 'Kontrol gerekli',
};

export function WorkflowPanel({ workflow, project }: WorkflowPanelProps) {
  const { projectId } = useProjectContext();
  if (!workflow) return <div>Yükleniyor…</div>;

  const primaryAction = getPrimaryWorkflowAction(workflow.nextActions);
  const areas = summarizeWorkflowAreas(workflow.summary.steps);
  const summaryText = getWorkflowSummaryText(workflow);
  const ocrRecords = project?.studentAnswerOcrRecords ?? [];
  const approvedOcrCount = ocrRecords.filter((record) => record.status === 'teacher_approved').length;
  const reviewOcrCount = ocrRecords.filter((record) => record.needsReview).length;
  const scoringReviewCount = (project?.scoringRecords ?? []).filter((record) => record.needsReview).length;

  const areaPath = (area: 'exam' | 'students' | 'ocr' | 'grading') =>
    projectNavigation.find((item) => item.area === area)?.path(projectId) ?? '#';

  return (
    <div style={{ display: 'grid', gap: '1.5rem', maxWidth: '1180px', margin: '0 auto' }}>
      <section style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) auto', gap: '1.5rem', alignItems: 'center', padding: '1.5rem', color: '#fff', background: 'linear-gradient(135deg, #312e81, #4f46e5)', borderRadius: '1rem' }}>
        <div>
          <div style={{ fontSize: '0.75rem', fontWeight: 800, letterSpacing: '0.08em', textTransform: 'uppercase', color: '#c7d2fe' }}>Sonraki adım</div>
          <h2 style={{ margin: '0.45rem 0 0', fontSize: '1.35rem' }}>{primaryAction?.label ?? 'İş akışı tamamlandı'}</h2>
          <p style={{ margin: '0.6rem 0 0', maxWidth: '680px', color: '#e0e7ff', lineHeight: 1.55 }}>
            {primaryAction?.disabledReason || summaryText || 'Projenin güncel durumu backend iş akışı tarafından doğrulandı.'}
          </p>
        </div>
        {primaryAction && <NextActions actions={[primaryAction]} />}
      </section>

      {workflow.blockingReasons.length > 0 && (
        <section aria-label="İş akışı engelleri">
          <BlockingReasons reasons={workflow.blockingReasons} />
        </section>
      )}

      <section>
        <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', alignItems: 'end', marginBottom: '0.9rem' }}>
          <div>
            <h2 style={{ margin: 0, fontSize: '1.05rem' }}>Proje aşamaları</h2>
            <p style={{ margin: '0.35rem 0 0', color: '#64748b', fontSize: '0.8rem' }}>Durumlar backend iş akışı özetinden gelir.</p>
          </div>
          <span style={{ color: '#475569', fontSize: '0.75rem', fontWeight: 700 }}>{workflow.currentStageLabel}</span>
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: '0.9rem' }}>
          {areas.map((area) => (
            <Link key={area.area} to={areaPath(area.area)} style={{ padding: '1rem', color: '#0f172a', background: '#fff', border: '1px solid #e2e8f0', borderRadius: '0.9rem', textDecoration: 'none' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', gap: '0.75rem', alignItems: 'center' }}>
                <strong style={{ fontSize: '0.9rem' }}>{area.label}</strong>
                <span style={{ width: 10, height: 10, flex: '0 0 10px', borderRadius: '50%', background: area.status === 'succeeded' ? '#10b981' : area.status === 'failed' ? '#ef4444' : area.status === 'running' ? '#4f46e5' : '#f59e0b' }} aria-hidden="true" />
              </div>
              <p style={{ margin: '0.6rem 0 0', color: '#475569', fontSize: '0.78rem', fontWeight: 700 }}>{statusLabels[area.status]}</p>
              <p style={{ margin: '0.3rem 0 0', color: '#64748b', fontSize: '0.75rem', lineHeight: 1.45 }}>{area.message}</p>
              {area.current !== undefined && area.total !== undefined && area.total > 0 && (
                <p style={{ margin: '0.55rem 0 0', color: '#3730a3', fontSize: '0.75rem', fontWeight: 800 }}>{area.current} / {area.total}</p>
              )}
            </Link>
          ))}
        </div>
      </section>

      <section>
        <h2 style={{ margin: '0 0 0.9rem', fontSize: '1.05rem' }}>Özet</h2>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(170px, 1fr))', gap: '0.9rem' }}>
          {[
            ['Öğrenci', project?.students?.length ?? 0],
            ['Soru', project?.questions?.length ?? 0],
            ['Onaylanan OCR', `${approvedOcrCount} / ${ocrRecords.length}`],
            ['OCR kontrolü', reviewOcrCount],
            ['Puan kontrolü', scoringReviewCount],
          ].map(([label, value]) => (
            <div key={label} style={{ padding: '1rem', background: '#fff', border: '1px solid #e2e8f0', borderRadius: '0.9rem' }}>
              <div style={{ color: '#64748b', fontSize: '0.75rem' }}>{label}</div>
              <div style={{ marginTop: '0.35rem', fontSize: '1.35rem', fontWeight: 800 }}>{value}</div>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
