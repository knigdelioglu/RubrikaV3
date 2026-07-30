import type { SpeakingMetrics } from '../../api/types';
import { speakingMeasurementConfidenceLabel } from '../../pages/speechExamUi';

export type SpeechMetricsPanelProps = {
  metrics?: SpeakingMetrics | null;
};

function formatMsToDuration(ms?: number): string {
  if (ms === undefined || ms === null || !Number.isFinite(ms)) return '—';
  const seconds = Math.max(0, Math.round(ms / 1000));
  const mins = Math.floor(seconds / 60);
  const remSecs = seconds % 60;
  if (mins > 0) {
    return `${mins} dk ${remSecs} sn`;
  }
  return `${remSecs} sn`;
}

function formatRatioPercent(ratio?: number): string {
  if (ratio === undefined || ratio === null || !Number.isFinite(ratio)) return '—';
  return `%${(ratio * 100).toFixed(1)}`;
}

export function SpeechMetricsPanel({ metrics }: SpeechMetricsPanelProps) {
  if (!metrics || metrics.durationMs === 0) {
    return (
      <div className="speech-metrics-panel speech-metrics-panel--empty">
        <div className="speech-metrics-panel__header">
          <h4>Ses ve konuşma göstergeleri</h4>
          <span className="speech-metrics-badge">Ölçüm mevcut değil</span>
        </div>
        <p className="speech-metrics-panel__note">
          Konuşma kaydı tamamlandığında backend ses ve zamanlama metrikleri burada görüntülenecektir.
        </p>
      </div>
    );
  }

  const hasClippingWarning = Boolean(
    (metrics.clippingRatio && metrics.clippingRatio > 0.005) ||
      (metrics.clippingEventCount && metrics.clippingEventCount > 5),
  );

  return (
    <div className="speech-metrics-panel">
      <div className="speech-metrics-panel__header">
        <div>
          <h4>Ses ve konuşma göstergeleri</h4>
          <p>Speakoflow motorundan gelen gerçek ses ve zamanlama metrikleri</p>
        </div>
        <span className="speech-metrics-badge">
          {speakingMeasurementConfidenceLabel(metrics.measurementConfidence)}
        </span>
      </div>

      {hasClippingWarning && (
        <div className="speech-inline-warning">
          <strong>Kayıt kalitesi uyarısı:</strong> Kayıtta ses taşması (clipping) tespit edildi.
          Bu durum mikrofon seviyesinden kaynaklanabilir; öğrenci puanı otomatik olarak düşürülmedi.
        </div>
      )}

      <div className="speech-metrics-grid">
        <div className="speech-metric-card">
          <span className="speech-metric-card__label">Toplam kayıt süresi</span>
          <strong className="speech-metric-card__value">{formatMsToDuration(metrics.durationMs)}</strong>
          <small className="speech-metric-card__tag">Süre ölçütünde kullanıldı</small>
        </div>

        <div className="speech-metric-card">
          <span className="speech-metric-card__label">Aktif konuşma süresi</span>
          <strong className="speech-metric-card__value">{formatMsToDuration(metrics.activeSpeechDurationMs)}</strong>
          <small className="speech-metric-card__tag">Akıcılık puanında kullanıldı</small>
        </div>

        <div className="speech-metric-card">
          <span className="speech-metric-card__label">Konuşma hızı (WPM)</span>
          <strong className="speech-metric-card__value">{metrics.wordsPerMinute ? `${Math.round(metrics.wordsPerMinute)} kelime/dk` : '—'}</strong>
          <small className="speech-metric-card__tag">Akıcılık puanında kullanıldı</small>
        </div>

        <div className="speech-metric-card">
          <span className="speech-metric-card__label">Uzun sessizlik sayısı</span>
          <strong className="speech-metric-card__value">{metrics.longPauseCount}</strong>
          <small className="speech-metric-card__tag">Akıcılık puanında kullanıldı</small>
        </div>

        <div className="speech-metric-card">
          <span className="speech-metric-card__label">En uzun sessizlik</span>
          <strong className="speech-metric-card__value">{formatMsToDuration(metrics.longestSilenceMs ?? 0)}</strong>
          <small className="speech-metric-card__tag">Yalnız yardımcı gösterge</small>
        </div>

        <div className="speech-metric-card">
          <span className="speech-metric-card__label">Toplam sessizlik oranı</span>
          <strong className="speech-metric-card__value">{formatRatioPercent(metrics.silenceRatio ?? (metrics.totalSilenceMs / metrics.durationMs))}</strong>
          <small className="speech-metric-card__tag">Yalnız yardımcı gösterge</small>
        </div>

        <div className="speech-metric-card">
          <span className="speech-metric-card__label">Dolgu ifadesi sayısı</span>
          <strong className="speech-metric-card__value">{metrics.fillerCount}</strong>
          <small className="speech-metric-card__tag">Akıcılık puanında kullanıldı</small>
        </div>

        <div className="speech-metric-card">
          <span className="speech-metric-card__label">Belirgin tekrar sayısı</span>
          <strong className="speech-metric-card__value">{metrics.repetitionCount}</strong>
          <small className="speech-metric-card__tag">Akıcılık puanında kullanıldı</small>
        </div>

        <div className="speech-metric-card">
          <span className="speech-metric-card__label">Clipping olay sayısı</span>
          <strong className="speech-metric-card__value">{metrics.clippingEventCount ?? 0}</strong>
          <small className="speech-metric-card__tag">Yalnız kayıt kalitesi uyarısı</small>
        </div>

        <div className="speech-metric-card">
          <span className="speech-metric-card__label">Clipping oranı</span>
          <strong className="speech-metric-card__value">{formatRatioPercent(metrics.clippingRatio ?? 0)}</strong>
          <small className="speech-metric-card__tag">Puan düşürmez</small>
        </div>

        <div className="speech-metric-card">
          <span className="speech-metric-card__label">Düşük sesli bölüm oranı</span>
          <strong className="speech-metric-card__value">{formatRatioPercent(metrics.lowVolumeRatio ?? 0)}</strong>
          <small className="speech-metric-card__tag">Yalnız yardımcı gösterge</small>
        </div>
      </div>
    </div>
  );
}
