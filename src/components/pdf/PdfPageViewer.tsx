import { useState } from 'react';
import type { PointerEvent, ReactNode } from 'react';
import { resolveImageSrc } from './resolveImageSrc.ts';

type OverlayBox = {
  x: number;
  y: number;
  width: number;
  height: number;
};

type OverlayItem = {
  box: OverlayBox;
  label?: string | null;
};

type PdfPageViewerProps = {
  imagePath?: string | null;
  pageNumber: number;
  zoom: number;
  width?: number;
  height?: number;
  overlayBox?: OverlayBox | null;
  overlayBoxes?: OverlayBox[] | null;
  overlayItems?: OverlayItem[] | null;
  editable?: boolean;
  onOverlayChange?: (box: OverlayBox) => void;
  emptyState?: ReactNode;
  minimal?: boolean;
  fitToPage?: boolean;
  altText?: string;
};

function clampUnit(value: number) {
  return Math.min(1, Math.max(0, value));
}

function pointFromEvent(event: PointerEvent<HTMLElement>) {
  const rect = event.currentTarget.getBoundingClientRect();
  return {
    x: clampUnit((event.clientX - rect.left) / rect.width),
    y: clampUnit((event.clientY - rect.top) / rect.height),
  };
}

export function PdfPageViewer({ imagePath, pageNumber, zoom, width, height, overlayBox, overlayBoxes, overlayItems, editable, onOverlayChange, emptyState, minimal, fitToPage = false, altText }: PdfPageViewerProps) {
  const [dragStart, setDragStart] = useState<{ x: number; y: number } | null>(null);
  const [draftBox, setDraftBox] = useState<OverlayBox | null>(null);

  if (!imagePath) {
    return (
      <div style={{ minHeight: minimal ? 'auto' : '420px', height: '100%', display: 'grid', placeItems: 'center', border: minimal ? 'none' : '1px dashed #cbd5e1', borderRadius: minimal ? '0' : '16px', background: minimal ? 'transparent' : '#f8fafc' }}>
        <div>{emptyState ?? 'Sayfa önizlemesi bekleniyor.'}</div>
      </div>
    );
  }

  const src = resolveImageSrc(imagePath);
  const displayWidth = width ? Math.round(width * zoom) : undefined;
  const displayHeight = height ? Math.round(height * zoom) : undefined;
  const shownItems: OverlayItem[] = draftBox
    ? [{ box: draftBox }]
    : overlayItems?.length
      ? overlayItems
      : overlayBoxes?.length
        ? overlayBoxes.map((box) => ({ box }))
        : overlayBox
          ? [{ box: overlayBox }]
          : [];

  function updateDraft(event: PointerEvent<HTMLElement>) {
    if (!editable || !dragStart) return;
    const point = pointFromEvent(event);
    setDraftBox({
      x: Math.min(dragStart.x, point.x),
      y: Math.min(dragStart.y, point.y),
      width: Math.abs(point.x - dragStart.x),
      height: Math.abs(point.y - dragStart.y),
    });
  }

  return (
    <div style={{ overflow: 'auto', maxHeight: minimal ? '100%' : '72vh', border: minimal ? 'none' : '1px solid #dbe4ef', borderRadius: minimal ? '0' : '16px', background: minimal ? 'transparent' : '#0f172a', padding: minimal ? '0' : '1rem', width: '100%', maxWidth: '100%', height: '100%' }}>
      <div style={{ position: 'relative', display: 'grid', placeItems: 'center', minHeight: minimal ? 'auto' : '420px' }}>
        <div
          style={{
            position: 'relative',
            width: fitToPage ? 'min(100%, 920px)' : displayWidth ? `${displayWidth}px` : '100%',
            maxWidth: fitToPage ? '100%' : undefined,
            cursor: editable ? 'crosshair' : 'default',
          }}
          onPointerDown={(event) => {
            if (!editable) return;
            const point = pointFromEvent(event);
            setDragStart(point);
            setDraftBox({ x: point.x, y: point.y, width: 0, height: 0 });
            event.currentTarget.setPointerCapture(event.pointerId);
          }}
          onPointerMove={updateDraft}
          onPointerUp={(event) => {
            if (!editable || !dragStart) return;
            updateDraft(event);
            const point = pointFromEvent(event);
            const box = {
              x: Math.min(dragStart.x, point.x),
              y: Math.min(dragStart.y, point.y),
              width: Math.abs(point.x - dragStart.x),
              height: Math.abs(point.y - dragStart.y),
            };
            setDragStart(null);
            setDraftBox(null);
            if (box.width > 0.01 && box.height > 0.01) {
              onOverlayChange?.(box);
            }
          }}
        >
          <img
            src={src}
            alt={altText ?? `PDF sayfa ${pageNumber}`}
            style={{
              display: 'block',
              width: '100%',
              height: fitToPage ? 'auto' : displayHeight ? `${displayHeight}px` : 'auto',
              maxWidth: fitToPage ? '100%' : undefined,
              objectFit: 'contain',
              background: 'white',
              boxShadow: '0 24px 60px rgba(15, 23, 42, 0.35)',
              borderRadius: '12px',
            }}
          />
          {shownItems.map((item, index) => (
            <div
              key={`${item.box.x}-${item.box.y}-${item.box.width}-${item.box.height}-${index}`}
              aria-hidden="true"
              style={{
                position: 'absolute',
                left: `${item.box.x * 100}%`,
                top: `${item.box.y * 100}%`,
                width: `${item.box.width * 100}%`,
                height: `${item.box.height * 100}%`,
                pointerEvents: 'none',
              }}
            >
              <div
                style={{
                  position: 'absolute',
                  inset: 0,
                  border: '2px solid #ef4444',
                  background: 'rgba(239, 68, 68, 0.10)',
                  boxShadow: '0 0 0 1px rgba(255,255,255,0.35) inset',
                  borderRadius: '0.2rem',
                }}
              />
              {item.label && (
                <div
                  style={{
                    position: 'absolute',
                    left: 0,
                    top: 'calc(-1.25rem - 2px)',
                    maxWidth: '100%',
                    padding: '0.12rem 0.35rem',
                    borderRadius: '999px',
                    background: 'rgba(15, 23, 42, 0.9)',
                    color: 'white',
                    fontSize: '0.68rem',
                    lineHeight: 1.2,
                    whiteSpace: 'nowrap',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                  }}
                >
                  {item.label}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
