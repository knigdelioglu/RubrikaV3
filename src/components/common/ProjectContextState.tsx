import { Link } from 'react-router-dom';

type ProjectContextStateProps = {
  pageLabel: string;
  projectPath?: string;
  loading?: boolean;
};

export function ProjectContextState({ pageLabel, loading }: ProjectContextStateProps) {
  if (loading) {
    return <div style={{ padding: '2rem' }}>{pageLabel} için proje bağlamı yükleniyor...</div>;
  }

  return (
    <div style={{ padding: '2rem' }}>
      {pageLabel} açılırken proje bağlamı bulunamadı.
      <div style={{ marginTop: '0.75rem' }}>
        <Link to="/projects">Projeler sayfasına git</Link>
      </div>
    </div>
  );
}
