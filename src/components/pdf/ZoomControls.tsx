import { Maximize2, Minus, Plus } from 'lucide-react';

type ZoomControlsProps = {
  zoom: number;
  fitToPage?: boolean;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onFit?: () => void;
  onReset: () => void;
};

export function ZoomControls({ zoom, fitToPage = false, onZoomIn, onZoomOut, onFit, onReset }: ZoomControlsProps) {
  return (
    <div className="zoom-controls" aria-label="PDF yakınlaştırma kontrolleri">
      <button type="button" data-project-write="false" className="viewer-icon-button" onClick={onZoomOut} aria-label="PDF’ten uzaklaştır" title="Uzaklaştır">
        <Minus size={17} aria-hidden="true" />
      </button>
      <span className="zoom-controls__value" aria-live="polite">{fitToPage ? 'Sığdır' : `%${(zoom * 100).toFixed(0)}`}</span>
      <button type="button" data-project-write="false" className="viewer-icon-button" onClick={onZoomIn} aria-label="PDF’e yakınlaştır" title="Yakınlaştır">
        <Plus size={17} aria-hidden="true" />
      </button>
      {onFit && (
        <button type="button" data-project-write="false" className="viewer-text-button" onClick={onFit} aria-label="PDF sayfasını alana sığdır">
          <Maximize2 size={15} aria-hidden="true" /> Sayfaya sığdır
        </button>
      )}
      <button type="button" data-project-write="false" className="viewer-text-button" onClick={onReset} aria-label="PDF yakınlaştırmasını yüzde yüze getir">
        %100
      </button>
    </div>
  );
}
