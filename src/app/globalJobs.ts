import type { JobKind, JobSnapshot } from '../api/types';

const jobLabels: Record<JobKind, string> = {
  document_import: 'Belge yükleniyor',
  question_text_extraction: 'Soru metinleri hazırlanıyor',
  pdf_preview_render: 'PDF önizlemesi hazırlanıyor',
  rubric_pdf_import: 'Rubrikler hazırlanıyor',
  exam_package_build: 'Sınav paketi hazırlanıyor',
  student_answer_ocr: 'Öğrenci cevapları okunuyor',
  student_identity_ocr: 'Öğrenci kimlikleri okunuyor',
  scoring: 'Notlandırma yapılıyor',
  speaking_evaluation: 'Konuşma değerlendirmesi yapılıyor',
  assessment_analysis: 'Sınav analizi hazırlanıyor',
  project_backup: 'Proje yedeği hazırlanıyor',
  project_restore: 'Proje yedeği geri yükleniyor',
  project_recovery: 'Recovery kopyası hazırlanıyor',
};

export function getJobLabel(kind: JobKind): string {
  return jobLabels[kind] || 'İşlem yürütülüyor';
}

export function getActiveJobs(jobs: JobSnapshot[]): JobSnapshot[] {
  return jobs.filter((job) => job.status === 'queued' || job.status === 'running');
}

export function getFailedJobs(jobs: JobSnapshot[]): JobSnapshot[] {
  return jobs.filter((job) => job.status === 'failed' || job.status === 'interrupted');
}

export function getPartialJobs(jobs: JobSnapshot[]): JobSnapshot[] {
  return jobs.filter((job) => job.status === 'partial');
}

export function getJobProgressPercent(job: JobSnapshot): number {
  if (job.progress.total <= 0) return 0;
  return Math.min(100, Math.max(0, Math.round((job.progress.current / job.progress.total) * 100)));
}

export function getJobCenterButtonLabel(jobs: JobSnapshot[]): string {
  const activeJobs = getActiveJobs(jobs);
  const failedJobs = getFailedJobs(jobs);
  const partialJobs = getPartialJobs(jobs);
  const [activeJob] = activeJobs;
  if (activeJob && activeJobs.length === 1) {
    if (activeJob.cancellationRequested) {
      return `${getJobLabel(activeJob.kind)} (İptal ediliyor...)`;
    }
    return getJobLabel(activeJob.kind);
  }
  if (activeJobs.length > 1) return `${activeJobs.length} işlem devam ediyor`;
  if (failedJobs.length > 0) return `${failedJobs.length} işlem kontrol bekliyor`;
  if (partialJobs.length > 0) return `${partialJobs.length} işlem kısmen tamamlandı`;
  return 'Aktif işlem yok';
}
