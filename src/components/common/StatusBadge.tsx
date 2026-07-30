export function StatusBadge({ status, label }: { status: 'ready' | 'error' | 'loading', label: string }) {
  const colors = {
    ready: '#dcfce7',
    error: '#fee2e2',
    loading: '#fef3c7',
  };
  return (
    <span style={{ padding: '0.25rem 0.5rem', background: colors[status], borderRadius: '4px', fontSize: '0.875rem' }}>
      {label}
    </span>
  );
}
