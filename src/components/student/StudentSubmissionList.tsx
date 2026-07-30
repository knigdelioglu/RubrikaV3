import { Link } from 'react-router-dom';
import type { Student, StudentSubmission } from '../../api/types';
import { studentAnswerSlotStatusLabels, studentSubmissionStatusLabels } from '../../utils/labels';
import { PageGroupEditor } from './PageGroupEditor';
import { StudentIdentityEditor } from './StudentIdentityEditor';
import { formatPageRange } from '../../utils/formatting';

type StudentSubmissionListProps = {
  projectId: string;
  submissions: StudentSubmission[];
  students: Student[];
  onChanged?: () => void;
};

export function StudentSubmissionList({ projectId, submissions, students, onChanged }: StudentSubmissionListProps) {
  const studentById = new Map(students.map((student) => [student.id, student]));

  if (submissions.length === 0) {
    return <div>Henüz öğrenci grubu oluşturulmadı.</div>;
  }

  return (
    <div style={{ display: 'grid', gap: '1rem' }}>
      {submissions.map((submission, index) => {
        const student = studentById.get(submission.studentId);
        const pageRange = formatPageRange(submission.pageNumbers);
        const title = `Öğrenci ${index + 1}`;
        return (
          <article key={submission.id} style={{ padding: '1rem', border: '1px solid #cbd5e1', borderRadius: '18px', background: 'white', display: 'grid', gap: '0.75rem' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', flexWrap: 'wrap', alignItems: 'center' }}>
              <strong>{title}</strong>
              <span>{studentSubmissionStatusLabels[submission.status] || submission.status}</span>
            </div>
            <div>Sayfalar: {pageRange}</div>
            <div>Ad Soyad: {student?.displayName || '-'}</div>
            <div>Numara: {student?.number || '-'}</div>
            <div>Durum: Kontrol bekliyor</div>
            <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap' }}>
              <Link to={`/project/${encodeURIComponent(projectId)}/exam/documents?documentId=${encodeURIComponent(submission.documentId)}&documentType=student`}>
                Önizle
              </Link>
            </div>
            <div style={{ marginTop: '0.25rem' }}>
              <h4 style={{ marginTop: 0 }}>Kimlik</h4>
              <StudentIdentityEditor projectId={projectId} submission={submission} student={student} onSaved={onChanged} />
            </div>
            <div>
              <h4 style={{ marginTop: 0 }}>Sayfa Grubu</h4>
              <PageGroupEditor projectId={projectId} submission={submission} onSaved={onChanged} />
            </div>
            <div>
              <h4 style={{ marginTop: 0 }}>Cevap Slotları</h4>
              <ul style={{ margin: 0, paddingLeft: '1.25rem' }}>
                {submission.answerSlots.map((slot) => (
                  <li key={slot.questionId}>
                    Soru {slot.questionNumber}: {studentAnswerSlotStatusLabels[slot.status] || slot.status}
                  </li>
                ))}
              </ul>
            </div>
            {submission.warnings.length > 0 && <div>Uyarılar: {submission.warnings.join(', ')}</div>}
          </article>
        );
      })}
    </div>
  );
}
