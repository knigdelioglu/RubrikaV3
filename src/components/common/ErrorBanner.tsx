import { isProjectConflictError, type AppError } from '../../api/errors';

export function ErrorBanner({
  error,
  showTechnicalDetails = false,
  onRefresh,
}: {
  error: AppError;
  showTechnicalDetails?: boolean;
  onRefresh?: () => void | Promise<void>;
}) {
  if (!error) return null;
  const isConflict = isProjectConflictError(error);
  return (
    <div style={{ padding: '1rem', background: '#fee2e2', color: '#991b1b', borderRadius: '4px' }}>
      <strong>Hata:</strong> {error.message}
      {error.suggestedAction && <p>Öneri: {error.suggestedAction}</p>}
      {isConflict && onRefresh && (
        <button type="button" onClick={() => void onRefresh()}>
          Son durumu yenile
        </button>
      )}
      {showTechnicalDetails && error.technicalDetails && (
        <details style={{ marginTop: '0.5rem' }}>
          <summary style={{ cursor: 'pointer' }}>Teknik ayrıntılar</summary>
          <pre style={{ whiteSpace: 'pre-wrap', marginTop: '0.5rem' }}>{error.technicalDetails}</pre>
        </details>
      )}
    </div>
  );
}
