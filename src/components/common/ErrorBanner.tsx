import type { AppError } from '../../api/errors';

export function ErrorBanner({ error, showTechnicalDetails = false }: { error: AppError; showTechnicalDetails?: boolean }) {
  if (!error) return null;
  return (
    <div style={{ padding: '1rem', background: '#fee2e2', color: '#991b1b', borderRadius: '4px' }}>
      <strong>Hata:</strong> {error.message}
      {error.suggestedAction && <p>Öneri: {error.suggestedAction}</p>}
      {showTechnicalDetails && error.technicalDetails && (
        <details style={{ marginTop: '0.5rem' }}>
          <summary style={{ cursor: 'pointer' }}>Teknik ayrıntılar</summary>
          <pre style={{ whiteSpace: 'pre-wrap', marginTop: '0.5rem' }}>{error.technicalDetails}</pre>
        </details>
      )}
    </div>
  );
}
