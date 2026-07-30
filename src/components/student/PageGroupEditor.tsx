import { useEffect, useState } from 'react';
import { commands } from '../../api/commands';
import type { AppError } from '../../api/errors';
import type { StudentSubmission } from '../../api/types';
import { ErrorBanner } from '../common/ErrorBanner';
import { LoadingButton } from '../common/LoadingButton';

function parsePageNumbers(value: string): number[] {
  return value
    .split(',')
    .map((part) => Number(part.trim()))
    .filter((value) => Number.isInteger(value) && value > 0);
}

type PageGroupEditorProps = {
  projectId: string;
  submission: StudentSubmission;
  onSaved?: () => void;
};

export function PageGroupEditor({ projectId, submission, onSaved }: PageGroupEditorProps) {
  const [value, setValue] = useState(submission.pageNumbers.join(', '));
  const [error, setError] = useState<AppError | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setValue(submission.pageNumbers.join(', '));
  }, [submission.pageNumbers]);

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      await commands.updateSubmissionPages({
        projectId,
        submissionId: submission.id,
        pageNumbers: parsePageNumbers(value),
      });
      onSaved?.();
    } catch (err) {
      setError(err as AppError);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div style={{ display: 'grid', gap: '0.5rem' }}>
      {error && <ErrorBanner error={error} />}
      <label style={{ display: 'grid', gap: '0.25rem' }}>
        <span>Sayfalar</span>
        <input
          value={value}
          onChange={(event) => setValue(event.target.value)}
          placeholder="1, 2, 3"
        />
      </label>
      <LoadingButton onClick={handleSave} loading={saving}>
        Sayfaları Kaydet
      </LoadingButton>
    </div>
  );
}
