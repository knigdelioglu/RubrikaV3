import { useEffect, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type { QuestionAnswerRegion, QuestionAnswerTemplate, StudentAnswerOcrCropBBox, StudentIdentityCropTemplate } from '../api/types';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { LoadingButton } from '../components/common/LoadingButton';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { PageNavigation } from '../components/pdf/PageNavigation';
import { PdfPageViewer } from '../components/pdf/PdfPageViewer';
import { useProjectContext } from '../state/useProjectContext';
import { getOcrPreprocessModeLabel, getStudentAnswerCropTemplateSummary } from './studentAnswerOcrUi';
import type { OcrImagePreprocessMode } from '../api/types';
import { projectStudentOperationsPath } from '../app/projectRoutes';
import { filterStudentSubmissions } from './studentOperations';

function templateItemKey(item: QuestionAnswerTemplate) {
  return item.questionId;
}

export function CropTemplatePage() {
  const [searchParams] = useSearchParams();
  const { projectId, projectPath, isResolving } = useProjectContext();
  const classId = searchParams.get('classId') || '';
  const batchId = searchParams.get('batchId') || '';
  const queryClient = useQueryClient();
  const [error, setError] = useState<AppError | null>(null);
  const [selectedQuestionId, setSelectedQuestionId] = useState<string | null>(null);
  const [templatePageIndex, setTemplatePageIndex] = useState(0);
  const [templateDrafts, setTemplateDrafts] = useState<Record<string, QuestionAnswerTemplate>>({});
  const [selectedRegionOrder, setSelectedRegionOrder] = useState(0);
  const [mode, setMode] = useState<'answers' | 'identity'>('answers');
  const [identityDraft, setIdentityDraft] = useState<StudentIdentityCropTemplate | null>(null);
  const [previewMode, setPreviewMode] = useState<OcrImagePreprocessMode>('handwriting_enhanced');

  const { data: project } = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: !!projectId,
  });

  const visibleSubmissions = filterStudentSubmissions(project?.studentSubmissions ?? [], classId, batchId);
  const firstSubmission = visibleSubmissions[0] ?? null;
  const templatePageNumber = firstSubmission?.pageNumbers[templatePageIndex] ?? null;
  const templatePageCount = firstSubmission?.pageNumbers.length ?? 0;
  const studentScanDocumentId = firstSubmission?.documentId ?? project?.studentScanDocumentId ?? null;

  const { data: templatePreview } = useQuery({
    queryKey: ['student-answer-template-preview', projectId, studentScanDocumentId, templatePageNumber],
    queryFn: () => commands.getPdfPagePreview({ projectId: projectId ?? '', documentId: studentScanDocumentId ?? '', pageNumber: templatePageNumber ?? 1 }),
    enabled: !!projectId && !!studentScanDocumentId && !!templatePageNumber,
  });

  const { data: cleanTemplatePreview } = useQuery({
    queryKey: ['student-answer-template-clean-preview', projectId, templatePreview?.imagePath, previewMode],
    queryFn: () => commands.preprocessOcrImage({
      projectId: projectId ?? '',
      imagePath: templatePreview?.imagePath ?? '',
      mode: previewMode,
    }),
    enabled: !!projectId && !!templatePreview?.imagePath && previewMode !== 'original',
  });

  const saveTemplateMutation = useMutation({
    mutationFn: (templates: QuestionAnswerTemplate[]) =>
      commands.saveStudentAnswerCropTemplate({ projectId, templates }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
    },
    onError: (err: AppError) => setError(err),
  });

  const saveIdentityTemplateMutation = useMutation({
    mutationFn: (template: StudentIdentityCropTemplate) =>
      commands.saveStudentIdentityCropTemplate({ projectId, template }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
    },
    onError: (err: AppError) => setError(err),
  });

  useEffect(() => {
    if (!project) return;
    setSelectedQuestionId((current) => current ?? project.questions[0]?.id ?? null);
    setTemplateDrafts(Object.fromEntries(project.studentAnswerCropTemplate.templates.map((item) => [templateItemKey(item), item])));
    setIdentityDraft(project.studentIdentityCropTemplate ?? null);
  }, [project]);

  useEffect(() => {
    if (templatePageCount === 0) return;
    setTemplatePageIndex((current) => Math.min(current, templatePageCount - 1));
  }, [templatePageCount]);

  const templateItems = Object.values(templateDrafts);
  const selectedQuestion = project?.questions.find((question) => question.id === selectedQuestionId) ?? project?.questions[0] ?? null;
  const selectedTemplate = selectedQuestion ? templateDrafts[selectedQuestion.id] : null;
  const selectedRegion = selectedTemplate?.regions.find((region) => region.order === selectedRegionOrder)
    ?? (selectedRegionOrder === 0 ? selectedTemplate?.regions[0] : null)
    ?? null;
  const selectedTemplateOnCurrentPage = selectedRegion?.pageOffset === templatePageIndex ? selectedRegion : null;

  useEffect(() => {
    if (!selectedTemplate || templatePageCount === 0) return;
    setSelectedRegionOrder(selectedRegion?.order ?? 0);
    setTemplatePageIndex(Math.min(selectedRegion?.pageOffset ?? 0, templatePageCount - 1));
  }, [selectedQuestionId, selectedTemplate, selectedRegion, templatePageCount]);

  if (isResolving) {
    return <ProjectContextState pageLabel="Crop Şablonu" loading projectPath={projectPath} />;
  }

  if (!projectId) {
    return <ProjectContextState pageLabel="Crop Şablonu" projectPath={projectPath} />;
  }

  return (
    <div style={{ padding: '2rem', display: 'grid', gap: '1rem' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', flexWrap: 'wrap' }}>
        <Link to={`/project/${encodeURIComponent(projectId)}/overview`}>← İş akışına dön</Link>
        <Link to={projectStudentOperationsPath(projectId, 'ocr', searchParams.toString())}>Öğrenci Cevap OCR →</Link>
      </div>

      <h1>Crop Şablonu</h1>
      <p style={{ color: '#64748b', margin: 0 }}>
        Seçili sınıf veya paketteki ilk öğrenci kağıdı üzerinden cevap ve kimlik alanlarını işaretleyin. Şablon sınav paketine aittir ve bütün sınıflarda ortak kullanılır.
      </p>

      {visibleSubmissions.length === 0 && (
        <div role="status" style={{ padding: '1rem', border: '1px solid #fde68a', borderRadius: '12px', background: '#fffbeb', color: '#92400e' }}>
          Seçili kapsamda örnek öğrenci kağıdı yok. Önce PDF paketini gruplayın veya başka bir sınıf/paket seçin.
        </div>
      )}

      {error && <ErrorBanner error={error} />}

      <section style={{ padding: '1rem', border: '1px solid #e2e8f0', borderRadius: '16px', background: 'white', display: 'grid', gap: '0.75rem' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', flexWrap: 'wrap' }}>
          <strong>Şablon Durumu</strong>
          <div style={{ display: 'grid', gap: '0.25rem', textAlign: 'right' }}>
            <span>Cevap crop’ları: {getStudentAnswerCropTemplateSummary(project?.questions ?? [], templateItems)}</span>
            <span>Kimlik crop’u: {identityDraft ? 'hazır' : 'eksik'}</span>
          </div>
        </div>

        <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
          <button type="button" data-project-write="false" onClick={() => setMode('answers')} style={{ padding: '0.5rem 0.75rem', borderRadius: '8px', border: mode === 'answers' ? '2px solid #2563eb' : '1px solid #cbd5e1', background: mode === 'answers' ? '#eff6ff' : 'white' }}>
            Cevap alanı crop’ları
          </button>
          <button type="button" data-project-write="false" onClick={() => setMode('identity')} style={{ padding: '0.5rem 0.75rem', borderRadius: '8px', border: mode === 'identity' ? '2px solid #2563eb' : '1px solid #cbd5e1', background: mode === 'identity' ? '#eff6ff' : 'white' }}>
            Kimlik alanı crop’u
          </button>
        </div>

        <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
          {(['original', 'clean_grayscale', 'handwriting_enhanced', 'high_contrast', 'high_contrast_bw'] as OcrImagePreprocessMode[]).map((candidate) => (
            <button
              key={candidate}
              type="button"
              data-project-write="false"
              onClick={() => setPreviewMode(candidate)}
              style={{ padding: '0.5rem 0.75rem', borderRadius: '8px', border: previewMode === candidate ? '2px solid #16a34a' : '1px solid #cbd5e1', background: previewMode === candidate ? '#f0fdf4' : 'white' }}
            >
              {getOcrPreprocessModeLabel(candidate)}
            </button>
          ))}
        </div>
        
        {mode === 'answers' && <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
          {(project?.questions ?? []).map((question) => (
            <button
              key={question.id}
              type="button"
              data-project-write="false"
              onClick={() => setSelectedQuestionId(question.id)}
              style={{
                padding: '0.45rem 0.7rem',
                borderRadius: '8px',
                border: selectedQuestion?.id === question.id ? '2px solid #2563eb' : '1px solid #cbd5e1',
                background: templateDrafts[question.id] ? '#ecfdf5' : 'white',
              }}
            >
              S{question.number}
            </button>
          ))}
        </div>}
        
        {mode === 'answers' && selectedQuestion && (
          <div style={{ display: 'grid', gap: '0.75rem' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '1rem', flexWrap: 'wrap' }}>
              <div style={{ color: '#475569' }}>
                Öğrenci 1 · Sayfa {templatePageNumber ?? '-'} · Soru {selectedQuestion.number}
              </div>
              <PageNavigation
                currentPage={templatePageIndex + 1}
                totalPages={Math.max(templatePageCount, 1)}
                onChange={(page) => setTemplatePageIndex(page - 1)}
              />
            </div>
            <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap', alignItems: 'center' }}>
              <span style={{ color: '#64748b', fontSize: '0.85rem' }}>Cevap bölgeleri:</span>
              {(selectedTemplate?.regions ?? []).map((region) => (
                <button
                  key={region.regionId}
                  type="button"
                  data-project-write="false"
                  onClick={() => setSelectedRegionOrder(region.order)}
                  style={{ padding: '0.35rem 0.55rem', borderRadius: '7px', border: selectedRegionOrder === region.order ? '2px solid #2563eb' : '1px solid #cbd5e1', background: selectedRegionOrder === region.order ? '#eff6ff' : 'white' }}
                >
                  {region.order + 1}. {region.regionRole === 'continuation' ? 'Devam' : 'Ana'} · sf {region.pageOffset + 1}
                </button>
              ))}
            </div>
            
            <PdfPageViewer
              imagePath={previewMode === 'original' ? (templatePreview?.imagePath ?? null) : (cleanTemplatePreview?.outputImagePath ?? templatePreview?.imagePath ?? null)}
              projectId={projectId}
              pageNumber={templatePageNumber ?? 1}
              zoom={0.8}
              overlayBox={selectedTemplateOnCurrentPage?.normalizedBBox ?? null}
              editable
              emptyState={<div>Öğrenci 1 sayfa önizlemesi hazır değil.</div>}
              onOverlayChange={(box) => {
                const currentTemplate = selectedTemplate ?? { questionId: selectedQuestion.id, regions: [] };
                const currentRegion = selectedRegion ?? {
                  regionId: `${selectedQuestion.id}-region-${selectedRegionOrder}`,
                  pageOffset: templatePageIndex,
                  order: selectedRegionOrder,
                  normalizedBBox: box,
                  regionRole: selectedRegionOrder === 0 ? 'primary' as const : 'continuation' as const,
                  continuationPolicy: selectedRegionOrder === 0 ? 'independent' as const : 'continues_previous' as const,
                  label: `Soru ${selectedQuestion.number}`,
                };
                const nextRegion: QuestionAnswerRegion = {
                  ...currentRegion,
                  pageOffset: templatePageIndex,
                  normalizedBBox: box,
                };
                setTemplateDrafts((current) => ({
                  ...current,
                  [selectedQuestion.id]: {
                    questionId: selectedQuestion.id,
                    regions: [...currentTemplate.regions.filter((region) => region.order !== currentRegion.order), nextRegion]
                      .sort((left, right) => left.order - right.order),
                  },
                }));
              }}
            />
            
            <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
              <button
                type="button"
                data-project-write="false"
                onClick={() => {
                  setTemplateDrafts((current) => {
                    const next = { ...current };
                    const template = next[selectedQuestion.id];
                    if (!template) return next;
                    const regions = template.regions.filter((region) => region.order !== (selectedRegion?.order ?? selectedRegionOrder));
                    if (regions.length === 0) delete next[selectedQuestion.id];
                    else next[selectedQuestion.id] = { ...template, regions };
                    return next;
                  });
                }}
              >
                Seçili Crop’u Sil
              </button>
              <LoadingButton
                onClick={() => saveTemplateMutation.mutate(templateItems)}
                loading={saveTemplateMutation.isPending}
              >
                Şablonu Kaydet
              </LoadingButton>
              <button
                type="button"
                data-project-write="false"
                onClick={() => {
                  const nextOrder = (selectedTemplate?.regions.length ?? 0);
                  setSelectedRegionOrder(nextOrder);
                }}
              >
                Devam bölgesi ekle
              </button>
            </div>
          </div>
        )}

        {mode === 'identity' && (
          <div style={{ display: 'grid', gap: '0.75rem' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '1rem', flexWrap: 'wrap' }}>
              <div style={{ color: '#475569' }}>
                Öğrenci 1 · Sayfa {templatePageNumber ?? '-'} · Kimlik alanı
              </div>
              <PageNavigation
                currentPage={templatePageIndex + 1}
                totalPages={Math.max(templatePageCount, 1)}
                onChange={(page) => setTemplatePageIndex(page - 1)}
              />
            </div>

            <PdfPageViewer
              imagePath={previewMode === 'original' ? (templatePreview?.imagePath ?? null) : (cleanTemplatePreview?.outputImagePath ?? templatePreview?.imagePath ?? null)}
              projectId={projectId}
              pageNumber={templatePageNumber ?? 1}
              zoom={0.8}
              overlayBox={identityDraft?.pageIndexWithinSubmission === templatePageIndex ? identityDraft.bbox : null}
              editable
              emptyState={<div>Öğrenci 1 sayfa önizlemesi hazır değil.</div>}
              onOverlayChange={(box) => {
                const bbox: StudentAnswerOcrCropBBox = { ...box, pageIndex: templatePageIndex };
                setIdentityDraft({
                  pageIndexWithinSubmission: templatePageIndex,
                  bbox,
                  label: 'identity_header',
                });
              }}
            />
            {previewMode !== 'original' && cleanTemplatePreview && (
              <div style={{ fontSize: '0.8rem', color: '#475569' }}>
                {getOcrPreprocessModeLabel(previewMode)} • {cleanTemplatePreview.diagnostics.applied ? 'uygulandı' : 'orijinal kullanıldı'}
              </div>
            )}
            
            <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
              <button type="button" data-project-write="false" onClick={() => setIdentityDraft(null)}>
                Kimlik Crop’unu Sil
              </button>
              <LoadingButton
                onClick={() => identityDraft && saveIdentityTemplateMutation.mutate(identityDraft)}
                loading={saveIdentityTemplateMutation.isPending}
                disabled={!identityDraft}
              >
                Kimlik Şablonunu Kaydet
              </LoadingButton>
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
