import { LoadingButton } from '../common/LoadingButton';
import { answerTypeLabels, rubricSourceLabels, rubricStatusLabels, textFieldStatusLabels } from '../../utils/labels';
import type { AnswerType, RubricQuestionSnapshot } from '../../api/types';
import { teacherFacingRubricWarnings } from '../../utils/rubricWarnings';
import { optionalGuidanceEmptyText, optionalGuidanceText } from '../../utils/rubricGuidance';
import { CheckCircle2, AlertCircle, Trash2, Plus, Edit2, Save, FileText } from 'lucide-react';
import { useState } from 'react';

export type CriterionDraft = { id: string; label: string; description: string; points: string; };
export type RubricDraft = { answerType: AnswerType; maxScore: string; expectedAnswer: string; criteria: CriterionDraft[]; partialCreditHints: string; zeroScoreConditions: string; commonMistakes: string; };
type RubricQuestionCardProps = { item: RubricQuestionSnapshot; draft: RubricDraft; saving: boolean; disabledReason?: string; onDraftChange: (questionId: string, next: RubricDraft) => void; onSave: (questionId: string) => void | boolean | Promise<void | boolean>; onConfirm: (questionId: string) => void | boolean | Promise<void | boolean>; };

function updateDraft(draft: RubricDraft, patch: Partial<RubricDraft>): RubricDraft { return { ...draft, ...patch }; }

