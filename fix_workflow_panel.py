import re

f = "src/components/workflow/WorkflowPanel.tsx"
with open(f, "r") as file:
    content = file.read()

# We want to remove the block from `const expectedQuestionCount` all the way to the end of `flowSteps` array (around line 238)
# and replace it with something simpler that maps `workflow.summary.steps` with active job statuses.

start_str = "  const expectedQuestionCount = project?.questions.length ?? 0;"
end_str = "  ] as const;"

start_idx = content.find(start_str)
end_idx = content.find(end_str) + len(end_str)

replacement = """  const expectedQuestionCount = project?.questions.length ?? 0;
  const questionSummary = project?.questions ?? [];
  const questionNumbers = questionSummary.map((q) => q.number);
  const questionByNumber = new Map(questionSummary.map((q) => [q.number, q]));
  const missingQuestionNumbers = questionNumbers.filter((number) => {
    const status = questionByNumber.get(number)?.questionText.status ?? 'missing';
    return status === 'missing' || status === 'failed';
  });
  const examSourceDocuments = project?.documents.filter((document) => document.role === 'exam_source') ?? [];
  const studentScanDocuments = project?.documents.filter((document) => document.role === 'student_scan') ?? [];
  const studentSubmissionCount = project?.studentSubmissions?.length ?? 0;
  const hasExamSource = examSourceDocuments.length > 0;
  const hasRubricDocs = (project?.documents ?? []).some((document) => document.role === 'answer_key' || document.role === 'rubric');

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
