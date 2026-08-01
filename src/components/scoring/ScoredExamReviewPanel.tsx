import { useEffect, useMemo, useRef, useState } from 'react';
import { AlertTriangle, ChevronLeft, ChevronRight, Maximize2, ZoomIn, ZoomOut } from 'lucide-react';
import type { GradedExamReview } from '../../api/types';
import { resolveImageSrc } from '../pdf/resolveImageSrc';
import { annotationColors, calculateFitScale, calculatePreservedPageScale, formatReviewScore, scoreBreakdown } from './gradedExamReviewUi';

type ScoredExamReviewPanelProps = {
  review: GradedExamReview;
  projectId: string;
};

const PAPER_FIT_PADDING = 28;

export function ScoredExamReviewPanel({ review, projectId }: ScoredExamReviewPanelProps) {
  const viewportRef = useRef<HTMLElement | null>(null);
  const fitReferenceRef = useRef<{ width: number; height: number } | null>(null);
  const [pageIndex, setPageIndex] = useState(0);
  const [zoom, setZoom] = useState(1);
  const [fitScale, setFitScale] = useState(1);
  const page = review.pages[pageIndex];
  if (page && !fitReferenceRef.current) {
    fitReferenceRef.current = {
      width: page.width,
      height: page.height,
    };
  }
  const allAnnotations = useMemo(
    () => review.pages.flatMap((item, index) => item.annotations.map((annotation) => ({ ...annotation, pageIndex: index, pageNumber: item.pageNumber }))),
    [review.pages],
  );

  useEffect(() => {
    setPageIndex(0);
  }, [review.submissionId]);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const updateFit = () => {
      const reference = fitReferenceRef.current;
      if (!reference) return;
      const rect = viewport.getBoundingClientRect();
      setFitScale(calculateFitScale(rect.width, rect.height, reference.width, reference.height, PAPER_FIT_PADDING));
    };
    updateFit();
    const observer = new ResizeObserver(updateFit);
    observer.observe(viewport);
    return () => observer.disconnect();
  }, []);

  if (!page) return null;

  const referencePageWidth = fitReferenceRef.current?.width ?? page.width;
  const pageScale = calculatePreservedPageScale(fitScale, referencePageWidth, page.width);
  const renderedWidth = Math.max(1, page.width * pageScale * zoom);
  const renderedHeight = Math.max(1, page.height * pageScale * zoom);
  const fitCurrentPage = () => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const reference = {
      width: page.width,
      height: page.height,
    };
    fitReferenceRef.current = reference;
    const rect = viewport.getBoundingClientRect();
    setFitScale(calculateFitScale(rect.width, rect.height, reference.width, reference.height, PAPER_FIT_PADDING));
    setZoom(1);
  };

  return (
    <section style={{ height: '100%', minHeight: 0, display: 'grid', gridTemplateRows: 'auto minmax(0, 1fr)', background: '#e2e8f0' }}>
      <header style={{ padding: '0.65rem 0.85rem', background: '#fff', borderBottom: '1px solid #dbe4ef', display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '0.75rem' }}>
        <div style={{ minWidth: 0 }}>
          <h2 style={{ margin: 0, color: '#0f172a', fontSize: '0.95rem', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{review.studentDisplayName}</h2>
          <p style={{ margin: '0.18rem 0 0', color: '#64748b', fontSize: '0.72rem' }}>
            {[review.studentNumber && `No ${review.studentNumber}`, review.studentClassName].filter(Boolean).join(' · ') || 'Öğrenci bilgisi'}
          </p>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: '0.4rem' }}>
          <div style={{ padding: '0.38rem 0.65rem', borderRadius: '0.65rem', background: '#fef2f2', border: '1px solid #fecaca', color: '#991b1b', textAlign: 'right' }}>
            <small style={{ display: 'block', fontSize: '0.58rem', fontWeight: 800, letterSpacing: '0.06em', textTransform: 'uppercase' }}>Model toplamı</small>
            <strong style={{ fontSize: '0.95rem' }}>{formatReviewScore(review.modelTotalScore)} <span style={{ color: '#94a3b8', fontWeight: 600 }}>/</span> {formatReviewScore(review.maxTotalScore)}</strong>
          </div>
          <button type="button" aria-label="Uzaklaştır" onClick={() => setZoom((value) => Math.max(0.65, value - 0.1))} style={toolbarButtonStyle}><ZoomOut size={16} /></button>
          <button type="button" onClick={fitCurrentPage} title="Kâğıdı ekrana sığdır ve yeni ölçek referansı yap" style={{ ...toolbarButtonStyle, width: 'auto', padding: '0 0.55rem', gap: '0.3rem' }}><Maximize2 size={14} /> Sığdır</button>
          <span style={{ minWidth: '2.8rem', textAlign: 'center', color: '#475569', fontSize: '0.72rem', fontWeight: 800 }}>%{Math.round(zoom * 100)}</span>
          <button type="button" aria-label="Yakınlaştır" onClick={() => setZoom((value) => Math.min(2.2, value + 0.1))} style={toolbarButtonStyle}><ZoomIn size={16} /></button>
        </div>
      </header>

      <div style={{ minHeight: 0, display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) 224px' }}>
        <main ref={viewportRef} style={{ minWidth: 0, minHeight: 0, overflow: 'auto', padding: '0.85rem', background: '#cbd5e1', display: 'grid', placeItems: 'center' }}>
          <div style={{ width: `${renderedWidth}px`, height: `${renderedHeight}px`, position: 'relative', flexShrink: 0, background: 'white', boxShadow: '0 16px 42px rgba(15, 23, 42, 0.24)', transition: 'width 120ms ease, height 120ms ease' }}>
            <img src={resolveImageSrc(page.imagePath, projectId)} alt={`Puanlı sınav sayfası ${page.pageNumber}`} style={{ display: 'block', width: '100%', height: '100%', objectFit: 'fill' }} />
            {page.annotations.map((annotation) => {
              const colors = annotationColors(annotation);
              const breakdown = scoreBreakdown(annotation);
              const annotationWidth = annotation.width * renderedWidth;
              const annotationHeight = annotation.height * renderedHeight;
              const scoreTextLength = annotation.modelScore === null || annotation.modelScore === undefined
                ? 7
                : `${formatReviewScore(annotation.modelScore)}/${formatReviewScore(annotation.maxScore)}`.length;
              const breakdownTextLength = breakdown?.length ?? 0;
              const markerQuestionSize = Math.max(16, Math.min(34, annotationHeight * 0.28, annotationWidth * 0.18));
              const markerScoreFontSize = Math.max(11, Math.min(30, annotationHeight * 0.28, (annotationWidth * 0.7) / Math.max(scoreTextLength, 1)));
              const markerBreakdownFontSize = breakdown
                ? Math.max(9, Math.min(18, annotationHeight * 0.16, (annotationWidth * 0.78) / Math.max(breakdownTextLength * 0.52, 1)))
                : 0;
              return (
                <div key={annotation.recordId}>
                  <div
                    title={`Soru ${annotation.questionNumber}: model puanı ${annotation.label}`}
                    style={{
                      position: 'absolute', left: `${annotation.x * 100}%`, top: `${annotation.y * 100}%`, width: `${annotation.width * 100}%`, minHeight: `${annotation.height * 100}%`,
                      boxSizing: 'border-box', display: 'grid', gridTemplateColumns: 'auto 1fr', alignItems: 'center', gap: '0.2rem', padding: '0.18rem 0.26rem', borderRadius: '0.45rem', border: `2px solid ${colors.border}`,
                      background: annotation.status === 'needs_review' ? 'rgba(255, 247, 237, 0.58)' : 'rgba(255, 255, 255, 0.48)',
                      color: colors.color, boxShadow: '0 2px 7px rgba(15, 23, 42, 0.14)', lineHeight: 1, whiteSpace: 'normal', overflow: 'hidden', pointerEvents: 'none',
                    }}
                  >
                    <span style={{ width: `${markerQuestionSize}px`, height: `${markerQuestionSize}px`, borderRadius: '999px', display: 'grid', placeItems: 'center', background: 'rgba(220, 38, 38, 0.82)', color: 'white', fontSize: `${Math.max(5, markerQuestionSize * 0.43)}px`, fontWeight: 900, flexShrink: 0 }}>S{annotation.questionNumber}</span>
                    <span style={{ minWidth: 0, textAlign: 'center' }}>
                      {breakdown && <small style={{ display: 'block', maxWidth: '100%', marginBottom: '0.1rem', color: '#475569', fontSize: `${markerBreakdownFontSize}px`, fontWeight: 800, lineHeight: 1.05, overflowWrap: 'anywhere' }}>{breakdown}</small>}
                      {annotation.modelScore === null || annotation.modelScore === undefined ? (
                        <strong style={{ color: '#c2410c', fontSize: `${markerScoreFontSize}px` }}>Kontrol</strong>
                      ) : (
                        <strong style={{ fontSize: `${markerScoreFontSize}px`, letterSpacing: '-0.03em' }}>
                          <span style={{ color: '#1d4ed8' }}>{formatReviewScore(annotation.modelScore)}</span>
                          <span style={{ color: '#64748b', margin: '0 0.08rem' }}>/</span>
                          <span style={{ color: '#dc2626' }}>{formatReviewScore(annotation.maxScore)}</span>
                        </strong>
                      )}
                    </span>
                  </div>

                  {annotation.needsReview && annotation.reviewGuidance.length > 0 && (
                    <div style={{
                      position: 'absolute', top: `${annotation.y * 100}%`,
                      ...(annotation.x >= 0.5 ? { left: 'calc(100% + 0.7rem)' } : { right: 'calc(100% + 0.7rem)' }),
                      width: '8.6rem', boxSizing: 'border-box', padding: '0.48rem 0.55rem', borderRadius: '0.55rem',
                      background: 'rgba(248, 250, 252, 0.86)', border: '1px solid rgba(148, 163, 184, 0.9)', color: '#334155',
                      boxShadow: '0 4px 14px rgba(15, 23, 42, 0.12)', fontSize: '0.62rem', lineHeight: 1.35, pointerEvents: 'none',
                    }}>
                      <strong style={{ display: 'block', marginBottom: '0.2rem', color: '#c2410c' }}>S{annotation.questionNumber} · Neyi kontrol etmeli?</strong>
                      {annotation.reviewGuidance.slice(0, 2).map((guidance) => <div key={guidance}>• {guidance}</div>)}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </main>

        <aside style={{ minHeight: 0, overflow: 'auto', background: 'white', borderLeft: '1px solid #dbe4ef', padding: '0.65rem' }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '0.3rem', marginBottom: '0.65rem' }}>
            <button type="button" aria-label="Önceki sayfa" disabled={pageIndex === 0} onClick={() => setPageIndex((value) => Math.max(0, value - 1))} style={{ ...toolbarButtonStyle, opacity: pageIndex === 0 ? 0.35 : 1 }}><ChevronLeft size={16} /></button>
            <strong style={{ color: '#334155', fontSize: '0.72rem' }}>Sayfa {pageIndex + 1}/{review.pages.length}</strong>
            <button type="button" aria-label="Sonraki sayfa" disabled={pageIndex === review.pages.length - 1} onClick={() => setPageIndex((value) => Math.min(review.pages.length - 1, value + 1))} style={{ ...toolbarButtonStyle, opacity: pageIndex === review.pages.length - 1 ? 0.35 : 1 }}><ChevronRight size={16} /></button>
          </div>

          {review.needsReviewCount > 0 && (
            <div style={{ display: 'flex', gap: '0.4rem', padding: '0.5rem', borderRadius: '0.55rem', background: '#fffbeb', border: '1px solid #fde68a', color: '#92400e', fontSize: '0.66rem', lineHeight: 1.35, marginBottom: '0.6rem' }}>
              <AlertTriangle size={14} style={{ flexShrink: 0 }} />
              <span><strong>{review.needsReviewCount} kontrol</strong><br />“Kontrol” sıfır değildir.</span>
            </div>
          )}

          <div style={{ display: 'grid', gap: '0.32rem' }}>
            {allAnnotations.map((annotation) => {
              const colors = annotationColors(annotation);
              const activePage = pageIndex === annotation.pageIndex;
              return (
                <button key={annotation.recordId} type="button" onClick={() => setPageIndex(annotation.pageIndex)} style={{ display: 'grid', gridTemplateColumns: '1.7rem minmax(0, 1fr)', alignItems: 'center', gap: '0.45rem', padding: '0.42rem 0.48rem', borderRadius: '0.55rem', border: activePage ? `1px solid ${colors.border}` : '1px solid #e2e8f0', background: activePage ? colors.background : '#fff', color: '#0f172a', cursor: 'pointer', textAlign: 'left' }}>
                  <span style={{ width: '1.65rem', height: '1.65rem', borderRadius: '0.5rem', display: 'grid', placeItems: 'center', background: activePage ? colors.border : '#f1f5f9', color: activePage ? 'white' : '#475569', fontSize: '0.68rem', fontWeight: 900 }}>{annotation.questionNumber}</span>
                  <span style={{ minWidth: 0, display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', gap: '0.35rem' }}>
                    <span style={{ fontSize: '0.58rem', color: '#94a3b8' }}>s.{annotation.pageNumber}</span>
                    <strong style={{ fontSize: '0.86rem', letterSpacing: '-0.02em' }}><span style={{ color: '#1d4ed8' }}>{formatReviewScore(annotation.modelScore)}</span><small style={{ color: '#dc2626', fontSize: '0.58rem' }}>/{formatReviewScore(annotation.maxScore)}</small></strong>
                  </span>
                </button>
              );
            })}
          </div>

          {review.unplacedScores.length > 0 && (
            <div style={{ marginTop: '0.65rem', paddingTop: '0.65rem', borderTop: '1px solid #e2e8f0' }}>
              <strong style={{ color: '#9a3412', fontSize: '0.66rem' }}>Konumu eksik</strong>
              {review.unplacedScores.map((score) => (
                <div key={score.recordId} title={score.reason} style={{ marginTop: '0.3rem', padding: '0.42rem', borderRadius: '0.5rem', background: '#fff7ed', color: '#9a3412', fontSize: '0.65rem', display: 'flex', justifyContent: 'space-between', gap: '0.4rem' }}>
                  <span>S{score.questionNumber}</span><strong>{formatReviewScore(score.modelScore)}/{formatReviewScore(score.maxScore)}</strong>
                </div>
              ))}
            </div>
          )}
        </aside>
      </div>
    </section>
  );
}

const toolbarButtonStyle = {
  height: '2rem', width: '2rem', display: 'inline-flex', alignItems: 'center', justifyContent: 'center', borderRadius: '0.55rem', border: '1px solid #cbd5e1', background: 'white', color: '#334155', cursor: 'pointer', fontSize: '0.68rem', fontWeight: 750,
} as const;
