import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type { ModelServerArgsPreview } from '../api/types';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { Server, CheckCircle2, AlertCircle, FileCode2, Terminal, Play, Square, RefreshCcw, Settings, FileSearch, Loader2, ArrowRight } from 'lucide-react';

export function ModelStatusPage() {
  const queryClient = useQueryClient();
  const [error, setError] = useState<AppError | null>(null);
  const [preview, setPreview] = useState<ModelServerArgsPreview | null>(null);
  const [detailsExpanded, setDetailsExpanded] = useState(false);

  const { data: status, isLoading, refetch } = useQuery({
    queryKey: ['model-status'],
    queryFn: commands.probeModelServer,
  });

  const startDisabledReason = useMemo(() => {
    if (!status) return undefined;
    return status.startDisabledReason || undefined;
  }, [status]);

  const stopDisabledReason = useMemo(() => {
    if (!status) return undefined;
    if (!status.canStopFromApp) {
      return 'Bu süreç RubrikaV3 tarafından başlatılmadı.';
    }
    return undefined;
  }, [status]);

  const refresh = async () => {
    setError(null);
    setPreview(null);
    await refetch();
    await queryClient.invalidateQueries({ queryKey: ['model-status'] });
  };

  const setModeMutation = useMutation({
    mutationFn: commands.setModelMode,
    onSuccess: async (newStatus) => {
      setError(null);
      setPreview(null);
      queryClient.setQueryData(['model-status'], newStatus);
      await queryClient.invalidateQueries({ queryKey: ['model-status'] });
    },
    onError: (err: AppError) => setError(err),
  });

  const startMutation = useMutation({
    mutationFn: commands.startModelServer,
    onSuccess: async (result) => {
      setError(null);
      setPreview(null);
      await refresh();
      if (!result.started && !result.healthOk) {
        setError({
          code: 'MODEL_PORT_ALREADY_IN_USE',
          message: result.message,
          recoverable: true,
          correlationId: crypto.randomUUID(),
        });
      }
    },
    onError: (err: AppError) => setError(err),
  });

  const switchToManagedAndStartMutation = useMutation({
    mutationFn: async (profileId: string | undefined) => {
      await commands.setModelMode({ profileId, mode: 'managed' });
      return await commands.startModelServer();
    },
    onSuccess: async (result) => {
      setError(null);
      setPreview(null);
      await refresh();
      if (!result.started && !result.healthOk) {
        setError({
          code: 'MODEL_PORT_ALREADY_IN_USE',
          message: result.message,
          recoverable: true,
          correlationId: crypto.randomUUID(),
        });
      }
    },
    onError: (err: AppError) => setError(err),
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

  const previewMutation = useMutation({
    mutationFn: commands.previewModelServerArgs,
    onSuccess: (data) => {
      setError(null);
      setPreview(data);
    },
    onError: (err: AppError) => setError(err),
  });

  const resetMutation = useMutation({
    mutationFn: commands.resetModelProfile,
    onSuccess: async (newStatus) => {
      setError(null);
      setPreview(null);
      queryClient.setQueryData(['model-status'], newStatus);
      await queryClient.invalidateQueries({ queryKey: ['model-status'] });
    },
    onError: (err: AppError) => setError(err),
  });

  const currentStatus = status;
  
  // Custom button styling for the modern look
  const btnStyle = (disabled: boolean, isPrimary: boolean = false) => ({
    display: 'flex', alignItems: 'center', gap: '0.5rem', padding: '0.625rem 1rem', 
    borderRadius: '0.5rem', fontSize: '0.875rem', fontWeight: 500, cursor: disabled ? 'not-allowed' : 'pointer',
    background: disabled ? '#f1f5f9' : isPrimary ? '#4f46e5' : 'white',
    color: disabled ? '#94a3b8' : isPrimary ? 'white' : '#475569',
    border: `1px solid ${disabled ? '#e2e8f0' : isPrimary ? '#4338ca' : '#cbd5e1'}`,
    opacity: disabled ? 0.7 : 1,
    transition: 'all 0.2s',
  });

  return (
    <div style={{ padding: '2rem', maxWidth: '56rem', margin: '0 auto', fontFamily: 'system-ui, -apple-system, sans-serif' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginBottom: '1.5rem', fontSize: '0.875rem' }}>
        <Link to={`/`} style={{ color: '#64748b', textDecoration: 'none' }}>Ana Sayfa</Link>
        <span style={{ color: '#cbd5e1' }}>/</span>
        <span style={{ color: '#0f172a', fontWeight: 500 }}>Model Durumu</span>
      </div>

      <div style={{ marginBottom: '2rem' }}>
        <h2 style={{ fontSize: '1.5rem', fontWeight: 700, color: '#0f172a', margin: 0, letterSpacing: '-0.025em' }}>Model Durumu</h2>
        <p style={{ fontSize: '0.875rem', color: '#64748b', margin: '0.25rem 0 0 0' }}>Llama sunucusunun ve yerel modellerin çalışma durumunu kontrol edin.</p>
      </div>

      {error && (
        <div style={{ marginBottom: '1.5rem' }}>
          <ErrorBanner error={error} showTechnicalDetails={false} />
        </div>
      )}

      {isLoading && (
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', color: '#64748b', padding: '2rem', justifyContent: 'center' }}>
          <Loader2 className="animate-spin" size={20} /> Durum yükleniyor...
        </div>
      )}

      {currentStatus && !isLoading && (
        <>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))', gap: '1.5rem', marginBottom: '1.5rem' }}>
            {/* Server Status Panel */}
            <div style={{ background: 'white', border: '1px solid #e2e8f0', borderRadius: '1rem', overflow: 'hidden', boxShadow: '0 1px 2px 0 rgba(0, 0, 0, 0.05)' }}>
              <div style={{ padding: '1.25rem', borderBottom: '1px solid #f1f5f9', background: '#f8fafc', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
                  <div style={{ padding: '0.5rem', borderRadius: '0.75rem', background: currentStatus.serverRunning ? '#d1fae5' : '#f1f5f9', color: currentStatus.serverRunning ? '#059669' : '#64748b' }}>
                    <Server size={20} />
                  </div>
                  <h3 style={{ margin: 0, fontSize: '1rem', fontWeight: 600, color: '#0f172a' }}>Sunucu Durumu</h3>
                </div>
                {currentStatus.serverRunning ? (
                  <span style={{ display: 'flex', alignItems: 'center', gap: '0.375rem', fontSize: '0.75rem', fontWeight: 600, color: '#047857', background: '#d1fae5', padding: '0.25rem 0.625rem', borderRadius: '9999px', border: '1px solid #a7f3d0' }}>
                    <div style={{ width: '0.5rem', height: '0.5rem', borderRadius: '9999px', background: '#10b981', animation: 'pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite' }}></div>
                    Aktif
                  </span>
                ) : (
                  <span style={{ display: 'flex', alignItems: 'center', gap: '0.375rem', fontSize: '0.75rem', fontWeight: 600, color: '#475569', background: '#f1f5f9', padding: '0.25rem 0.625rem', borderRadius: '9999px', border: '1px solid #e2e8f0' }}>
                    <div style={{ width: '0.5rem', height: '0.5rem', borderRadius: '9999px', background: '#94a3b8' }}></div>
                    Kapalı
                  </span>
                )}
              </div>
              <div style={{ padding: '1.25rem', display: 'flex', flexDirection: 'column', gap: '1rem' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: '0.875rem' }}>
                  <span style={{ color: '#64748b' }}>Adres / Port</span>
                  <span style={{ fontFamily: 'monospace', color: '#0f172a', fontWeight: 500 }}>{currentStatus.baseUrl || '-'}</span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: '0.875rem' }}>
                  <span style={{ color: '#64748b' }}>Health Check</span>
                  {currentStatus.healthOk ? (
                    <span style={{ color: '#059669', display: 'flex', alignItems: 'center', gap: '0.25rem', fontWeight: 500 }}><CheckCircle2 size={16} /> OK</span>
                  ) : (
                    <span style={{ color: '#d97706', display: 'flex', alignItems: 'center', gap: '0.25rem', fontWeight: 500 }}><AlertCircle size={16} /> Yanıt Yok</span>
                  )}
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: '0.875rem' }}>
                  <span style={{ color: '#64748b' }}>Yönetim Modu</span>
                  <span style={{ color: '#0f172a', fontWeight: 500 }}>{currentStatus.startedByApp ? 'RubrikaV3' : 'Harici'}</span>
                </div>

                {!currentStatus.serverRunning && currentStatus.modelPathExists && currentStatus.serverPathExists && (
                   <div style={{ marginTop: '0.5rem', padding: '0.75rem', background: '#eef2ff', border: '1px solid #e0e7ff', borderRadius: '0.5rem', fontSize: '0.75rem', color: '#3730a3', lineHeight: 1.5 }}>
                     Model sunucusu şu an kapalı. <strong>OCR işlemi başlatıldığında otomatik olarak ayağa kaldırılacaktır.</strong>
                   </div>
                )}
                {currentStatus.serverRunning && currentStatus.healthOk && (
                   <div style={{ marginTop: '0.5rem', padding: '0.75rem', background: '#f0fdf4', border: '1px solid #dcfce7', borderRadius: '0.5rem', fontSize: '0.75rem', color: '#166534', lineHeight: 1.5, display: 'flex', gap: '0.35rem' }}>
                     <CheckCircle2 size={14} style={{ flexShrink: 0, marginTop: '1px' }} />
                     <strong>Model hazır. OCR için kullanılabilir.</strong>
                   </div>
                )}
              </div>
            </div>

            {/* Model Files Panel */}
            <div style={{ background: 'white', border: '1px solid #e2e8f0', borderRadius: '1rem', overflow: 'hidden', boxShadow: '0 1px 2px 0 rgba(0, 0, 0, 0.05)' }}>
              <div style={{ padding: '1.25rem', borderBottom: '1px solid #f1f5f9', background: '#f8fafc', display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
                 <div style={{ padding: '0.5rem', borderRadius: '0.75rem', background: '#e0e7ff', color: '#4f46e5' }}>
                   <FileCode2 size={20} />
                 </div>
                 <h3 style={{ margin: 0, fontSize: '1rem', fontWeight: 600, color: '#0f172a' }}>Model Dosyaları</h3>
              </div>
              <div style={{ padding: '1.25rem', display: 'flex', flexDirection: 'column', gap: '1rem' }}>
                <div style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem', fontSize: '0.875rem' }}>
                  <span style={{ color: '#64748b', fontSize: '0.75rem', textTransform: 'uppercase', letterSpacing: '0.05em', fontWeight: 600 }}>Aktif Profil</span>
                  <span style={{ fontWeight: 500, color: '#0f172a' }}>{currentStatus.displayName}</span>
                </div>
                
                <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', fontSize: '0.875rem', marginTop: '0.5rem' }}>
                   {currentStatus.modelPathExists && currentStatus.mmprojPathExists ? (
                     <CheckCircle2 size={16} color="#10b981" style={{ flexShrink: 0 }} />
                   ) : (
                     <AlertCircle size={16} color="#ef4444" style={{ flexShrink: 0 }} />
                   )}
                   <span style={{ color: '#334155' }}>
                     {currentStatus.modelPathExists && currentStatus.mmprojPathExists 
                        ? 'Dosya yolları doğrulandı (GGUF & MMPROJ)' 
                        : 'Model (GGUF) veya MMPROJ dosyası bulunamadı.'}
                   </span>
                </div>
                
                <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', fontSize: '0.875rem' }}>
                   {currentStatus.serverPathExists ? (
                     <CheckCircle2 size={16} color="#10b981" style={{ flexShrink: 0 }} />
                   ) : (
                     <AlertCircle size={16} color="#ef4444" style={{ flexShrink: 0 }} />
                   )}
                   <span style={{ color: '#334155' }}>
                     {currentStatus.serverPathExists ? 'llama-server binary bulundu' : 'llama-server binary bulunamadı'}
                   </span>
                </div>
              </div>
            </div>
          </div>
          
          {/* Action Bar */}
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.75rem', marginBottom: '2rem', padding: '1rem', background: '#f8fafc', border: '1px solid #e2e8f0', borderRadius: '1rem' }}>
            {currentStatus.mode === 'external' && currentStatus.canStartFromApp && (
              <button
                onClick={() => switchToManagedAndStartMutation.mutate(currentStatus.profileId)}
                disabled={switchToManagedAndStartMutation.isPending || setModeMutation.isPending || !!startDisabledReason}
                style={btnStyle(switchToManagedAndStartMutation.isPending || setModeMutation.isPending || !!startDisabledReason, true)}
                title={startDisabledReason}
              >
                {switchToManagedAndStartMutation.isPending ? <Loader2 size={16} className="animate-spin" /> : <Play size={16} />}
                Yönetilen moda al ve başlat
              </button>
            )}

            {currentStatus.mode === 'managed' && currentStatus.canStartFromApp && (
              <button
                onClick={() => startMutation.mutate(currentStatus.profileId)}
                disabled={startMutation.isPending || !!startDisabledReason}
                style={btnStyle(startMutation.isPending || !!startDisabledReason, true)}
                title={startDisabledReason}
              >
                {startMutation.isPending ? <Loader2 size={16} className="animate-spin" /> : <Play size={16} />}
                Modeli Başlat
              </button>
            )}

            {currentStatus.canStopFromApp && (
              <button
                onClick={() => stopMutation.mutate(currentStatus.profileId)}
                disabled={stopMutation.isPending || !!stopDisabledReason}
                style={btnStyle(stopMutation.isPending || !!stopDisabledReason, false)}
                title={stopDisabledReason}
              >
                {stopMutation.isPending ? <Loader2 size={16} className="animate-spin" /> : <Square size={16} />}
                Durdur
              </button>
            )}

            {currentStatus.mode === 'managed' && (
              <button
                onClick={() => setModeMutation.mutate({ profileId: currentStatus.profileId, mode: 'external' })}
                disabled={setModeMutation.isPending}
                style={btnStyle(setModeMutation.isPending, false)}
              >
                {setModeMutation.isPending ? <Loader2 size={16} className="animate-spin" /> : <ArrowRight size={16} />}
                Harici Moda Al
              </button>
            )}
            
            {currentStatus.mode === 'external' && (
              <button
                onClick={() => setModeMutation.mutate({ profileId: currentStatus.profileId, mode: 'managed' })}
                disabled={setModeMutation.isPending}
                style={btnStyle(setModeMutation.isPending, false)}
              >
                {setModeMutation.isPending ? <Loader2 size={16} className="animate-spin" /> : <ArrowRight size={16} />}
                Yönetilen Moda Al
              </button>
            )}

            <button onClick={refresh} style={btnStyle(false, false)}>
              <RefreshCcw size={16} /> Durumu Yenile
            </button>

            <button
              onClick={() => previewMutation.mutate(currentStatus.profileId)}
              disabled={previewMutation.isPending}
              style={btnStyle(previewMutation.isPending, false)}
            >
              {previewMutation.isPending ? <Loader2 size={16} className="animate-spin" /> : <FileSearch size={16} />}
              Argümanları Önizle
            </button>

            <button
              onClick={() => resetMutation.mutate()}
              disabled={resetMutation.isPending}
              style={btnStyle(resetMutation.isPending, false)}
            >
              {resetMutation.isPending ? <Loader2 size={16} className="animate-spin" /> : <Settings size={16} />}
              Varsayılana Dön
            </button>
          </div>

          {currentStatus.warnings.length > 0 && (
            <div style={{ marginBottom: '1.5rem', padding: '1rem', background: '#fffbeb', border: '1px solid #fef08a', borderRadius: '0.75rem' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', color: '#92400e', fontWeight: 600, marginBottom: '0.5rem' }}>
                <AlertCircle size={18} /> Uyarılar
              </div>
              <ul style={{ margin: 0, paddingLeft: '1.5rem', color: '#b45309', fontSize: '0.875rem' }}>
                {currentStatus.warnings.map((warning, i) => (
                  <li key={i}>{warning}</li>
                ))}
              </ul>
            </div>
          )}

          {currentStatus.lastError && (
            <div style={{ marginBottom: '1.5rem', padding: '1rem', background: '#fef2f2', border: '1px solid #fecaca', borderRadius: '0.75rem' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', color: '#b91c1c', fontWeight: 600, marginBottom: '0.5rem' }}>
                <AlertCircle size={18} /> Son Hata
              </div>
              <p style={{ margin: 0, color: '#991b1b', fontSize: '0.875rem', fontWeight: 500 }}>{currentStatus.lastError.message}</p>
              {currentStatus.lastError.suggestedAction && (
                <p style={{ margin: '0.25rem 0 0 0', color: '#b91c1c', fontSize: '0.875rem' }}>Öneri: {currentStatus.lastError.suggestedAction}</p>
              )}
            </div>
          )}

          {/* Technical Details Panel */}
          <div style={{ background: '#0f172a', borderRadius: '1rem', overflow: 'hidden', border: '1px solid #1e293b', boxShadow: '0 4px 6px -1px rgba(0, 0, 0, 0.1)' }}>
            <div 
              style={{ background: '#1e293b', padding: '0.75rem 1.25rem', borderBottom: '1px solid #334155', display: 'flex', alignItems: 'center', justifyContent: 'space-between', cursor: 'pointer', userSelect: 'none' }}
              onClick={() => setDetailsExpanded(!detailsExpanded)}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                 <Terminal size={16} color="#94a3b8" />
                 <span style={{ fontSize: '0.75rem', fontFamily: 'monospace', color: '#cbd5e1' }}>Teknik Detaylar (Diagnostik)</span>
              </div>
              <span style={{ fontSize: '0.75rem', color: '#64748b' }}>{detailsExpanded ? 'Gizle' : 'Göster'}</span>
            </div>
            {detailsExpanded && (
              <div style={{ padding: '1rem', fontFamily: 'monospace', fontSize: '0.75rem', color: '#34d399', maxHeight: '16rem', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '0.25rem', opacity: 0.9 }}>
                 <p style={{ margin: 0 }}>&gt; PID: {currentStatus.managedProcessPid || 'Yok'}</p>
                 <p style={{ margin: 0 }}>&gt; Completion Probe: {currentStatus.completionProbeOk ? 'OK' : 'Failed'}</p>
                 <p style={{ margin: 0 }}>&gt; Log Path: {currentStatus.logPath || 'Yok'}</p>
                 {currentStatus.lastError?.technicalDetails && (
                   <p style={{ margin: '0.5rem 0 0 0', color: '#f87171' }}>&gt; Error Details:<br/>{currentStatus.lastError.technicalDetails}</p>
                 )}
                 <p style={{ margin: '0.5rem 0 0 0', color: '#64748b' }}>&gt; Raw status object available in DevTools.</p>
              </div>
            )}
          </div>

          {preview && (
            <div style={{ marginTop: '1.5rem', padding: '1.25rem', background: 'white', border: '1px solid #e2e8f0', borderRadius: '1rem' }}>
              <h2 style={{ marginTop: 0, fontSize: '1.125rem', fontWeight: 600, color: '#0f172a', marginBottom: '1rem' }}>Argüman Önizleme</h2>
              <div style={{ display: 'grid', gap: '0.75rem', fontSize: '0.875rem' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <strong style={{ color: '#64748b' }}>Komut</strong>
                  <span style={{ color: '#0f172a' }}>{preview.command}</span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <strong style={{ color: '#64748b' }}>Desteklenen</strong>
                  <span style={{ color: '#0f172a' }}>{preview.supportedFlags.join(', ') || '-'}</span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <strong style={{ color: '#64748b' }}>Desteklenmeyen</strong>
                  <span style={{ color: '#ef4444' }}>{preview.unsupportedFlags.join(', ') || '-'}</span>
                </div>
              </div>
              <pre style={{ marginTop: '1rem', whiteSpace: 'pre-wrap', overflowX: 'auto', background: '#f1f5f9', padding: '1rem', borderRadius: '0.5rem', fontSize: '0.75rem', fontFamily: 'monospace', color: '#334155' }}>
                {preview.command} {preview.args.map((arg) => JSON.stringify(arg)).join(' ')}
              </pre>
            </div>
          )}
        </>
      )}
    </div>
  );
}
