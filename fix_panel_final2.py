f = "src/components/workflow/WorkflowPanel.tsx"
with open(f, "r") as file:
    content = file.read()

start_str = "  const expectedQuestionCount = project?.expectedQuestionCount ?? questionSummary.length;"
end_str = "  ] as const;"

start_idx = content.find(start_str)
end_idx = content.find(end_str) + len(end_str)

replacement = """  const expectedQuestionCount = project?.expectedQuestionCount ?? questionSummary.length;
  const questionNumbers = Array.from({ length: expectedQuestionCount }, (_, index) => index + 1);
  const questionByNumber = new Map(questionSummary.map((q) => [q.number, q]));
  const missingQuestionNumbers = questionNumbers.filter((number) => {
    const status = questionByNumber.get(number)?.questionText.status ?? 'missing';
    return status === 'missing' || status === 'failed';
  });
  const suggestedCount = questionNumbers.filter((number) => (questionByNumber.get(number)?.questionText.status ?? 'missing') === 'suggested').length;
  const missingCount = missingQuestionNumbers.length;
  
  const rubricMissingCount = questionNumbers.filter((number) => (questionByNumber.get(number)?.rubric.status ?? 'missing') === 'missing').length;
  const rubricImportedCount = questionNumbers.filter((number) => {
    const status = questionByNumber.get(number)?.rubric.status ?? 'missing';
    return status === 'imported' || status === 'manual' || status === 'confirmed' || status === 'suggested';
  }).length;
  const rubricManualCount = questionSummary.filter((q) => q.rubric.status === 'manual').length;
  const rubricConfirmedCount = questionSummary.filter((q) => q.rubric.status === 'confirmed').length;
  const rubricInvalidCount = questionSummary.filter((q) => q.rubric.status === 'invalid').length;

  const studentScanDocuments = project?.documents.filter((document) => document.role === 'student_scan') ?? [];
  const studentSubmissionCount = project?.studentSubmissions?.length ?? 0;
  const hasExamSource = examSourceDocuments.length > 0;
  const hasRubricDocs = (project?.documents ?? []).some((document) => document.role === 'answer_key' || document.role === 'rubric');
  
  const buildJob = jobs.find((job) => job.kind === 'exam_package_build');
  const buildJobActive = activeJobs.some((job) => job.kind === 'exam_package_build');
  const buildJobSucceeded = buildJob?.status === 'succeeded';

  const stageLabel = workflow.currentStageLabel;
  const flowSteps = workflow.summary.steps.map(step => {
    // Override status if there is an active job
    const job = activeJobs.find(j => 
        (step.code === 'pdf_preview_render' && j.kind === 'pdf_preview_render') ||
        (step.code === 'question_text_extraction' && j.kind === 'question_text_extraction') ||
        (step.code === 'rubric_pdf_import' && j.kind === 'rubric_pdf_import')
    );
    if (job?.status === 'running' || job?.status === 'queued') {
        return { ...step, status: 'running', message: job.progress.message || step.message };
    }
    if (job?.status === 'failed') {
        return { ...step, status: 'failed', message: job.error?.message || step.message };
    }
    return step;
  });"""

new_content = content[:start_idx] + replacement + content[end_idx:]

with open(f, "w") as file:
    file.write(new_content)
