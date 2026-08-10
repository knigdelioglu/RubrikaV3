import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getMobileConnection, mobileClient, saveMobileConnection } from '../api/mobileClient';

export function MobileDashboardPage() {
  const saved = getMobileConnection();
  const [baseUrl, setBaseUrl] = useState(saved.baseUrl);
  const [token, setToken] = useState(saved.token);
  const [connected, setConnected] = useState(false);
  const healthQuery = useQuery({
    queryKey: ['mobile-health', connected],
    queryFn: mobileClient.health,
    enabled: connected,
  });
  const projectsQuery = useQuery({
    queryKey: ['mobile-projects', connected],
    queryFn: mobileClient.listProjects,
    enabled: connected,
  });

  const connect = () => {
    saveMobileConnection(baseUrl, token);
    setConnected(true);
    void healthQuery.refetch();
  };

  return (
    <main className="mobile-mvp" aria-labelledby="mobile-mvp-title">
      <section className="mobile-mvp__header">
        <p className="mobile-mvp__eyebrow">Rubrika V3 · Tablet MVP</p>
        <h1 id="mobile-mvp-title">MacBook bağlantısı</h1>
        <p>Gemma ve Rust backend MacBook’ta çalışır; tablet bu yerel ağ API’sine bağlanır.</p>
      </section>

      <section className="mobile-mvp__card">
        <label>
          MacBook adresi
          <input value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} placeholder="http://192.168.1.20:8787" inputMode="url" />
        </label>
        <label>
          Mobil erişim anahtarı
          <input value={token} onChange={(event) => setToken(event.target.value)} type="password" autoComplete="off" />
        </label>
        <button type="button" className="button button--primary" onClick={connect}>
          Bağlan
        </button>
        {healthQuery.isError && <p className="mobile-mvp__error" role="alert">MacBook’a bağlanılamadı. Adres, anahtar ve aynı Wi‑Fi ağını kontrol edin.</p>}
        {healthQuery.data && <p className="mobile-mvp__success">Bağlandı · Rust backend hazır · {healthQuery.data.platform}</p>}
      </section>

      {connected && (
        <section className="mobile-mvp__card" aria-labelledby="mobile-projects-title">
          <div className="mobile-mvp__section-heading">
            <h2 id="mobile-projects-title">Projeler</h2>
            <button type="button" className="button button--secondary" onClick={() => void projectsQuery.refetch()}>Yenile</button>
          </div>
          {projectsQuery.isLoading && <p>Projeler yükleniyor…</p>}
          {projectsQuery.isError && <p className="mobile-mvp__error" role="alert">Projeler okunamadı.</p>}
          {projectsQuery.data?.projects.map((project) => (
            <article className="mobile-mvp__project" key={project.id}>
              <div>
                <strong>{project.name}</strong>
                <span>{project.questionCount ?? 0} soru · {project.statusSummary?.hasStudentScan ? 'Öğrenci taraması hazır' : 'Taraması yok'}</span>
              </div>
              <span className="mobile-mvp__project-id">{project.id.slice(0, 8)}</span>
            </article>
          ))}
          {!projectsQuery.isLoading && projectsQuery.data?.projects.length === 0 && <p>MacBook’ta henüz proje yok.</p>}
        </section>
      )}
    </main>
  );
}
