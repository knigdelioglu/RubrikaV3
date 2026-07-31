import { useMemo } from 'react';
import { Link, useParams, useSearchParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { BarChart3, CircleAlert, FileText, Loader2, Mic2, Sparkles, Users } from 'lucide-react';
import { commands } from '../api/commands';
import type { AssessmentKind } from '../api/types';
import {
  analysisStatusLabel,
  clampAnalysisPercentage,
  latestAnalysisId,
  percentageLabel,
} from './analysisUi';

function AnalysisBar({ value }: { value: number }) {
  return (
    <div
      style={{
        height: 10,
        borderRadius: 999,
        background: '#e2e8f0',
        overflow: 'hidden',
        flex: 1,
      }}
      aria-label={`Yüzde ${Math.round(value)}`}
    >
      <span
        style={{
          display: 'block',
          width: `${clampAnalysisPercentage(value)}%`,
          height: '100%',
          background: value >= 70 ? '#0f766e' : value >= 50 ? '#d97706' : '#dc2626',
        }}
      />
    </div>
  );
}

export function AnalysisPage({ kind }: { kind: AssessmentKind }) {
  const { projectId = '' } = useParams<{ projectId: string }>();
  const [searchParams] = useSearchParams();
  const requestedAnalysisId = searchParams.get('analysisId') ?? '';
  const analysesQuery = useQuery({
    queryKey: ['assessment-analyses', projectId],
    queryFn: () => commands.listAssessmentAnalyses(projectId),
    enabled: Boolean(projectId) && !requestedAnalysisId,
  });
  const latestId = useMemo(
    () => latestAnalysisId(analysesQuery.data, kind),
    [analysesQuery.data, kind],
  );
  const analysisId = requestedAnalysisId || latestId;
  const analysisQuery = useQuery({
    queryKey: ['assessment-analysis', projectId, analysisId],
    queryFn: () => commands.getAssessmentAnalysis(projectId, analysisId),
    enabled: Boolean(projectId && analysisId),
    refetchInterval: (query) => (query.state.data?.status === 'generating' ? 1000 : false),
  });
  const analysis = analysisQuery.data;
  const backPath =
    kind === 'speaking'
      ? `/project/${encodeURIComponent(projectId)}/speaking`
      : `/project/${encodeURIComponent(projectId)}/grading`;

  if (analysesQuery.isLoading || (analysisId && analysisQuery.isLoading)) {
    return <div style={{ padding: '2rem' }}><Loader2 className="animate-spin" /> Analiz yükleniyor…</div>;
  }

  if (!analysisId) {
    const allAnalyses = analysesQuery.data ?? [];
    if (allAnalyses.length === 0) {
      return (
        <div style={{ padding: "2.5rem", maxWidth: "800px", margin: "2rem auto 0" }}>
          <section style={{ padding: "3.5rem 2rem", background: "#fff", border: "1px dashed #cbd5e1", borderRadius: "1.25rem", textAlign: "center" }}>
            <h2 style={{ fontSize: "1.35rem", fontWeight: 800, color: "#0f172a", margin: 0 }}>Henüz tamamlanmış sınav raporu bulunmuyor</h2>
            <p style={{ margin: "0.75rem 0 0", color: "#64748b", fontSize: "0.925rem", lineHeight: 1.55 }}>
              Bir sınavın notlandırmasını veya değerlendirmesini tamamlayıp sonuçları kesinleştirdiğinizde raporlar bu alanda listelenir.
            </p>
          </section>
        </div>
      );
    }

    return (
      <div style={{ padding: "2rem", maxWidth: "1000px", margin: "0 auto" }}>
        <div style={{ marginBottom: "1.5rem" }}>
          <h2 style={{ fontSize: "1.5rem", fontWeight: 800, color: "#0f172a", margin: 0 }}>Raporlar</h2>
          <p style={{ color: "#64748b", fontSize: "0.875rem", margin: "0.25rem 0 0" }}>
            Tamamlanan sınavların grafiklerini ve başarı raporlarını inceleyin.
          </p>
        </div>
        <div style={{ display: "grid", gap: "1rem" }}>
          {allAnalyses.map((item) => (
            <article key={item.id} style={{ padding: "1.25rem", background: "#fff", border: "1px solid #e2e8f0", borderRadius: "1rem", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <h3 style={{ margin: 0, fontSize: "1.05rem", fontWeight: 700, color: "#0f172a" }}>{item.title}</h3>
                <p style={{ margin: "0.35rem 0 0", fontSize: "0.825rem", color: "#64748b" }}>
                  {item.kind === "speaking" ? "Konuşma Sınavı" : "Yazılı Sınav"} · {item.studentCount} öğrenci · Sonuçlar kesinleştirildi
                </p>
              </div>
              <Link to={`/project/${encodeURIComponent(projectId)}/analysis?analysisId=${encodeURIComponent(item.id)}`} className="button button--primary" style={{ padding: "0.5rem 1rem", fontSize: "0.85rem" }}>
                Raporu aç
              </Link>
            </article>
          ))}
        </div>
      </div>
    );
  }

  if (!analysis) {
    return (
      <section style={{ padding: '2rem' }}>
        <CircleAlert size={24} />
        <h2>Analiz açılamadı</h2>
        <p>Analiz dosyası bulunamadı veya okunamadı.</p>
        <Link className="button button--secondary" to={backPath}>Geri dön</Link>
      </section>
    );
  }

  return (
    <div style={{ padding: '2rem', maxWidth: 1280, margin: '0 auto' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', flexWrap: 'wrap', marginBottom: '1.5rem' }}>
        <div>
          <p className="eyebrow">{kind === 'speaking' ? 'Konuşma sınavı analizi' : 'Yazılı sınav analizi'}</p>
          <h2 style={{ margin: 0 }}>{analysis.title}</h2>
          <p style={{ color: '#64748b' }}>
            {analysis.studentCount} öğrenci · grafikler kaydedildi ·{' '}
            {analysisStatusLabel(analysis.status)}
          </p>
        </div>
        <div style={{ display: 'flex', gap: '0.75rem', alignItems: 'center' }}>
          <button type="button" className="button button--primary" onClick={() => window.alert('Sınav sonuçları kesinleştirildi.')}>
            <Sparkles size={16} /> Sonuçları Kesinleştir
          </button>
          <Link className="button button--secondary" to={backPath}>
            {kind === 'speaking' ? <Mic2 size={16} /> : <FileText size={16} />} Değerlendirmeye dön
          </Link>
        </div>
      </div>

      <section
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
          gap: '1rem',
          marginBottom: '1rem',
        }}
      >
        <article className="speech-panel">
          <div className="speech-panel__heading"><div><h3>Başarı dağılımı</h3><p>Toplam puan yüzdesine göre</p></div><BarChart3 size={19} /></div>
          <div style={{ display: 'grid', gap: '.8rem' }}>
            {analysis.scoreBands.map((band) => (
              <div key={band.label} style={{ display: 'grid', gridTemplateColumns: '110px 1fr 32px', gap: '.65rem', alignItems: 'center' }}>
                <span>{band.label}</span>
                <AnalysisBar value={analysis.studentCount ? (band.count / analysis.studentCount) * 100 : 0} />
                <strong>{band.count}</strong>
              </div>
            ))}
          </div>
        </article>
        <article className="speech-panel">
          <div className="speech-panel__heading"><div><h3>Öğrenci sonuçları</h3><p>Kaydedilmiş nihai puanlar</p></div><Users size={19} /></div>
          <div style={{ display: 'grid', gap: '.8rem' }}>
            {analysis.students.map((student) => (
              <div key={student.studentId} style={{ display: 'grid', gridTemplateColumns: 'minmax(100px, 1fr) 2fr 54px', gap: '.65rem', alignItems: 'center' }}>
                <span>{student.displayName}</span>
                <AnalysisBar value={student.percentage} />
                <strong>{percentageLabel(student.percentage)}</strong>
              </div>
            ))}
          </div>
        </article>
      </section>

      <section className="speech-panel" style={{ marginBottom: '1rem' }}>
        <div className="speech-panel__heading"><div><h3>Rubrik / soru bazlı görünüm</h3><p>Yalnızca puan bulunan ölçütler</p></div><BarChart3 size={19} /></div>
        <div style={{ display: 'grid', gap: '.9rem' }}>
          {analysis.criteria.map((criterion) => (
            <div key={criterion.id} style={{ display: 'grid', gridTemplateColumns: 'minmax(180px, 1fr) 3fr 90px', gap: '.8rem', alignItems: 'center' }}>
              <span>{criterion.label}</span>
              <AnalysisBar value={criterion.percentage} />
              <strong>{criterion.averageScore}/{criterion.maxScore}</strong>
            </div>
          ))}
        </div>
      </section>

      <section className="speech-panel">
        <div className="speech-panel__heading">
          <div><h3>Gemma 4 12B öğretmen raporu</h3><p>Anonim toplu ölçümlere dayalıdır</p></div>
          {analysis.status === 'generating' ? <Loader2 size={19} className="animate-spin" /> : <Sparkles size={19} />}
        </div>
        {analysis.modelReport ? (
          <div style={{ whiteSpace: 'pre-wrap', lineHeight: 1.7 }}>{analysis.modelReport}</div>
        ) : analysis.status === 'generating' ? (
          <p>Rapor arka planda hazırlanıyor. Grafikler şimdiden kullanılabilir.</p>
        ) : (
          <div className="speech-form-note">
            <CircleAlert size={16} /> {analysis.modelReportError ?? 'Rapor üretilemedi; grafik verileri korunmuştur.'}
          </div>
        )}
      </section>
    </div>
  );
}
