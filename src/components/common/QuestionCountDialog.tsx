import { useEffect, useRef, useState } from 'react';
import { LoadingButton } from './LoadingButton';

type Props = {
  open: boolean;
  title: string;
  description: string;
  confirmLabel: string;
  initialValue?: number;
  onCancel: () => void;
  onConfirm: (count: number) => Promise<boolean>;
};

export function QuestionCountDialog({
  open,
  title,
  description,
  confirmLabel,
  initialValue = 6,
  onCancel,
  onConfirm,
}: Props) {
  const [value, setValue] = useState(String(initialValue));
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (open) {
      returnFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      setValue(String(initialValue));
      setError(null);
      setLoading(false);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
    return () => {
      if (open) returnFocusRef.current?.focus();
    };
  }, [initialValue, open]);

  useEffect(() => {
    if (!open) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !loading) {
        event.preventDefault();
        onCancel();
        return;
      }
      if (event.key !== 'Tab') return;
      const focusable = Array.from(
        dialogRef.current?.querySelectorAll<HTMLElement>(
          'button:not([disabled]), input:not([disabled]), [href], [tabindex]:not([tabindex="-1"])',
        ) ?? [],
      );
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (!first || !last) {
        event.preventDefault();
        dialogRef.current?.focus();
      } else if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [loading, onCancel, open]);

  if (!open) return null;

  const parsed = Number(value);
  const isValid = Number.isInteger(parsed) && parsed >= 1 && parsed <= 50;

  const handleConfirm = async () => {
    if (!isValid) {
      setError('Soru sayısı 1 ile 50 arasında olmalıdır.');
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const shouldClose = await onConfirm(parsed);
      if (shouldClose) {
        onCancel();
      }
    } catch {
      setError('İşlem tamamlanamadı.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 50,
        background: 'rgba(15, 23, 42, 0.45)',
        display: 'grid',
        placeItems: 'center',
        padding: '1rem',
      }}
      role="presentation"
      onClick={() => !loading && onCancel()}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="question-count-dialog-title"
        aria-describedby="question-count-dialog-description"
        tabIndex={-1}
        style={{
          width: 'min(520px, 100%)',
          borderRadius: '14px',
          background: '#fff',
          padding: '1.25rem',
          boxShadow: '0 20px 60px rgba(15, 23, 42, 0.25)',
        }}
        onClick={(event) => event.stopPropagation()}
      >
        <h2 id="question-count-dialog-title" style={{ margin: 0, fontSize: '1.1rem' }}>
          {title}
        </h2>
        <p id="question-count-dialog-description" style={{ margin: '0.65rem 0 1rem', color: '#475569' }}>{description}</p>
        <label style={{ display: 'grid', gap: '0.35rem' }}>
          <span>Soru sayısı</span>
          <input
            ref={inputRef}
            type="number"
            min={1}
            max={50}
            step={1}
            value={value}
            onChange={(event) => setValue(event.target.value)}
            style={{
              padding: '0.75rem 0.9rem',
              borderRadius: '10px',
              border: '1px solid #cbd5e1',
              fontSize: '1rem',
            }}
          />
        </label>
        {error && (
          <div role="alert" style={{ marginTop: '0.75rem', color: '#b91c1c' }}>
            {error}
          </div>
        )}
        <div style={{ display: 'flex', gap: '0.75rem', justifyContent: 'flex-end', marginTop: '1rem' }}>
          <button
            type="button"
            data-project-write="false"
            onClick={onCancel}
            disabled={loading}
            style={{
              padding: '0.7rem 1rem',
              borderRadius: '10px',
              border: '1px solid #cbd5e1',
              background: '#fff',
            }}
          >
            İptal
          </button>
          <LoadingButton
            type="button"
            projectWrite
            onClick={handleConfirm}
            loading={loading}
            disabledReason={!isValid ? 'Soru sayısı 1 ile 50 arasında olmalıdır.' : undefined}
            style={{
              padding: '0.7rem 1rem',
              borderRadius: '10px',
              border: 'none',
              background: '#2563eb',
              color: 'white',
            }}
          >
            {confirmLabel}
          </LoadingButton>
        </div>
      </div>
    </div>
  );
}
