import type { RubricQuestionSnapshot } from '../../api/types';
import { RubricQuestionCard, type RubricDraft } from './RubricQuestionCard';

type RubricEditorProps = {
  items: RubricQuestionSnapshot[];
  drafts: Record<string, RubricDraft>;
  savingQuestionId: string | null;
  disabledReasonByQuestionId: Record<string, string | undefined>;
  onDraftChange: (questionId: string, next: RubricDraft) => void;
  onSave: (questionId: string) => void;
  onConfirm: (questionId: string) => void;
};

export function RubricEditor({
  items,
  drafts,
  savingQuestionId,
  disabledReasonByQuestionId,
  onDraftChange,
  onSave,
  onConfirm,
}: RubricEditorProps) {
  if (items.length === 0) {
    return <div>Henüz rubrik sorusu yok.</div>;
  }

  const fallbackDraft: RubricDraft = {
    answerType: 'general_text',
    maxScore: '',
    expectedAnswer: '',
    keyConcepts: '',
    criteria: [],
    partialCreditHints: '',
    zeroScoreConditions: '',
    commonMistakes: '',
  };

  return (
    <div style={{ display: 'grid', gap: '1rem' }}>
      {items.map((item) => (
        <RubricQuestionCard
          key={item.question.id}
          item={item}
          draft={drafts[item.question.id] ?? fallbackDraft}
          saving={savingQuestionId === item.question.id}
          disabledReason={disabledReasonByQuestionId[item.question.id]}
          onDraftChange={onDraftChange}
          onSave={onSave}
          onConfirm={onConfirm}
        />
      ))}
    </div>
  );
}
