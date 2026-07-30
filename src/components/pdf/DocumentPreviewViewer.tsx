import { useEffect, useState } from 'react';
import type { PdfPagePreview } from '../../api/types';
import { PageNavigation } from './PageNavigation';
import { PdfPageViewer } from './PdfPageViewer';
import { ZoomControls } from './ZoomControls';

type DocumentPreviewViewerProps = {
  documentName: string;
  previews: PdfPagePreview[];
  initialPage: number;
  onPageChange: (page: number) => void;
};

export function DocumentPreviewViewer({
  documentName,
  previews,
  initialPage,
  onPageChange,
}: DocumentPreviewViewerProps) {
  const [currentPage, setCurrentPage] = useState(initialPage);
  const [zoom, setZoom] = useState(1);
  const [fitToPage, setFitToPage] = useState(true);
  const totalPages = Math.max(previews.length, 1);
  const currentPreview = previews.find((preview) => preview.pageNumber === currentPage) ?? previews[0];

  useEffect(() => {
    const nextPage = Math.min(Math.max(initialPage, 1), totalPages);
    setCurrentPage(nextPage);
  }, [initialPage, totalPages]);

  function changePage(page: number) {
    const nextPage = Math.min(Math.max(page, 1), totalPages);
    setCurrentPage(nextPage);
    onPageChange(nextPage);
  }

  function changeZoom(delta: number) {
    setFitToPage(false);
    setZoom((current) => Math.min(2.5, Math.max(0.5, Number((current + delta).toFixed(2)))));
  }

  return (
    <section
      className="document-viewer"
      aria-label={`${documentName} sayfa inceleme alanı`}
      tabIndex={0}
      onKeyDown={(event) => {
        if (event.key === 'ArrowLeft') changePage(currentPage - 1);
        if (event.key === 'ArrowRight') changePage(currentPage + 1);
      }}
    >
      <div className="document-viewer__toolbar">
        <div className="document-viewer__identity">
          <strong title={documentName}>{documentName}</strong>
          <span>İncelemeye hazır</span>
        </div>
        <div className="document-viewer__controls" aria-label="PDF görüntüleme kontrolleri">
          <ZoomControls
            zoom={zoom}
            fitToPage={fitToPage}
            onZoomOut={() => changeZoom(-0.1)}
            onZoomIn={() => changeZoom(0.1)}
            onFit={() => setFitToPage(true)}
            onReset={() => {
              setFitToPage(false);
              setZoom(1);
            }}
          />
          <PageNavigation currentPage={currentPage} totalPages={totalPages} onChange={changePage} dark />
        </div>
      </div>
      <div className="document-viewer__canvas">
        <PdfPageViewer
          imagePath={currentPreview?.imagePath}
          pageNumber={currentPage}
          zoom={zoom}
          fitToPage={fitToPage}
          width={currentPreview?.width}
          height={currentPreview?.height}
          altText={`${documentName}, PDF sayfa ${currentPage}`}
          emptyState="Bu sayfanın önizleme görüntüsü bulunamadı."
        />
      </div>
      <p className="document-viewer__keyboard-hint">Sayfalar arasında sol ve sağ ok tuşlarıyla da gezinebilirsiniz.</p>
    </section>
  );
}
