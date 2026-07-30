import { ChevronLeft, ChevronRight } from 'lucide-react';

type PageNavigationProps = {
  currentPage: number;
  totalPages: number;
  onChange: (page: number) => void;
  dark?: boolean;
};

export function PageNavigation({ currentPage, totalPages, onChange, dark = false }: PageNavigationProps) {
  if (totalPages <= 1) {
    return (
      <div className={`page-navigation ${dark ? 'is-dark' : ''}`} aria-label="PDF sayfa konumu">
        <strong aria-live="polite">Sayfa 1 / 1</strong>
      </div>
    );
  }

  return (
    <div className={`page-navigation ${dark ? 'is-dark' : ''}`} aria-label="PDF sayfa gezinme">
      <button
        type="button"
        className={`viewer-icon-button ${dark ? 'is-dark' : ''}`}
        onClick={() => onChange(Math.max(1, currentPage - 1))}
        disabled={currentPage <= 1}
        aria-label="Önceki PDF sayfası"
        title="Önceki sayfa"
      >
        <ChevronLeft size={18} aria-hidden="true" />
      </button>
      <strong aria-live="polite">Sayfa {currentPage} / {totalPages}</strong>
      <button
        type="button"
        className={`viewer-icon-button ${dark ? 'is-dark' : ''}`}
        onClick={() => onChange(Math.min(totalPages, currentPage + 1))}
        disabled={currentPage >= totalPages}
        aria-label="Sonraki PDF sayfası"
        title="Sonraki sayfa"
      >
        <ChevronRight size={18} aria-hidden="true" />
      </button>
    </div>
  );
}
