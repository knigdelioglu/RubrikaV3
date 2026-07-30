import type { SchoolClass, StudentScanBatch } from '../../api/types';

type ClassSelectorProps = {
  classes: SchoolClass[];
  batches?: StudentScanBatch[];
  classId: string;
  batchId?: string;
  onClassChange: (classId: string) => void;
  onBatchChange?: (batchId: string) => void;
  includeAllClasses?: boolean;
  includeAllBatches?: boolean;
  idPrefix?: string;
};

export function ClassSelector({
  classes,
  batches = [],
  classId,
  batchId = '',
  onClassChange,
  onBatchChange,
  includeAllClasses = true,
  includeAllBatches = true,
  idPrefix = 'class-filter',
}: ClassSelectorProps) {
  const visibleClasses = [...classes].sort((left, right) => (
    left.displayOrder - right.displayOrder || left.name.localeCompare(right.name, 'tr')
  ));
  const visibleBatches = batches.filter((batch) => !classId || batch.classId === classId);

  return (
    <div className="class-selector" aria-label="Sınıf ve öğrenci PDF paketi filtresi">
      <label htmlFor={`${idPrefix}-class`}>
        <span>Sınıf</span>
        <select
          id={`${idPrefix}-class`}
          value={classId}
          onChange={(event) => onClassChange(event.target.value)}
        >
          {includeAllClasses && <option value="">Tüm sınıflar</option>}
          {!includeAllClasses && <option value="">Sınıf seçin</option>}
          {visibleClasses.map((schoolClass) => (
            <option key={schoolClass.id} value={schoolClass.id}>
              {schoolClass.name}{schoolClass.status === 'archived' ? ' (Arşivlenmiş)' : ''}
            </option>
          ))}
        </select>
      </label>
      {onBatchChange && (
        <label htmlFor={`${idPrefix}-batch`}>
          <span>Öğrenci PDF paketi</span>
          <select
            id={`${idPrefix}-batch`}
            value={batchId}
            onChange={(event) => onBatchChange(event.target.value)}
          >
            {includeAllBatches && <option value="">Tüm paketler</option>}
            {!includeAllBatches && <option value="">Paket seçin</option>}
            {visibleBatches.map((batch) => (
              <option key={batch.id} value={batch.id}>{batch.displayName || batch.originalFileName}</option>
            ))}
          </select>
        </label>
      )}
    </div>
  );
}