export function RubricQuestionCard({ item, draft, saving, disabledReason, onDraftChange, onSave, onConfirm }: RubricQuestionCardProps) {
  const [isEditing, setIsEditing] = useState(
    item.question.rubric.status === 'missing'
      || item.question.rubric.status === 'suggested'
      || item.question.rubric.status === 'invalid'
      || item.question.rubric.status === 'legacy',
  );
  
  const statusLabel = rubricStatusLabels[item.question.rubric.status] || item.question.rubric.status;
  const sourceLabel = item.question.rubric.source ? (rubricSourceLabels[item.question.rubric.source] || item.question.rubric.source) : 'Yok';
  const teacherWarnings = teacherFacingRubricWarnings(item.validation.warnings);
  const isApproved = item.question.rubric.status === 'confirmed';
  const isMissing = item.question.rubric.status === 'missing'
    || item.question.rubric.status === 'invalid'
    || item.question.rubric.status === 'legacy';
  const criterionPointsTotal = draft.criteria.reduce(
    (total, criterion) => total + Number(criterion.points || 0),
    0,
  );

  const handleCriterionChange = (criterionId: string, patch: Partial<CriterionDraft>) => {
    onDraftChange(item.question.id, updateDraft(draft, { criteria: draft.criteria.map((criterion) => criterion.id === criterionId ? { ...criterion, ...patch } : criterion) }));
  };

  const addCriterion = () => {
    onDraftChange(item.question.id, updateDraft(draft, { criteria: [...draft.criteria, { id: crypto.randomUUID(), label: '', description: '', points: '' }] }));
  };

  const removeCriterion = (criterionId: string) => {
    onDraftChange(item.question.id, updateDraft(draft, { criteria: draft.criteria.filter((criterion) => criterion.id !== criterionId) }));
  };

  const handleSaveWrapper = async () => {
    const succeeded = await onSave(item.question.id);
    if (succeeded !== false) setIsEditing(false);
  };

  const handleConfirmWrapper = async () => {
    const succeeded = await onConfirm(item.question.id);
    if (succeeded !== false) setIsEditing(false);
  };

  return (
    <article style={{ background: 'white', borderRadius: '1rem', border: '1px solid #e2e8f0', overflow: 'hidden', boxShadow: '0 1px 3px 0 rgba(0,0,0,0.1)', transition: 'all 0.2s', ...(isEditing ? { border: '1px solid #c7d2fe', boxShadow: '0 4px 6px -1px rgba(99, 102, 241, 0.1), 0 2px 4px -1px rgba(99, 102, 241, 0.06)' } : {}) }}>
      {/* Header */}
      <div style={{ padding: '1.25rem 1.5rem', background: isEditing ? '#f8fafc' : 'white', borderBottom: '1px solid #e2e8f0', display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '1rem' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '1rem' }}>
          <div style={{ fontSize: '1.125rem', fontWeight: 700, color: '#0f172a', display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', width: '2rem', height: '2rem', borderRadius: '0.5rem', background: '#e0e7ff', color: '#4f46e5', fontSize: '1rem', fontWeight: 700 }}>
              {item.question.number}
            </div>
            Soru {item.question.number}
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', flexWrap: 'wrap' }}>
            {isApproved ? (
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: '0.25rem', fontSize: '0.75rem', fontWeight: 500, color: '#15803d', background: '#dcfce7', padding: '0.25rem 0.75rem', borderRadius: '9999px' }}>
                <CheckCircle2 size={12} /> {statusLabel}
              </span>
            ) : isMissing ? (
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: '0.25rem', fontSize: '0.75rem', fontWeight: 500, color: '#b91c1c', background: '#fee2e2', padding: '0.25rem 0.75rem', borderRadius: '9999px' }}>
                <AlertCircle size={12} /> {statusLabel}
              </span>
            ) : (
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: '0.25rem', fontSize: '0.75rem', fontWeight: 500, color: '#b45309', background: '#fef3c7', padding: '0.25rem 0.75rem', borderRadius: '9999px' }}>
                <AlertCircle size={12} /> {statusLabel}
              </span>
            )}
            <span style={{ fontSize: '0.75rem', color: '#64748b', background: '#f1f5f9', padding: '0.25rem 0.75rem', borderRadius: '9999px' }}>
              Kaynak: {sourceLabel}
            </span>
            <span style={{ fontSize: '0.75rem', color: '#64748b', background: '#f1f5f9', padding: '0.25rem 0.75rem', borderRadius: '9999px' }}>
              Metin: {textFieldStatusLabels[item.question.questionText.status] || item.question.questionText.status}
            </span>
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
          <div style={{ fontSize: '1.25rem', fontWeight: 700, color: '#0f172a', background: '#f8fafc', padding: '0.25rem 0.75rem', borderRadius: '0.5rem', border: '1px solid #e2e8f0' }}>
            {draft.maxScore || item.question.rubric.maxScore || '0'}p
          </div>
          {!isEditing && (
            <button
              onClick={() => setIsEditing(true)}
              style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', padding: '0.5rem 1rem', background: 'white', color: '#475569', border: '1px solid #cbd5e1', borderRadius: '0.5rem', fontSize: '0.875rem', fontWeight: 500, cursor: 'pointer', transition: 'all 0.2s' }}
              onMouseOver={(e) => { e.currentTarget.style.color = '#4f46e5'; e.currentTarget.style.borderColor = '#c7d2fe'; e.currentTarget.style.backgroundColor = '#e0e7ff'; }}
              onMouseOut={(e) => { e.currentTarget.style.color = '#475569'; e.currentTarget.style.borderColor = '#cbd5e1'; e.currentTarget.style.backgroundColor = 'white'; }}
            >
              <Edit2 size={16} /> Düzenle
            </button>
          )}
          {!isApproved && !isEditing && (
             <LoadingButton
               onClick={() => onConfirm(item.question.id)}
               loading={saving}
               disabledReason={disabledReason}
               style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', padding: '0.5rem 1rem', background: '#16a34a', color: 'white', border: 'none', borderRadius: '0.5rem', fontSize: '0.875rem', fontWeight: 500 }}
             >
               <CheckCircle2 size={16} /> Onayla
             </LoadingButton>
          )}
        </div>
      </div>

      <div style={{ padding: '1.5rem', display: 'flex', flexDirection: 'column', gap: '1.5rem' }}>
        
        {/* Question Text */}
        <div style={{ display: 'flex', gap: '1rem', background: '#f8fafc', padding: '1rem', borderRadius: '0.75rem', border: '1px solid #e2e8f0' }}>
          <FileText size={20} color="#64748b" style={{ flexShrink: 0, marginTop: '0.1rem' }} />
          <div style={{ fontSize: '0.95rem', color: '#334155', lineHeight: 1.5, whiteSpace: 'pre-wrap' }}>
            {item.question.questionText.value || <span style={{ fontStyle: 'italic', color: '#94a3b8' }}>Soru metni bekleniyor.</span>}
          </div>
        </div>

        {isEditing ? (
          <div style={{ display: 'grid', gridTemplateColumns: '1fr', gap: '1.5rem', padding: '1.5rem', background: '#fafafa', borderRadius: '0.75rem', border: '1px solid #e5e5e5' }}>
            
            {/* Top Row: Max Score & Expected Answer */}
            <div className="rubric-card__top-fields" style={{ display: 'flex', gap: '1.5rem', alignItems: 'flex-start', flexWrap: 'wrap' }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem', minWidth: '13rem' }}>
                <label style={{ fontSize: '0.875rem', fontWeight: 600, color: '#334155' }}>Soru / cevap tipi</label>
                <select
                  aria-label={`Soru ${item.question.number} cevap tipi`}
                  value={draft.answerType}
                  onChange={(event) => onDraftChange(item.question.id, updateDraft(draft, { answerType: event.target.value as AnswerType }))}
                  style={{ padding: '0.75rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1', background: 'white', fontSize: '0.875rem' }}
                >
                  {Object.entries(answerTypeLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
                </select>
                <span style={{ fontSize: '0.75rem', color: '#64748b' }}>OCR bu seçime göre boşlukları, eşleri, işaretleri veya hücreleri ayrı okuyacaktır.</span>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem', width: '8rem', flexShrink: 0 }}>
                <label style={{ fontSize: '0.875rem', fontWeight: 600, color: '#334155' }}>Max Puan</label>
                <input
                  aria-label={`Soru ${item.question.number} maksimum puanı`}
                  type="number"
                  min="0"
                  step="0.25"
                  value={draft.maxScore}
                  onChange={(e) => onDraftChange(item.question.id, updateDraft(draft, { maxScore: e.target.value }))}
                  style={{ padding: '0.75rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1', outline: 'none', fontSize: '1rem', fontWeight: 500 }}
                />
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem', flex: 1 }}>
                <label style={{ fontSize: '0.875rem', fontWeight: 600, color: '#334155' }}>Beklenen Cevap</label>
                <textarea
                  aria-label={`Soru ${item.question.number} beklenen cevabı`}
                  rows={3}
                  value={draft.expectedAnswer}
                  onChange={(e) => onDraftChange(item.question.id, updateDraft(draft, { expectedAnswer: e.target.value }))}
                  placeholder="Bu soru için beklenen tam, doğru cevap nedir?"
                  style={{ padding: '0.75rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1', outline: 'none', fontSize: '0.875rem', resize: 'vertical' }}
                />
              </div>
            </div>

            <div style={{ height: '1px', background: '#e5e5e5', margin: '0.5rem 0' }} />

            {/* Criteria Section */}
            <div>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1rem' }}>
                <h4 style={{ margin: 0, fontSize: '1rem', fontWeight: 600, color: '#0f172a' }}>Değerlendirme Kriterleri · Toplam {criterionPointsTotal} puan</h4>
                <button
                  type="button"
                  onClick={addCriterion}
                  style={{ display: 'flex', alignItems: 'center', gap: '0.25rem', padding: '0.375rem 0.75rem', background: '#eff6ff', color: '#2563eb', border: 'none', borderRadius: '0.5rem', fontSize: '0.875rem', fontWeight: 500, cursor: 'pointer' }}
                >
                  <Plus size={16} /> Kriter Ekle
                </button>
              </div>

              <div style={{ display: 'grid', gap: '1rem' }}>
                {draft.criteria.length === 0 && (
                  <div style={{ padding: '1rem', textAlign: 'center', color: '#64748b', fontSize: '0.875rem', background: 'white', borderRadius: '0.5rem', border: '1px dashed #cbd5e1' }}>
                    Henüz kriter girilmedi. Puanlama için en az bir kriter eklemelisiniz.
                  </div>
                )}
                {draft.criteria.map((criterion) => (
                  <div key={criterion.id} className="rubric-card__criterion" style={{ display: 'grid', gridTemplateColumns: '1fr 3fr 100px auto', gap: '0.75rem', background: 'white', padding: '1rem', borderRadius: '0.5rem', border: '1px solid #e2e8f0', alignItems: 'start' }}>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                      <span style={{ fontSize: '0.75rem', color: '#64748b', fontWeight: 500 }}>Etiket / Başlık</span>
                      <input
                        aria-label={`${criterion.label || 'Kriter'} başlığı`}
                        type="text"
                        placeholder="Örn: Doğru formül"
                        value={criterion.label}
                        onChange={(e) => handleCriterionChange(criterion.id, { label: e.target.value })}
                        style={{ padding: '0.5rem', borderRadius: '0.375rem', border: '1px solid #cbd5e1', fontSize: '0.875rem' }}
                      />
                    </div>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                      <span style={{ fontSize: '0.75rem', color: '#64748b', fontWeight: 500 }}>Açıklama</span>
                      <textarea
                        aria-label={`${criterion.label || 'Kriter'} açıklaması`}
                        rows={2}
                        placeholder="Bu kriterin sağlanması için ne gerekiyor?"
                        value={criterion.description}
                        onChange={(e) => handleCriterionChange(criterion.id, { description: e.target.value })}
                        style={{ padding: '0.5rem', borderRadius: '0.375rem', border: '1px solid #cbd5e1', fontSize: '0.875rem', resize: 'vertical' }}
                      />
                    </div>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                      <span style={{ fontSize: '0.75rem', color: '#64748b', fontWeight: 500 }}>Puan</span>
                      <input
                        aria-label={`${criterion.label || 'Kriter'} puanı`}
                        type="number"
                        min="0"
                        step="0.25"
                        placeholder="0.0"
                        value={criterion.points}
                        onChange={(e) => handleCriterionChange(criterion.id, { points: e.target.value })}
                        style={{ padding: '0.5rem', borderRadius: '0.375rem', border: '1px solid #cbd5e1', fontSize: '0.875rem', textAlign: 'center' }}
                      />
                    </div>
                    <div style={{ display: 'flex', alignItems: 'center', height: '100%', paddingTop: '1.25rem' }}>
                      <button
                        type="button"
                        onClick={() => removeCriterion(criterion.id)}
                        aria-label={`${criterion.label || 'Kriter'} kriterini sil`}
                        style={{ background: 'transparent', border: 'none', color: '#ef4444', padding: '0.5rem', cursor: 'pointer', borderRadius: '0.25rem' }}
                        title="Kriteri Sil"
                        onMouseOver={(e) => e.currentTarget.style.backgroundColor = '#fee2e2'}
                        onMouseOut={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                      >
                        <Trash2 size={16} />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </div>

            <div style={{ height: '1px', background: '#e5e5e5', margin: '0.5rem 0' }} />

            {/* Advanced Rich Fields */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))', gap: '1.5rem' }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                <label style={{ fontSize: '0.875rem', fontWeight: 600, color: '#334155' }}>Kısmi Puan İpuçları (Opsiyonel)</label>
                <textarea
                  aria-label={`Soru ${item.question.number} kısmi puan ipuçları`}
                  rows={3}
                  value={draft.partialCreditHints}
                  onChange={(e) => onDraftChange(item.question.id, updateDraft(draft, { partialCreditHints: e.target.value }))}
                  placeholder="Gidiş yolu doğruysa verilecek puan..."
                  style={{ padding: '0.75rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1', outline: 'none', fontSize: '0.875rem', resize: 'vertical' }}
                />
                <span style={{ fontSize: '0.75rem', color: '#64748b' }}>
                  {draft.partialCreditHints.trim() ? optionalGuidanceText : optionalGuidanceEmptyText('partialCreditHints')}
                </span>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                <label style={{ fontSize: '0.875rem', fontWeight: 600, color: '#334155' }}>Sıfır Puan Koşulları (Opsiyonel)</label>
                <textarea
                  aria-label={`Soru ${item.question.number} sıfır puan koşulları`}
                  rows={3}
                  value={draft.zeroScoreConditions}
                  onChange={(e) => onDraftChange(item.question.id, updateDraft(draft, { zeroScoreConditions: e.target.value }))}
                  placeholder="Direkt 0 verilecek durumlar..."
                  style={{ padding: '0.75rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1', outline: 'none', fontSize: '0.875rem', resize: 'vertical' }}
                />
                <span style={{ fontSize: '0.75rem', color: '#64748b' }}>
                  {draft.zeroScoreConditions.trim() ? optionalGuidanceText : optionalGuidanceEmptyText('zeroScoreConditions')}
                </span>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                <label style={{ fontSize: '0.875rem', fontWeight: 600, color: '#334155' }}>Yaygın Yanlışlar (Opsiyonel)</label>
                <textarea
                  aria-label={`Soru ${item.question.number} yaygın yanlışlar`}
                  rows={3}
                  value={draft.commonMistakes}
                  onChange={(e) => onDraftChange(item.question.id, updateDraft(draft, { commonMistakes: e.target.value }))}
                  placeholder="Öğrencilerin sık yaptığı hatalar..."
                  style={{ padding: '0.75rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1', outline: 'none', fontSize: '0.875rem', resize: 'vertical' }}
                />
                <span style={{ fontSize: '0.75rem', color: '#64748b' }}>
                  {draft.commonMistakes.trim() ? optionalGuidanceText : optionalGuidanceEmptyText('commonMistakes')}
                </span>
              </div>
            </div>

            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '0.75rem', marginTop: '1rem' }}>
              <button
                onClick={() => setIsEditing(false)}
                style={{ padding: '0.625rem 1rem', background: 'white', color: '#475569', border: '1px solid #cbd5e1', borderRadius: '0.5rem', fontSize: '0.875rem', fontWeight: 500, cursor: 'pointer' }}
              >
                İptal
              </button>
              <LoadingButton
                onClick={handleSaveWrapper}
                loading={saving}
                disabledReason={disabledReason}
                style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', padding: '0.625rem 1.25rem', background: '#2563eb', color: 'white', border: 'none', borderRadius: '0.5rem', fontSize: '0.875rem', fontWeight: 500 }}
              >
                <Save size={16} /> Kaydet
              </LoadingButton>
              <LoadingButton
                onClick={handleConfirmWrapper}
                loading={saving}
                disabledReason={disabledReason}
                style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', padding: '0.625rem 1.25rem', background: '#0f766e', color: 'white', border: 'none', borderRadius: '0.5rem', fontSize: '0.875rem', fontWeight: 500 }}
              >
                <CheckCircle2 size={16} /> Kaydet ve Onayla
              </LoadingButton>
            </div>
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1.5rem' }}>
            {/* View Mode */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
              <span style={{ fontSize: '0.875rem', fontWeight: 600, color: '#475569', textTransform: 'uppercase', letterSpacing: '0.05em' }}>Beklenen Cevap</span>
              <div style={{ fontSize: '0.95rem', color: '#0f172a', background: '#f8fafc', padding: '1rem', borderRadius: '0.5rem', border: '1px solid #e2e8f0', whiteSpace: 'pre-wrap' }}>
                {draft.expectedAnswer || <span style={{ color: '#94a3b8', fontStyle: 'italic' }}>Henüz girilmedi.</span>}
              </div>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
              <span style={{ fontSize: '0.875rem', fontWeight: 600, color: '#475569', textTransform: 'uppercase', letterSpacing: '0.05em' }}>Değerlendirme Kriterleri ({draft.criteria.length}) · {criterionPointsTotal} puan</span>
              {draft.criteria.length === 0 ? (
                <div style={{ fontSize: '0.95rem', color: '#94a3b8', fontStyle: 'italic', background: '#f8fafc', padding: '1rem', borderRadius: '0.5rem', border: '1px solid #e2e8f0' }}>Henüz kriter girilmedi.</div>
              ) : (
                <div style={{ display: 'grid', gap: '0.5rem' }}>
                  {draft.criteria.map((c, i) => (
                    <div key={c.id} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', background: '#f8fafc', padding: '0.75rem 1rem', borderRadius: '0.5rem', border: '1px solid #e2e8f0' }}>
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                        <strong style={{ fontSize: '0.875rem', color: '#0f172a' }}>{i+1}. {c.label || 'İsimsiz Kriter'}</strong>
                        {c.description && <span style={{ fontSize: '0.875rem', color: '#475569' }}>{c.description}</span>}
                      </div>
                      <span style={{ fontSize: '0.875rem', fontWeight: 600, color: '#2563eb', background: '#eff6ff', padding: '0.25rem 0.5rem', borderRadius: '0.25rem', whiteSpace: 'nowrap' }}>
                        {c.points}p
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {(draft.partialCreditHints || draft.zeroScoreConditions || draft.commonMistakes) && (
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '1rem', marginTop: '0.5rem' }}>
                {draft.partialCreditHints && (
                  <div style={{ background: '#f0fdf4', border: '1px solid #bbf7d0', padding: '1rem', borderRadius: '0.5rem' }}>
                    <span style={{ fontSize: '0.75rem', fontWeight: 600, color: '#166534', textTransform: 'uppercase' }}>Kısmi Puan</span>
                    <div style={{ marginTop: '0.5rem', fontSize: '0.875rem', color: '#14532d', whiteSpace: 'pre-wrap' }}>{draft.partialCreditHints}</div>
                  </div>
                )}
                {draft.zeroScoreConditions && (
                  <div style={{ background: '#fef2f2', border: '1px solid #fecaca', padding: '1rem', borderRadius: '0.5rem' }}>
                    <span style={{ fontSize: '0.75rem', fontWeight: 600, color: '#991b1b', textTransform: 'uppercase' }}>Sıfır Puan Koşulu</span>
                    <div style={{ marginTop: '0.5rem', fontSize: '0.875rem', color: '#7f1d1d', whiteSpace: 'pre-wrap' }}>{draft.zeroScoreConditions}</div>
                  </div>
                )}
                {draft.commonMistakes && (
                  <div style={{ background: '#fffbeb', border: '1px solid #fde68a', padding: '1rem', borderRadius: '0.5rem' }}>
                    <span style={{ fontSize: '0.75rem', fontWeight: 600, color: '#b45309', textTransform: 'uppercase' }}>Yaygın Yanlışlar</span>
                    <div style={{ marginTop: '0.5rem', fontSize: '0.875rem', color: '#92400e', whiteSpace: 'pre-wrap' }}>{draft.commonMistakes}</div>
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {/* Validation Errors & Warnings */}
        {(teacherWarnings.length > 0 || item.validation.issues.length > 0) && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem', marginTop: '1rem' }}>
            {item.validation.issues.map((issue) => (
              <div key={`${issue.code}-${issue.message}`} style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', fontSize: '0.875rem', color: '#b91c1c', background: '#fef2f2', padding: '0.5rem 0.75rem', borderRadius: '0.375rem', border: '1px solid #fecaca' }}>
                <AlertCircle size={16} /> {issue.message}
              </div>
            ))}
            {teacherWarnings.map((warning) => (
              <div key={warning} style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', fontSize: '0.875rem', color: '#b45309', background: '#fffbeb', padding: '0.5rem 0.75rem', borderRadius: '0.375rem', border: '1px solid #fde68a' }}>
                <AlertCircle size={16} /> {warning}
              </div>
            ))}
          </div>
        )}

      </div>
    </article>
  );
}
