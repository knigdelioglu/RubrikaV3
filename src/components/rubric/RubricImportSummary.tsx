import type { ImportRubricJsonOutput, RubricStateSnapshot, RubricValidationReport } from '../../api/types';
import { teacherFacingRubricWarnings } from '../../utils/rubricWarnings';

type RubricImportSummaryProps = {
  rubricState?: RubricStateSnapshot | null;
  validation?: RubricValidationReport | null;
  importResult?: ImportRubricJsonOutput | null;
};

export function RubricImportSummary({ rubricState, validation, importResult }: RubricImportSummaryProps) {
  const warnings = teacherFacingRubricWarnings([
    ...(rubricState?.warnings ?? []),
    ...(validation?.warnings ?? []),
    ...(importResult?.warnings ?? []),
  ]);

  return (
    <section style={{ padding: '1rem', borderRadius: '16px', background: 'rgba(15, 23, 42, 0.05)', border: '1px solid rgba(15, 23, 42, 0.08)' }}>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '1rem' }}>
        <div>
          <div style={{ fontSize: '0.85rem', color: '#64748b' }}>Durum</div>
          <div style={{ fontWeight: 700 }}>{rubricState?.summary ?? 'Rubrik özeti yok.'}</div>
        </div>
        <div>
          <div style={{ fontSize: '0.85rem', color: '#64748b' }}>İçe aktarılan</div>
          <div style={{ fontWeight: 700 }}>{importResult?.importedCount ?? rubricState?.importedCount ?? 0}</div>
        </div>
        <div>
          <div style={{ fontSize: '0.85rem', color: '#64748b' }}>Eksik</div>
          <div style={{ fontWeight: 700 }}>{importResult?.missingCount ?? rubricState?.missingCount ?? 0}</div>
        </div>
        <div>
          <div style={{ fontSize: '0.85rem', color: '#64748b' }}>Geçersiz</div>
          <div style={{ fontWeight: 700 }}>{importResult?.invalidCount ?? rubricState?.invalidCount ?? 0}</div>
        </div>
        <div>
          <div style={{ fontSize: '0.85rem', color: '#64748b' }}>Doğrulama</div>
          <div style={{ fontWeight: 700 }}>{validation ? (validation.valid ? 'Geçerli' : 'Sorun var') : 'Henüz çalıştırılmadı'}</div>
        </div>
      </div>

      {validation && validation.blockingQuestions.length > 0 && (
        <div style={{ marginTop: '1rem', padding: '0.75rem 0.9rem', borderRadius: '12px', background: '#fef3c7', color: '#92400e' }}>
          Onay engelli sorular: {validation.blockingQuestions.map((number) => `Soru ${number}`).join(', ')}
        </div>
      )}

      {warnings.length > 0 && (
        <ul style={{ marginTop: '1rem', marginBottom: 0, paddingLeft: '1.25rem', color: '#9a3412' }}>
          {warnings.slice(0, 8).map((warning, index) => (
            <li key={`${warning}-${index}`}>{warning}</li>
          ))}
        </ul>
      )}
    </section>
  );
}
