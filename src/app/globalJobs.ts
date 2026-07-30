import type { JobKind, JobSnapshot } from '../api/types';

const jobLabels: Record<JobKind, string> = {
  question_text_extraction: 'Soru metinleri hazırlanıyor',
  pdf_preview_render: 'PDF önizlemesi hazırlanıyor',
  rubric_pdf_import: 'Rubrikler hazırlanıyor',
  exam_package_build: 'Sınav paketi hazırlanıyor',
  student_answer_ocr: 'Öğrenci cevapları okunuyor',
  student_identity_ocr: 'Öğrenci kimlikleri okunuyor',
  scoring: 'Notlandırma yapılıyor',
  speaking_evaluation: 'Konuşma değerlendirmesi yapılıyor',
  assessment_analysis: 'Sınav analizi hazırlanıyor',
};

export function getJobLabel(kind: JobKind): string {
  return jobLabels[kind];
}

export function getActiveJobs(jobs: JobSnapshot[]): JobSnapshot[] {
  return jobs.filter((job) => job.status === 'queued' || job.status === 'running');
}

export function getFailedJobs(jobs: JobSnapshot[]): JobSnapshot[] {
  return jobs.filter((job) => job.status === 'failed');
}

export function getJobProgressPercent(job: JobSnapshot): number {
  if (job.progress.total <= 0) return 0;
  return Math.min(100, Math.max(0, Math.round((job.progress.current / job.progress.total) * 100)));
}

export function getJobCenterButtonLabel(jobs: JobSnapshot[]): string {
  const activeJobs = getActiveJobs(jobs);
  const failedJobs = getFailedJobs(jobs);
  const [activeJob] = activeJobs;
  if (activeJob && activeJobs.length === 1) return getJobLabel(activeJob.kind);
  if (activeJobs.length > 1) return `${activeJobs.length} işlem devam ediyor`;
  if (failedJobs.length > 0) return `${failedJobs.length} işlem kontrol bekliyor`;
  return 'Aktif işlem yok';
}
