import { useEffect, useState } from 'react';
import { useProjectContext } from '../state/useProjectContext';
import { UserCheck, AlertCircle, Save, Loader2, ChevronRight, ScanText } from 'lucide-react';
import { commands } from '../api/commands';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useSearchParams } from 'react-router-dom';
import type { AppError } from '../api/errors';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { tauriClient } from '../api/tauriClient';
import { projectStudentOperationsPath } from '../app/projectRoutes';
import {
  filterStudentSubmissions,
  getSubmissionClassName,
  hasIdentityClassMismatch,
} from './studentOperations';

export function StudentIdentityPage() {
  const [searchParams] = useSearchParams();
  const { projectId, isResolving } = useProjectContext();
  const classId = searchParams.get('classId') || '';
  const batchId = searchParams.get('batchId') || '';
  const queryClient = useQueryClient();

  const { data: project, isLoading: isProjectLoading } = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: !!projectId,
  });

  const [localStates, setLocalStates] = useState<Record<string, { displayName: string, number: string, className: string }>>({});
  const [savingIds, setSavingIds] = useState<Set<string>>(new Set());
  const [ocrStarting, setOcrStarting] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  useEffect(() => {
    if (!projectId) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void tauriClient.listenToJobEvents(() => {
      if (cancelled) return;
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    }).then((cleanup) => {
      unlisten = cleanup;
      if (cancelled) cleanup();
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [projectId, queryClient]);

  if (isResolving || isProjectLoading || !project || !projectId) {
    return (
      <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '16rem', color: '#64748b' }}>
        <Loader2 size={32} className="animate-spin" />
      </div>
    );
  }

  const submissions = filterStudentSubmissions(project.studentSubmissions, classId, batchId);
  if (!submissions || submissions.length === 0) {
    return (
      <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '16rem', color: '#64748b', fontSize: '0.875rem' }}>
        Öğrenci cevabı bulunamadı. Lütfen önce belgeler ekranından bir öğrenci cevap PDF'i yükleyin ve gruplayın.
      </div>
    );
  }

  const getStudent = (studentId: string) => project.students?.find(s => s.id === studentId);

  const getLocalState = (submissionId: string, studentId: string) => {
    if (localStates[submissionId]) return localStates[submissionId];
    const student = getStudent(studentId);
    return {
      displayName: student?.displayName || student?.identityOcr?.displayName || '',
      number: student?.number || student?.identityOcr?.number || '',
      className: student?.className || student?.identityOcr?.className || ''
    };
  };

  const identityReady = (studentId: string) => {
    const student = getStudent(studentId);
    return !!(student?.displayName?.trim() || student?.number?.trim());
  };

  const handleUpdate = (submissionId: string, studentId: string, field: 'displayName' | 'number' | 'className', value: string) => {
    const currentState = getLocalState(submissionId, studentId);
    setLocalStates(prev => ({
      ...prev,
      [submissionId]: { ...currentState, [field]: value }
    }));
  };

  const handleVerify = async (submissionId: string, studentId: string) => {
    const state = getLocalState(submissionId, studentId);
    try {
      setSavingIds(prev => { const next = new Set(prev); next.add(submissionId); return next; });
      await commands.updateStudentIdentity({
        projectId: project.id,
        submissionId: submissionId,
        displayName: state.displayName.trim() || null,
        number: state.number.trim() || null,
        className: state.className.trim() || null
      });
      await queryClient.invalidateQueries({ queryKey: ['project-snapshot', project.id] });
      await queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', project.id] });
      setError(null);
    } catch (saveError) {
      setError(saveError as AppError);
    } finally {
      setSavingIds(prev => { const next = new Set(prev); next.delete(submissionId); return next; });
    }
  };

  const handleStartIdentityOcr = async () => {
    try {
      setError(null);
      setOcrStarting(true);
      await commands.startStudentIdentityOcr({ projectId: project.id });
      await queryClient.invalidateQueries({ queryKey: ['project-snapshot', project.id] });
      await queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', project.id] });
    } catch (err) {
      setError(err as AppError);
    } finally {
      setOcrStarting(false);
    }
  };

  const allVerified = submissions.every(sub => identityReady(sub.studentId));
  const hasIdentityTemplate = !!project.studentIdentityCropTemplate;

  return (
    <div style={{ maxWidth: '64rem', margin: '0 auto', padding: '2rem', display: 'flex', flexDirection: 'column', gap: '1.5rem', fontFamily: 'system-ui, -apple-system, sans-serif' }}>
      
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
        <div>
          <h2 style={{ fontSize: '1.5rem', fontWeight: 700, color: '#0f172a', margin: 0, letterSpacing: '-0.025em' }}>Öğrenci Kimlik Doğrulama</h2>
          <p style={{ fontSize: '0.875rem', color: '#64748b', margin: '0.25rem 0 0 0' }}>Öğrencilerin ad, soyad ve numara bilgilerini kontrol edin ve onaylayın.</p>
        </div>
        <div style={{ display: 'flex', gap: '1rem', alignItems: 'center' }}>
          <Link to={projectStudentOperationsPath(projectId, 'grouping', searchParams.toString())} style={{ color: '#475569', fontSize: '0.875rem', fontWeight: 500, textDecoration: 'none' }}>
            ← Öğrenci Gruplama
          </Link>
          <Link to={projectStudentOperationsPath(projectId, 'crops', searchParams.toString())} style={{ color: '#475569', fontSize: '0.875rem', fontWeight: 500, textDecoration: 'none' }}>
            Kimlik alanını seç
          </Link>
          <Link to={`/project/${encodeURIComponent(projectId)}/overview`} style={{ color: '#4f46e5', fontSize: '0.875rem', fontWeight: 600, textDecoration: 'none', display: 'flex', alignItems: 'center', gap: '0.25rem' }}>
            İş Akışı <ChevronRight size={16} />
          </Link>
        </div>
      </div>

      <div style={{ display: 'flex', gap: '0.75rem', alignItems: 'center', flexWrap: 'wrap' }}>
        <button
          type="button"
          data-project-write="true"
          onClick={handleStartIdentityOcr}
          disabled={!hasIdentityTemplate || ocrStarting}
          title={!hasIdentityTemplate ? 'Önce Crop Şablonu sayfasında kimlik alanını seçin.' : undefined}
          style={{ display: 'inline-flex', alignItems: 'center', gap: '0.5rem', padding: '0.625rem 0.9rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1', background: (!hasIdentityTemplate || ocrStarting) ? '#e2e8f0' : '#0f172a', color: (!hasIdentityTemplate || ocrStarting) ? '#64748b' : 'white', fontWeight: 600, cursor: (!hasIdentityTemplate || ocrStarting) ? 'not-allowed' : 'pointer' }}
        >
          {ocrStarting ? <Loader2 size={16} className="animate-spin" /> : <ScanText size={16} />}
          Kimlik OCR’ını Başlat
        </button>
        {!hasIdentityTemplate && <span style={{ color: '#b45309', fontSize: '0.875rem' }}>Önce Crop Şablonu sayfasında kimlik alanını seçin.</span>}
      </div>

      {!allVerified && (
        <div style={{ padding: '0.75rem 1rem', background: '#fffbeb', border: '1px solid #fde68a', borderRadius: '0.5rem', color: '#d97706', fontSize: '0.875rem', display: 'flex', alignItems: 'center', gap: '0.5rem', alignSelf: 'flex-start', fontWeight: 500 }}>
          <AlertCircle size={18} /> Eksik kimlikler var
        </div>
      )}

      {error && <ErrorBanner error={error} />}

      {/* Table Container */}
      <div style={{ background: 'white', border: '1px solid #e2e8f0', borderRadius: '1rem', overflow: 'hidden', boxShadow: '0 1px 2px 0 rgba(0,0,0,0.05)' }}>
        <div style={{ overflowX: 'auto' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse', textAlign: 'left', minWidth: '800px' }}>
            <thead>
              <tr style={{ background: '#f8fafc', borderBottom: '1px solid #e2e8f0' }}>
                <th style={{ padding: '1rem', fontSize: '0.75rem', fontWeight: 600, color: '#64748b', textTransform: 'uppercase', letterSpacing: '0.05em', width: '120px' }}>Öğrenci Sıra</th>
                <th style={{ padding: '1rem', fontSize: '0.75rem', fontWeight: 600, color: '#64748b', textTransform: 'uppercase', letterSpacing: '0.05em', minWidth: '200px' }}>Ad Soyad</th>
                <th style={{ padding: '1rem', fontSize: '0.75rem', fontWeight: 600, color: '#64748b', textTransform: 'uppercase', letterSpacing: '0.05em', width: '150px' }}>Okul No</th>
                <th style={{ padding: '1rem', fontSize: '0.75rem', fontWeight: 600, color: '#64748b', textTransform: 'uppercase', letterSpacing: '0.05em', minWidth: '220px' }}>Sınıf / kağıt sinyali</th>
                <th style={{ padding: '1rem', fontSize: '0.75rem', fontWeight: 600, color: '#64748b', textTransform: 'uppercase', letterSpacing: '0.05em', width: '140px', textAlign: 'center' }}>Durum</th>
                <th style={{ padding: '1rem', fontSize: '0.75rem', fontWeight: 600, color: '#64748b', textTransform: 'uppercase', letterSpacing: '0.05em', width: '120px', textAlign: 'right' }}>İşlem</th>
              </tr>
            </thead>
            <tbody style={{ fontSize: '0.875rem' }}>
              {submissions.map((sub, idx) => {
                const isVerified = identityReady(sub.studentId);
                const state = getLocalState(sub.id, sub.studentId);
                const isSaving = savingIds.has(sub.id);
                const student = getStudent(sub.studentId);
                const canonicalClassName = getSubmissionClassName(project, sub);
                const detectedClassName = state.className.trim();
                const classMismatch = hasIdentityClassMismatch(project, sub, detectedClassName);
                // Can verify if at least name or number is provided
                const canVerify = !!(state.displayName.trim() || state.number.trim());

                return (
                  <tr key={sub.id} style={{ borderBottom: '1px solid #f1f5f9', background: isVerified ? '#f8fafc' : 'white' }}>
                    <td style={{ padding: '1rem', fontWeight: 500, color: '#0f172a' }}>Öğrenci {idx + 1}</td>
                    <td style={{ padding: '1rem' }}>
                      <div style={{ marginBottom: '0.4rem', color: '#0f172a', fontWeight: 700 }}>{canonicalClassName}</div>
                      <div style={{ marginBottom: '0.4rem', color: '#64748b', fontSize: '0.72rem' }}>Kaynak: öğrenci PDF paketi</div>
                      <input 
                        type="text" 
                        value={state.displayName}
                        onChange={(e) => handleUpdate(sub.id, sub.studentId, 'displayName', e.target.value)}
                        placeholder="Ad Soyad"
                        style={{ width: '100%', padding: '0.5rem 0.75rem', border: '1px solid #cbd5e1', borderRadius: '0.375rem', outline: 'none', fontSize: '0.875rem' }}
                        onFocus={(e) => e.target.style.borderColor = '#4f46e5'}
                        onBlur={(e) => e.target.style.borderColor = '#cbd5e1'}
                      />
                    </td>
                    <td style={{ padding: '1rem' }}>
                      <input 
                        type="text" 
                        value={state.number}
                        onChange={(e) => handleUpdate(sub.id, sub.studentId, 'number', e.target.value)}
                        placeholder="No"
                        style={{ width: '100%', padding: '0.5rem 0.75rem', border: '1px solid #cbd5e1', borderRadius: '0.375rem', outline: 'none', fontSize: '0.875rem' }}
                        onFocus={(e) => e.target.style.borderColor = '#4f46e5'}
                        onBlur={(e) => e.target.style.borderColor = '#cbd5e1'}
                      />
                    </td>
                    <td style={{ padding: '1rem' }}>
                      <input 
                        type="text" 
                        value={state.className}
                        onChange={(e) => handleUpdate(sub.id, sub.studentId, 'className', e.target.value)}
                        placeholder="Kağıtta okunan sınıf (opsiyonel)"
                        style={{ width: '100%', padding: '0.5rem 0.75rem', border: '1px solid #cbd5e1', borderRadius: '0.375rem', outline: 'none', fontSize: '0.875rem' }}
                        onFocus={(e) => e.target.style.borderColor = '#4f46e5'}
                        onBlur={(e) => e.target.style.borderColor = '#cbd5e1'}
                      />
                      {classMismatch && (
                        <div role="status" style={{ marginTop: '0.4rem', color: '#b45309', fontSize: '0.72rem', lineHeight: 1.35 }}>
                          Kağıtta okunan sınıf bilgisi, yüklenen PDF paketinin sınıfıyla uyuşmuyor. Paket sınıfı korunur.
                        </div>
                      )}
                    </td>
                    <td style={{ padding: '1rem', textAlign: 'center' }}>
                      {isVerified ? (
                        <span style={{ display: 'inline-flex', alignItems: 'center', gap: '0.25rem', fontSize: '0.75rem', fontWeight: 600, color: '#15803d', background: '#dcfce7', padding: '0.25rem 0.5rem', borderRadius: '9999px', border: '1px solid #bbf7d0' }}>
                          <UserCheck size={14} /> Doğrulandı
                        </span>
                      ) : (
                        <span style={{ display: 'inline-flex', alignItems: 'center', gap: '0.25rem', fontSize: '0.75rem', fontWeight: 600, color: '#b45309', background: '#fef3c7', padding: '0.25rem 0.5rem', borderRadius: '9999px', border: '1px solid #fde68a' }}>
                          <AlertCircle size={14} /> Eksik / kontrol gerekli
                        </span>
                      )}
                      {student?.identityOcr && (
                        <details style={{ marginTop: '0.5rem', textAlign: 'left', color: '#475569' }}>
                          <summary style={{ cursor: 'pointer', fontSize: '0.75rem' }}>OCR detayı</summary>
                          <pre style={{ whiteSpace: 'pre-wrap', fontSize: '0.7rem', maxWidth: '16rem' }}>{JSON.stringify(student.identityOcr, null, 2)}</pre>
                        </details>
                      )}
                    </td>
                    <td style={{ padding: '1rem', textAlign: 'right' }}>
                      <button
                        data-project-write="true"
                        onClick={() => handleVerify(sub.id, sub.studentId)}
                        disabled={!canVerify || isSaving}
                        style={{ 
                          display: 'inline-flex', 
                          alignItems: 'center', 
                          justifyContent: 'center', 
                          gap: '0.375rem', 
                          fontSize: '0.75rem', 
                          background: (!canVerify || isSaving) ? '#cbd5e1' : '#4f46e5', 
                          color: (!canVerify || isSaving) ? '#64748b' : 'white', 
                          padding: '0.5rem 1rem', 
                          borderRadius: '0.375rem', 
                          border: 'none', 
                          fontWeight: 600, 
                          cursor: (!canVerify || isSaving) ? 'not-allowed' : 'pointer',
                          minWidth: '100px'
                        }}
                      >
                        {isSaving ? <Loader2 size={14} className="animate-spin" /> : <><Save size={14} /> {isVerified ? 'Güncelle' : 'Doğrula'}</>}
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
      
      {allVerified && (
         <div style={{ padding: '1.25rem', background: '#f0fdf4', border: '1px solid #bbf7d0', borderRadius: '1rem', display: 'flex', alignItems: 'flex-start', gap: '1rem', boxShadow: '0 1px 2px 0 rgba(0,0,0,0.05)' }}>
            <UserCheck size={24} color="#166534" style={{ marginTop: '0.125rem', flexShrink: 0 }} />
            <div>
               <p style={{ margin: '0 0 0.25rem 0', fontWeight: 600, fontSize: '1rem', color: '#166534' }}>Tüm kimlikler doğrulandı!</p>
               <p style={{ margin: 0, fontSize: '0.875rem', color: '#15803d' }}>Öğrenci kimlikleri başarıyla sisteme kaydedildi. İş akışı blokajı kaldırıldı.</p>
            </div>
         </div>
      )}
      
      {!allVerified && (
         <div style={{ padding: '1.25rem', background: '#fef2f2', border: '1px solid #fecaca', borderRadius: '1rem', display: 'flex', alignItems: 'flex-start', gap: '1rem', boxShadow: '0 1px 2px 0 rgba(0,0,0,0.05)' }}>
            <AlertCircle size={24} color="#991b1b" style={{ marginTop: '0.125rem', flexShrink: 0 }} />
            <div>
               <p style={{ margin: '0 0 0.25rem 0', fontWeight: 600, fontSize: '1rem', color: '#991b1b' }}>Notlandırmadan önce öğrenci kimlikleri doğrulanmalı.</p>
               <p style={{ margin: 0, fontSize: '0.875rem', color: '#b91c1c' }}>Lütfen listedeki öğrencilerin en az ad soyad veya okul numarasını girip "Doğrula" butonuna basın.</p>
            </div>
         </div>
      )}
    </div>
  );
}
