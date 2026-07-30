import { useEffect, useState } from 'react';
import { commands } from '../../api/commands';
import type { AppError } from '../../api/errors';
import type { Student, StudentSubmission } from '../../api/types';
import { ErrorBanner } from '../common/ErrorBanner';
import { LoadingButton } from '../common/LoadingButton';

type StudentIdentityEditorProps = {
  projectId: string;
  submission: StudentSubmission;
  student?: Student | null;
  onSaved?: () => void;
};

export function StudentIdentityEditor({
  projectId,
  submission,
  student,
  onSaved,
}: StudentIdentityEditorProps) {
  const [displayName, setDisplayName] = useState(student?.displayName ?? '');
  const [number, setNumber] = useState(student?.number ?? '');
  const [className, setClassName] = useState(student?.className ?? '');
  const [error, setError] = useState<AppError | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setDisplayName(student?.displayName ?? '');
    setNumber(student?.number ?? '');
    setClassName(student?.className ?? '');
  }, [student?.displayName, student?.number, student?.className]);

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      await commands.updateStudentIdentity({
        projectId,
        submissionId: submission.id,
        displayName: displayName.trim().length > 0 ? displayName.trim() : null,
        number: number.trim().length > 0 ? number.trim() : null,
        className: className.trim().length > 0 ? className.trim() : null,
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
        <span>Ad / Soyad</span>
        <input value={displayName} onChange={(event) => setDisplayName(event.target.value)} placeholder="Öğrenci adı" />
      </label>
      <label style={{ display: 'grid', gap: '0.25rem' }}>
        <span>Numara</span>
        <input value={number} onChange={(event) => setNumber(event.target.value)} placeholder="Öğrenci numarası" />
      </label>
      <label style={{ display: 'grid', gap: '0.25rem' }}>
        <span>Sınıf</span>
        <input value={className} onChange={(event) => setClassName(event.target.value)} placeholder="Sınıf adı" />
      </label>
      <LoadingButton onClick={handleSave} loading={saving}>
        Kimliği Kaydet
      </LoadingButton>
    </div>
  );
}
