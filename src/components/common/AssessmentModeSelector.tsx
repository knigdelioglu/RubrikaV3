import { ClipboardCheck, FileText, Headphones, Mic2 } from 'lucide-react';
import { useLocation, useNavigate } from 'react-router-dom';
import { getAssessmentMode, getAssessmentModePath, type AssessmentMode } from '../../app/assessmentMode';
import { useProjectContext } from '../../state/useProjectContext';

export function AssessmentModeSelector() {
  const navigate = useNavigate();
  const location = useLocation();
  const { projectId } = useProjectContext();
  const activeMode = getAssessmentMode(location.pathname, location.search);

  const selectMode = (mode: AssessmentMode) => {
    if (mode !== activeMode) navigate(getAssessmentModePath(mode, projectId));
  };

  return (
    <div className="assessment-mode-selector" role="radiogroup" aria-label="Sınav türü">
      <button
        type="button"
        data-project-write="false"
        className={activeMode === 'written' ? 'is-active' : ''}
        role="radio"
        aria-checked={activeMode === 'written'}
        onClick={() => selectMode('written')}
      >
        <FileText size={15} aria-hidden="true" />
        Yazılı Sınav
      </button>
      <button
        type="button"
        data-project-write="false"
        className={activeMode === 'listening' ? 'is-active' : ''}
        role="radio"
        aria-checked={activeMode === 'listening'}
        onClick={() => selectMode('listening')}
      >
        <Headphones size={15} aria-hidden="true" />
        Dinleme Sınavı
      </button>
      <button
        type="button"
        data-project-write="false"
        className={activeMode === 'speaking' ? 'is-active' : ''}
        role="radio"
        aria-checked={activeMode === 'speaking'}
        onClick={() => selectMode('speaking')}
      >
        <Mic2 size={15} aria-hidden="true" />
        Konuşma Sınavı
      </button>
      <button
        type="button"
        data-project-write="false"
        className={activeMode === 'performance' ? 'is-active' : ''}
        role="radio"
        aria-checked={activeMode === 'performance'}
        onClick={() => selectMode('performance')}
      >
        <ClipboardCheck size={15} aria-hidden="true" />
        Performans Görevi
      </button>
    </div>
  );
}
