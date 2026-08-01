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
      <strong>Hata:</strong> {error.safeMessage}
      {error.recoveryAction && <p>Öneri: {error.recoveryAction}</p>}
      {isConflict && onRefresh && (
        <button type="button" onClick={() => void onRefresh()}>
          Son durumu yenile
        </button>
      )}
      {showTechnicalDetails && error.detailsAvailable && (
        <p style={{ marginTop: '0.5rem', fontSize: '0.8rem', opacity: 0.8 }}>
          Teknik ayrıntılar Tanılama görünümünde {error.correlationId} koduyla erişilebilir.
        </p>
      )}
    </div>
  );
}
