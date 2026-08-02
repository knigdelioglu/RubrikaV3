import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { open } from '@tauri-apps/plugin-dialog';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { LoadingButton } from '../components/common/LoadingButton';
import { setActiveProject } from '../state/projectSession';
import { projectOverviewPath } from '../app/projectRoutes';
import {
  DEFAULT_COURSE_ID,
  DEFAULT_COURSE_NAME,
  getDefaultAcademicYear,
  getDefaultProjectPathQueryConfig,
} from './projectCreateUi';

export function ProjectCreatePage() {
  const [name, setName] = useState('');
  const [rootPath, setRootPath] = useState('');
  const [academicYearId, setAcademicYearId] = useState(() => getDefaultAcademicYear());
  const [courseId, setCourseId] = useState(DEFAULT_COURSE_ID);
  const [courseName, setCourseName] = useState(DEFAULT_COURSE_NAME);
  const [pathTouchedByUser, setPathTouchedByUser] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const navigate = useNavigate();
  const defaultProjectPathQuery = getDefaultProjectPathQueryConfig(name, academicYearId);

  const { data: defaultPath, isFetching: isDefaultPathFetching } = useQuery({
    queryKey: defaultProjectPathQuery.queryKey,
    queryFn: () => commands.getDefaultProjectPath(name, academicYearId),
    enabled: defaultProjectPathQuery.enabled,
  });

  useEffect(() => {
    if (!pathTouchedByUser && defaultPath?.path) {
      setRootPath(defaultPath.path);
    }
  }, [defaultPath?.path, pathTouchedByUser]);

  const handleNameChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setName(e.target.value);
    if (!pathTouchedByUser) {
      setRootPath('');
    }
  };

  const handleAcademicYearChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setAcademicYearId(e.target.value);
    if (!pathTouchedByUser) {
      setRootPath('');
    }
  };

  const handlePathChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setRootPath(e.target.value);
    setPathTouchedByUser(true);
  };

  const handleSelectFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      if (selected && typeof selected === 'string') {
        setRootPath(selected);
        setPathTouchedByUser(true);
      }
    } catch (e) {
      console.error(e);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !rootPath.trim()) return;

    setIsLoading(true);
    setError(null);
    try {
      const result = await commands.createProject({ name, rootPath, academicYearId, courseId, courseName });
      setActiveProject(result.project.id, result.projectPath);
      navigate(projectOverviewPath(result.project.id));
    } catch (err) {
      setError(err as unknown as AppError);
    } finally {
      setIsLoading(false);
    }
  };

  let disabledReason: string | undefined = undefined;
  if (!name.trim()) disabledReason = 'Proje adı boş olamaz';
  else if (!rootPath.trim()) disabledReason = 'Proje klasörü seçilmedi';
  else if (!academicYearId.trim() || !courseId.trim() || !courseName.trim()) disabledReason = 'Eğitim yılı ve ders bilgileri zorunludur';
  else if (!pathTouchedByUser && isDefaultPathFetching) disabledReason = 'Varsayılan proje klasörü hazırlanıyor';

  return (
    <div style={{ padding: '2rem', fontFamily: 'system-ui, -apple-system, sans-serif', maxWidth: '800px', margin: '2rem auto' }}>
      <button 
        onClick={() => navigate('/')}
        style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', fontSize: '0.875rem', color: '#64748b', background: 'none', border: 'none', cursor: 'pointer', padding: 0, marginBottom: '1.5rem', transition: 'color 0.2s' }}
        onMouseOver={(e) => (e.currentTarget.style.color = '#0f172a')}
        onMouseOut={(e) => (e.currentTarget.style.color = '#64748b')}
      >
        <span>⬅️</span> Geri Dön
      </button>

      <div style={{ background: 'white', border: '1px solid #e2e8f0', borderRadius: '1rem', boxShadow: '0 1px 2px 0 rgba(0, 0, 0, 0.05)', overflow: 'hidden' }}>
        <div style={{ padding: '1.5rem', borderBottom: '1px solid #f1f5f9', background: '#f8fafc' }}>
          <h2 style={{ fontSize: '1.25rem', fontWeight: 700, color: '#0f172a', margin: 0 }}>Yeni Proje Oluştur</h2>
          <p style={{ fontSize: '0.875rem', color: '#64748b', margin: '0.25rem 0 0' }}>Sınav değerlendirmesi için yeni bir çalışma alanı başlatın.</p>
        </div>
        
        {error && <div style={{ padding: '1.5rem 1.5rem 0' }}><ErrorBanner error={error} /></div>}
        
        <form onSubmit={handleSubmit} style={{ padding: '1.5rem', display: 'flex', flexDirection: 'column', gap: '1.5rem' }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
            <label htmlFor="name" style={{ fontSize: '0.875rem', fontWeight: 600, color: '#334155' }}>Proje Adı</label>
            <input
              id="name"
              type="text"
              value={name}
              onChange={handleNameChange}
              disabled={isLoading}
              style={{ width: '100%', padding: '0.5rem 1rem', borderRadius: '0.75rem', border: '1px solid #cbd5e1', outline: 'none', fontSize: '1rem', transition: 'border-color 0.2s, box-shadow 0.2s', boxSizing: 'border-box' }}
              placeholder="Örn: 10. Sınıf Edebiyat 1. Dönem 1. Yazılı"
              autoFocus
              onFocus={(e) => { e.currentTarget.style.borderColor = '#6366f1'; e.currentTarget.style.boxShadow = '0 0 0 2px rgba(99, 102, 241, 0.2)'; }}
              onBlur={(e) => { e.currentTarget.style.borderColor = '#cbd5e1'; e.currentTarget.style.boxShadow = 'none'; }}
            />
            {!name.trim() && <p style={{ fontSize: '0.75rem', color: '#d97706', margin: 0 }}>Proje adı zorunludur.</p>}
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, minmax(0, 1fr))', gap: '0.75rem' }}>
            <label style={{ display: 'grid', gap: '0.4rem', fontSize: '0.8rem', fontWeight: 600, color: '#334155' }}>Eğitim yılı<input value={academicYearId} onChange={handleAcademicYearChange} placeholder="2026-2027" disabled={isLoading} style={{ padding: '0.5rem', border: '1px solid #cbd5e1', borderRadius: '0.6rem' }} /></label>
            <label style={{ display: 'grid', gap: '0.4rem', fontSize: '0.8rem', fontWeight: 600, color: '#334155' }}>Ders kodu<input value={courseId} onChange={(event) => setCourseId(event.target.value)} placeholder="tde" disabled={isLoading} style={{ padding: '0.5rem', border: '1px solid #cbd5e1', borderRadius: '0.6rem' }} /></label>
            <label style={{ display: 'grid', gap: '0.4rem', fontSize: '0.8rem', fontWeight: 600, color: '#334155' }}>Ders adı<input value={courseName} onChange={(event) => setCourseName(event.target.value)} placeholder="Türk Dili ve Edebiyatı" disabled={isLoading} style={{ padding: '0.5rem', border: '1px solid #cbd5e1', borderRadius: '0.6rem' }} /></label>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
            <label htmlFor="rootPath" style={{ fontSize: '0.875rem', fontWeight: 600, color: '#334155' }}>Klasör Yolu</label>
            <div style={{ display: 'flex', gap: '0.5rem' }}>
              <input
                id="rootPath"
                type="text"
                value={rootPath}
                onChange={handlePathChange}
                disabled={isLoading}
                placeholder="Belgeler/RubrikaV3/Projects/..."
                style={{ flex: 1, padding: '0.5rem 1rem', background: '#f8fafc', borderRadius: '0.75rem', border: '1px solid #cbd5e1', outline: 'none', fontSize: '1rem', color: '#475569', transition: 'border-color 0.2s, box-shadow 0.2s', boxSizing: 'border-box' }}
                onFocus={(e) => { e.currentTarget.style.borderColor = '#6366f1'; e.currentTarget.style.boxShadow = '0 0 0 2px rgba(99, 102, 241, 0.2)'; }}
                onBlur={(e) => { e.currentTarget.style.borderColor = '#cbd5e1'; e.currentTarget.style.boxShadow = 'none'; }}
              />
              <button
                type="button"
                onClick={handleSelectFolder}
                disabled={isLoading}
                style={{ padding: '0.5rem 1rem', border: '1px solid #cbd5e1', borderRadius: '0.75rem', background: 'white', color: '#475569', display: 'flex', alignItems: 'center', justifyContent: 'center', cursor: 'pointer', transition: 'background 0.2s' }}
                onMouseOver={(e) => (e.currentTarget.style.background = '#f8fafc')}
                onMouseOut={(e) => (e.currentTarget.style.background = 'white')}
              >
                📁
              </button>
            </div>
            <p style={{ fontSize: '0.75rem', color: '#64748b', margin: 0 }}>
              Varsayılan klasör adı eğitim yılıyla birlikte oluşturulur; örneğin <code>11_edebiyat_1_Yazili_2026-2027</code>.
            </p>
          </div>

          <div style={{ display: 'flex', justifyContent: 'flex-end', paddingTop: '1rem' }}>
            <LoadingButton 
              type="submit" 
              loading={isLoading} 
              disabledReason={disabledReason}
              style={{ background: '#4f46e5', color: 'white', padding: '0.5rem 1.5rem', borderRadius: '0.75rem', fontWeight: 600, border: 'none', cursor: isLoading || disabledReason ? 'not-allowed' : 'pointer', transition: 'background 0.2s', boxShadow: '0 4px 6px -1px rgba(79, 70, 229, 0.2)' }}
            >
              Proje Oluştur
            </LoadingButton>
          </div>
        </form>
      </div>
    </div>
  );
}
