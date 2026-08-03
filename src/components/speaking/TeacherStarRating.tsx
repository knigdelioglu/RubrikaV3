import { useState } from 'react';
import type { SpeakingPerformanceLevel } from '../../api/types';
import { getStarScore, levelToStarCount, STAR_DESCRIPTIONS } from '../../pages/speechExamUi';

export type TeacherStarRatingProps = {
  criterionId: string;
  criterionLabel: string;
  maxScore: number;
  currentLevel?: SpeakingPerformanceLevel | null;
  currentScore?: number | null;
  onSelectLevel: (level: SpeakingPerformanceLevel) => void;
  onClear: () => void;
  disabled?: boolean;
};

export function TeacherStarRating({
  criterionLabel,
  maxScore,
  currentLevel,
  currentScore,
  onSelectLevel,
  onClear,
  disabled = false,
}: TeacherStarRatingProps) {
  const [hoverStar, setHoverStar] = useState<number | null>(null);

  const selectedStars = levelToStarCount(currentLevel);
  const displayStars = hoverStar ?? selectedStars;
  const isPerformanceNotShown = currentLevel === 'performance_not_shown';
  const isNotObserved = currentLevel === 'not_observed' || currentLevel === 'not_evaluated' || (!currentLevel && (currentScore === undefined || currentScore === null));

  const calculatedScore = currentScore ?? getStarScore(currentLevel ?? 'not_evaluated', maxScore);

  function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    if (disabled) return;
    if (event.key === '1') {
      event.preventDefault();
      onSelectLevel('developing');
    } else if (event.key === '2') {
      event.preventDefault();
      onSelectLevel('good');
    } else if (event.key === '3') {
      event.preventDefault();
      onSelectLevel('very_good');
    } else if (event.key === 'Escape') {
      event.preventDefault();
      onClear();
    } else if (event.key === 'ArrowRight' || event.key === 'ArrowUp') {
      event.preventDefault();
      const next = Math.min(3, selectedStars + 1);
      if (next === 1) onSelectLevel('developing');
      if (next === 2) onSelectLevel('good');
      if (next === 3) onSelectLevel('very_good');
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowDown') {
      event.preventDefault();
      const prev = Math.max(1, selectedStars - 1);
      if (prev === 1) onSelectLevel('developing');
      if (prev === 2) onSelectLevel('good');
      if (prev === 3) onSelectLevel('very_good');
    }
  }

  return (
    <div className="teacher-star-rating" onKeyDown={handleKeyDown} tabIndex={disabled ? -1 : 0}>
      <div
        className="teacher-star-rating__stars"
        role="radiogroup"
        aria-label={`${criterionLabel} yıldız değerlendirmesi`}
      >
        {[1, 2, 3].map((starIndex) => {
          const isFilled = starIndex <= displayStars;
          const starInfo = STAR_DESCRIPTIONS[starIndex]!;
          const isCurrentSelected = selectedStars === starIndex && !isPerformanceNotShown;

          return (
            <button
              type="button"
              data-project-write="true"
              key={starIndex}
              className={`teacher-star-rating__star ${isFilled ? 'is-filled' : ''}`}
              disabled={disabled}
              role="radio"
              aria-checked={isCurrentSelected}
              aria-label={`${starIndex} yıldız - ${starInfo.label}`}
              title={`${starIndex} yıldız: ${starInfo.label} (${getStarScore(starInfo.level, maxScore)}/${maxScore})`}
              onMouseEnter={() => setHoverStar(starIndex)}
              onMouseLeave={() => setHoverStar(null)}
              onClick={() => onSelectLevel(starInfo.level)}
            >
              {isFilled ? '★' : '☆'}
            </button>
          );
        })}
      </div>

      <div className="teacher-star-rating__summary">
        {isPerformanceNotShown ? (
          <span className="teacher-star-rating__badge is-zero">
            Performans gösterilmedi · 0/{maxScore}
          </span>
        ) : isNotObserved || calculatedScore === null ? (
          <span className="teacher-star-rating__badge is-empty">
            Değerlendirilmedi
          </span>
        ) : (
          <span className="teacher-star-rating__badge is-selected">
            {selectedStars === 3 ? 'Çok iyi' : selectedStars === 2 ? 'İyi' : 'Geliştirilebilir'} · {calculatedScore}/{maxScore}
          </span>
        )}
      </div>

      <div className="teacher-star-rating__actions">
        {(selectedStars > 0 || isPerformanceNotShown) && (
          <button
            type="button"
            data-project-write="true"
            className="teacher-star-rating__action-btn"
            disabled={disabled}
            onClick={() => onClear()}
          >
            Seçimi temizle
          </button>
        )}

        <button
          type="button"
          data-project-write="true"
          className="teacher-star-rating__action-btn"
          disabled={disabled}
          onClick={() => onSelectLevel('not_observed')}
        >
          Değerlendirilemedi
        </button>

        {!isPerformanceNotShown && (
          <button
            type="button"
            data-project-write="true"
            className="teacher-star-rating__action-btn is-danger-text"
            disabled={disabled}
            onClick={() => onSelectLevel('performance_not_shown')}
          >
            Performans gösterilmedi (0)
          </button>
        )}
      </div>
    </div>
  );
}
