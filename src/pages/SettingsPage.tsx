import { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type { GcReport, ModelServerArgsPreview } from '../api/types';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { useProjectContext } from '../state/useProjectContext';
import { Loader2, RefreshCw } from 'lucide-react';
import { canConfirmExternalModel, getModelPrivacyWarning } from './settingsUi';

type SettingsTab = 'general' | 'models' | 'storage' | 'diagnostics';

export function SettingsPage({ defaultTab }: { defaultTab?: SettingsTab }) {
  const { projectId = '' } = useParams<{ projectId: string }>();
  const { projectId: contextProjectId, projectPath } = useProjectContext();
  const activeProjectId = projectId || contextProjectId;
  const queryClient = useQueryClient();

  const [activeTab, setActiveTab] = useState<SettingsTab>(defaultTab || 'general');
  const [error, setError] = useState<AppError | null>(null);
  const [preview, setPreview] = useState<ModelServerArgsPreview | null>(null);
  const [gcReport, setGcReport] = useState<GcReport | null>(null);
  const [externalConsent, setExternalConsent] = useState(false);

  const projectQuery = useQuery({
    queryKey: ['project-snapshot', activeProjectId],
    queryFn: () => commands.getProjectSnapshot(activeProjectId),
    enabled: !!activeProjectId,
  });

  const modelStatusQuery = useQuery({
    queryKey: ['model-status'],
    queryFn: commands.probeModelServer,
  });

  const assignmentsQuery = useQuery({
    queryKey: ['teaching-assignments', activeProjectId],
    queryFn: () => commands.listTeachingAssignments({ projectId: activeProjectId }),
    enabled: !!activeProjectId,
  });

  const refresh = async () => {
    setError(null);
    setPreview(null);
    await modelStatusQuery.refetch();
    await queryClient.invalidateQueries({ queryKey: ['model-status'] });
  };

  const startMutation = useMutation({
    mutationFn: commands.startModelServer,
    onSuccess: async (result) => {
      setError(null);
      setPreview(null);
      await refresh();
      if (!result.started && !result.healthOk) {
        setError({
          code: 'MODEL_PORT_ALREADY_IN_USE',
          safeMessage: result.message,
          recoveryAction: 'Portu serbest bırakıp yeniden deneyin.',
          retryable: true,
          correlationId: crypto.randomUUID(),
          detailsAvailable: false,
        });
      }
    },
    onError: (err: AppError) => setError(err),
  });

  const gcMutation = useMutation({
    mutationFn: async () => {
      const dryRun = await commands.runGenerationGc(activeProjectId, true);
      const report = await commands.runGenerationGc(activeProjectId, false);
      return { dryRun, report };
    },
    onSuccess: ({ report }) => {
      setGcReport(report);
      setError(null);
    },
    onError: (mutationError: unknown) => {
      setError(mutationError as AppError);
    },
  });

  const backupMutation = useMutation({
    mutationFn: () => commands.startBackupJob(activeProjectId),
    onSuccess: () => {
      setError(null);
    },
    onError: (mutationError: unknown) => {
      setError(mutationError as AppError);
    },
  });

  const stopMutation = useMutation({
    mutationFn: commands.stopModelServer,
    onSuccess: async () => {
      setError(null);
      setPreview(null);
      await refresh();
    },
    onError: (err: AppError) => setError(err),
  });

  const externalModelMutation = useMutation({
    mutationFn: commands.enableExternalModel,
    onSuccess: async () => {
      setExternalConsent(false);
      setError(null);
      await refresh();
    },
    onError: (err: AppError) => setError(err),
  });

  const previewMutation = useMutation({
    mutationFn: commands.previewModelServerArgs,
    onSuccess: (data) => {
      setError(null);
      setPreview(data);
    },
    onError: (err: AppError) => setError(err),
  });

  const project = projectQuery.data;
  const status = modelStatusQuery.data;
  const privacyWarning = getModelPrivacyWarning(status);
  const assignments = assignmentsQuery.data ?? [];
  const activeAssignments = assignments.filter((a) => a.isActive);

  // Filter genuine warnings (exclude normal closed state warnings)
  const genuineWarnings = (status?.warnings ?? []).filter((warning) => {
    const lower = warning.toLowerCase();
    return (
      lower.includes('bulunamadı') ||
      lower.includes('hata') ||
      lower.includes('başarısız') ||
      lower.includes('timeout') ||
      lower.includes('in use')
    );
  });

  return (
    <div style={{ padding: '2rem', maxWidth: '1000px', margin: '0 auto', fontFamily: 'system-ui, -apple-system, sans-serif' }}>
      <div style={{ marginBottom: '1.5rem' }}>
        <h2 style={{ fontSize: '1.75rem', fontWeight: 800, color: '#0f172a', margin: 0 }}>Ayarlar</h2>
        <p style={{ color: '#64748b', fontSize: '0.875rem', margin: '0.35rem 0 0' }}>
          Ders alanı tercihlerini, yerel yapay zekâ model durumunu ve depolama ayarlarını yönetin.
        </p>
      </div>

      {error && <ErrorBanner error={error} showTechnicalDetails={false} />}

      <div className="exam-package-tabs" role="tablist" style={{ marginBottom: '1.5rem' }}>
        <button
          type="button"
          data-project-write="false"
          className={activeTab === 'general' ? 'is-active' : ''}
          onClick={() => setActiveTab('general')}
        >
          Genel
        </button>
        <button
          type="button"
          data-project-write="false"
          className={activeTab === 'models' ? 'is-active' : ''}
          onClick={() => setActiveTab('models')}
        >
          Modeller
        </button>
        <button
          type="button"
          data-project-write="false"
          className={activeTab === 'storage' ? 'is-active' : ''}
          onClick={() => setActiveTab('storage')}
        >
          Depolama ve Yedekleme
        </button>
        <button
          type="button"
          data-project-write="false"
          className={activeTab === 'diagnostics' ? 'is-active' : ''}
          onClick={() => setActiveTab('diagnostics')}
        >
          Tanılama
        </button>
      </div>

      {activeTab === 'general' && (
        <section style={{ display: 'grid', gap: '1.25rem' }}>
          <article style={{ padding: '1.5rem', background: '#fff', border: '1px solid #e2e8f0', borderRadius: '1rem' }}>
            <h3 style={{ margin: 0, fontSize: '1.1rem', fontWeight: 700, color: '#0f172a' }}>Ders Alanı Bilgileri</h3>
            <div style={{ marginTop: '1rem', display: 'grid', gap: '0.75rem', fontSize: '0.875rem' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', padding: '0.5rem 0', borderBottom: '1px solid #f1f5f9' }}>
                <span style={{ color: '#64748b' }}>Ders Alanı Adı</span>
                <strong style={{ color: '#0f172a' }}>{project?.name || 'Yükleniyor…'}</strong>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', padding: '0.5rem 0', borderBottom: '1px solid #f1f5f9' }}>
                <span style={{ color: '#64748b' }}>Eğitim Yılı</span>
                <strong style={{ color: '#0f172a' }}>{project?.academicYearId || '2026-2027'}</strong>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', padding: '0.5rem 0' }}>
                <span style={{ color: '#64748b' }}>Aktif Ders–Sınıf Görevlendirmeleri</span>
                <strong style={{ color: '#0f172a' }}>{activeAssignments.length} sınıf</strong>
              </div>
            </div>
            <div style={{ marginTop: '1.25rem' }}>
              <Link to={`/project/${encodeURIComponent(activeProjectId)}/classes`} className="button button--secondary" style={{ fontSize: '0.85rem' }}>
                Ders–sınıf görevlendirmelerini yönet →
              </Link>
            </div>
          </article>
        </section>
      )}

      {activeTab === 'models' && (
        <section style={{ display: 'grid', gap: '1.25rem' }}>
          <article style={{ padding: '1.5rem', background: '#fff', border: '1px solid #e2e8f0', borderRadius: '1rem' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1rem' }}>
              <div>
                <h3 style={{ margin: 0, fontSize: '1.15rem', fontWeight: 700, color: '#0f172a' }}>Yerel yapay zekâ</h3>
                <p style={{ margin: '0.25rem 0 0', color: '#64748b', fontSize: '0.875rem' }}>
                  Sınav kâğıdı OCR ve notlandırma değerlendirmeleri için kullanılır.
                </p>
              </div>
              <span style={{
                padding: '0.3rem 0.75rem',
                borderRadius: '999px',
                fontSize: '0.8rem',
                fontWeight: 700,
                background: status?.draining ? '#fef3c7' : status?.serverRunning ? '#dcfce7' : '#f1f5f9',
                color: status?.draining ? '#92400e' : status?.serverRunning ? '#166534' : '#475569'
              }}>
                {status?.draining ? 'İşlemlerin bitmesi bekleniyor' : status?.serverRunning ? 'Hazır' : 'Kapalı'}
              </span>
            </div>

            {!status?.serverRunning ? (
              <div style={{ padding: '1rem', background: '#f8fafc', borderRadius: '0.75rem', border: '1px solid #e2e8f0', color: '#475569', fontSize: '0.875rem', marginBottom: '1.25rem', lineHeight: 1.5 }}>
                Yerel model şu anda kapalı. Bir OCR veya değerlendirme işlemi başladığında otomatik olarak açılacak.
              </div>
            ) : (
              <div style={{ padding: '1rem', background: '#f0fdf4', borderRadius: '0.75rem', border: '1px solid #bbf7d0', color: '#166534', fontSize: '0.875rem', marginBottom: '1.25rem', lineHeight: 1.5 }}>
                ✓ Yerel model aktif ve OCR / değerlendirme işlemlerine hazır.
                {(status?.activeLeaseCount ?? 0) > 0 && (
                  <><br />{status?.activeLeaseCount} işlem tarafından kullanılıyor.</>
                )}
              </div>
            )}

            {privacyWarning.visible && (
              <div
                role="alert"
                style={{
                  marginBottom: '1.25rem',
                  padding: '1rem',
                  background: '#fff1f2',
                  border: '2px solid #fda4af',
                  borderRadius: '0.75rem',
                  color: '#881337',
                  lineHeight: 1.5,
                }}
              >
                <strong style={{ display: 'block', marginBottom: '0.35rem' }}>
                  {privacyWarning.title}
                </strong>
                {privacyWarning.body}
                <label style={{ display: 'flex', gap: '0.5rem', alignItems: 'flex-start', marginTop: '0.75rem' }}>
                  <input
                    type="checkbox"
                    checked={externalConsent}
                    onChange={(event) => setExternalConsent(event.target.checked)}
                  />
                  <span>Harici modelin öğrenci verisi taşıyan işlemlerde kullanılmasını açıkça onaylıyorum.</span>
                </label>
                <button
                  type="button"
                  data-project-write="false"
                  className="button button--primary"
                  style={{ marginTop: '0.75rem', background: '#be123c', borderColor: '#be123c' }}
                  onClick={() => externalModelMutation.mutate({
                    profileId: status?.profileId,
                    projectRootPath: projectPath || project?.rootPath || null,
                    confirmExternalDataTransfer: externalConsent,
                  })}
                  disabled={!canConfirmExternalModel(externalConsent, externalModelMutation.isPending)}
                >
                  {externalModelMutation.isPending ? 'Onay kaydediliyor…' : 'Harici kullanımı açıkça onayla'}
                </button>
              </div>
            )}

            {genuineWarnings.length > 0 && (
              <div style={{ marginBottom: '1.25rem', padding: '1rem', background: '#fffbeb', border: '1px solid #fef08a', borderRadius: '0.75rem' }}>
                <strong style={{ color: '#92400e', display: 'block', marginBottom: '0.35rem' }}>Müdahale Gerektiren Uyarılar</strong>
                <ul style={{ margin: 0, paddingLeft: '1.25rem', color: '#b45309', fontSize: '0.875rem' }}>
                  {genuineWarnings.map((w, i) => <li key={i}>{w}</li>)}
                </ul>
              </div>
            )}

            <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap' }}>
              {!status?.serverRunning ? (
                <button
                  type="button"
                  data-project-write="false"
                  className="button button--primary"
                  onClick={() => startMutation.mutate(status?.profileId)}
                  disabled={startMutation.isPending}
                >
                  {startMutation.isPending ? 'Başlatılıyor…' : 'Şimdi başlat'}
                </button>
              ) : (
                <button
                  type="button"
                  data-project-write="false"
                  className="button button--secondary"
                  onClick={() => stopMutation.mutate(status?.profileId)}
                  disabled={stopMutation.isPending}
                >
                  {stopMutation.isPending
                    ? 'Durduruluyor…'
                    : (status?.activeLeaseCount ?? 0) > 0
                      ? 'İşlemler bitince durdur'
                      : 'Durdur'}
                </button>
              )}
              <button type="button" data-project-write="false" className="button button--secondary" onClick={() => void refresh()}>
                <RefreshCw size={15} /> Durumu Yenile
              </button>
            </div>
          </article>
        </section>
      )}

      {activeTab === 'storage' && (
        <section style={{ display: 'grid', gap: '1.25rem' }}>
          <article style={{ padding: '1.5rem', background: '#fff', border: '1px solid #e2e8f0', borderRadius: '1rem' }}>
            <h3 style={{ margin: 0, fontSize: '1.1rem', fontWeight: 700, color: '#0f172a' }}>Depolama ve Klasör Konumu</h3>
            <p style={{ margin: '0.35rem 0 1rem', color: '#64748b', fontSize: '0.875rem' }}>
              Bu ders alanının tüm verileri yerel diskinizde güvenle saklanır.
            </p>
            <div style={{ display: 'grid', gap: '0.75rem', fontSize: '0.875rem' }}>
              <div style={{ padding: '0.75rem', background: '#f8fafc', borderRadius: '0.5rem', border: '1px solid #e2e8f0' }}>
                <span style={{ color: '#64748b', fontSize: '0.75rem', display: 'block', fontWeight: 600 }}>Proje Klasör Yolu</span>
                <code style={{ fontSize: '0.85rem', color: '#0f172a', wordBreak: 'break-all' }}>{projectPath || project?.rootPath || '—'}</code>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', padding: '0.5rem 0', borderBottom: '1px solid #f1f5f9' }}>
                <span style={{ color: '#64748b' }}>Kayıtlı Doküman Sayısı</span>
                <strong style={{ color: '#0f172a' }}>{project?.documents?.length ?? 0} dosya</strong>
              </div>
            </div>
          </article>
          <article style={{ padding: '1.5rem', background: '#fff', border: '1px solid #e2e8f0', borderRadius: '1rem' }}>
            <h3 style={{ margin: 0, fontSize: '1.1rem', fontWeight: 700, color: '#0f172a' }}>Yedekleme ve Depolama Temizliği</h3>
            <p style={{ margin: '0.35rem 0 1rem', color: '#64748b', fontSize: '0.875rem' }}>
              Yedek; belgeler, rubrik, OCR ve notlandırma verilerini doğrulanmış tek arşiv olarak kaydeder.
              Temizlik yalnızca referanssız, eski veya başarısız üretimleri siler.
            </p>
            <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap' }}>
              <button
                type="button"
                className="button button--primary"
                data-project-write="false"
                onClick={() => backupMutation.mutate()}
                disabled={backupMutation.isPending || !activeProjectId}
                style={{ fontSize: '0.85rem' }}
              >
                {backupMutation.isPending ? 'Yedekleniyor…' : 'Yedek Oluştur'}
              </button>
              <button
                type="button"
                className="button button--secondary"
                data-project-write="true"
                onClick={() => gcMutation.mutate()}
                disabled={gcMutation.isPending || !activeProjectId}
                style={{ fontSize: '0.85rem' }}
              >
                {gcMutation.isPending ? 'Taranıyor…' : 'Depolamayı Temizle'}
              </button>
            </div>
            {gcReport && (
              <div style={{ marginTop: '1rem', padding: '0.85rem', background: '#f8fafc', border: '1px solid #e2e8f0', borderRadius: '0.75rem', fontSize: '0.85rem' }}>
                <strong style={{ color: '#0f172a' }}>Temizlik sonucu:</strong>{' '}
                {gcReport.deletedGenerations} silindi, {gcReport.deferredCleanup} ertelendi,
                {gcReport.protectedGenerations} korundu, {gcReport.orphanStagingDirs} staging temizlendi.
              </div>
            )}
            {backupMutation.isSuccess && (
              <p style={{ marginTop: '0.75rem', fontSize: '0.85rem', color: '#047857' }}>
                Yedek işi kuyruğa alındı. İş merkezinden ilerlemeyi takip edebilirsiniz.
              </p>
            )}
          </article>
        </section>
      )}

      {activeTab === 'diagnostics' && (
        <section style={{ display: 'grid', gap: '1.25rem' }}>
          <article style={{ padding: '1.5rem', background: '#fff', border: '1px solid #e2e8f0', borderRadius: '1rem' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1rem' }}>
              <h3 style={{ margin: 0, fontSize: '1.1rem', fontWeight: 700, color: '#0f172a' }}>Teknik Ayrıntılar ve Tanılama</h3>
              <button
                type="button"
                data-project-write="false"
                className="button button--secondary"
                onClick={() => previewMutation.mutate(status?.profileId)}
                disabled={previewMutation.isPending}
                style={{ fontSize: '0.8rem' }}
              >
                {previewMutation.isPending ? <Loader2 size={15} className="animate-spin" /> : 'Argüman Önizleme'}
              </button>
            </div>

            <div style={{ display: 'grid', gap: '0.75rem', fontSize: '0.85rem', color: '#334155' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid #f1f5f9', paddingBottom: '0.5rem' }}>
                <span style={{ color: '#64748b' }}>Sistem Tanımlayıcı (ID)</span>
                <span style={{ fontFamily: 'monospace' }}>{activeProjectId}</span>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid #f1f5f9', paddingBottom: '0.5rem' }}>
                <span style={{ color: '#64748b' }}>Sunucu Adresi / Port</span>
                <span style={{ fontFamily: 'monospace' }}>{status?.baseUrl || 'http://127.0.0.1:8080'}</span>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid #f1f5f9', paddingBottom: '0.5rem' }}>
                <span style={{ color: '#64748b' }}>Aktif Model Profili</span>
                <span>{status?.displayName || 'Gemma 4 12B'}</span>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid #f1f5f9', paddingBottom: '0.5rem' }}>
                <span style={{ color: '#64748b' }}>Yönetilen Süreç PID</span>
                <span style={{ fontFamily: 'monospace' }}>{status?.managedProcessPid || 'Yok'}</span>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid #f1f5f9', paddingBottom: '0.5rem' }}>
                <span style={{ color: '#64748b' }}>Log Yolu</span>
                <span style={{ fontFamily: 'monospace', wordBreak: 'break-all' }}>{status?.logPath || 'Yok'}</span>
              </div>
            </div>

            {preview && (
              <div style={{ marginTop: '1.25rem', padding: '1rem', background: '#f8fafc', border: '1px solid #e2e8f0', borderRadius: '0.75rem' }}>
                <strong style={{ fontSize: '0.875rem', color: '#0f172a', display: 'block', marginBottom: '0.5rem' }}>
                  Başlatma Argümanları
                </strong>
                <pre style={{ margin: 0, whiteSpace: 'pre-wrap', fontSize: '0.75rem', fontFamily: 'monospace', color: '#334155' }}>
                  {preview.command} {preview.args.join(' ')}
                </pre>
              </div>
            )}
          </article>
        </section>
      )}
    </div>
  );
}
