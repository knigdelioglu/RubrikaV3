import re

with open("src-tauri/src/services/workflow_engine.rs", "r") as f:
    content = f.read()

# 1. Rename function and change return type
content = content.replace(
    "pub fn evaluate_workflow_with_context(\n    project: &Project,\n    model_status: &crate::domain::model::ModelStatus,\n    question_text_job_active: bool,\n) -> WorkflowSnapshot {",
    "pub fn evaluate_workflow_inner(\n    project: &Project,\n    model_status: &crate::domain::model::ModelStatus,\n    question_text_job_active: bool,\n) -> (WorkflowStage, Vec<BlockingReason>, Vec<WorkflowAction>, Option<String>) {"
)

# 2. Early return for ExamPackageBuild stages
content = content.replace(
    "    if matches!(\n        project.workflow.current_stage,\n        WorkflowStage::ExamPackageBuildReady\n            | WorkflowStage::ExamPackageBuildRunning\n            | WorkflowStage::ExamPackageReviewNeeded\n            | WorkflowStage::ExamPackageIncomplete\n            | WorkflowStage::ExamPackageReadyForQep\n    ) {\n        return project.workflow.clone();\n    }",
    "    if matches!(\n        project.workflow.current_stage,\n        WorkflowStage::ExamPackageBuildReady\n            | WorkflowStage::ExamPackageBuildRunning\n            | WorkflowStage::ExamPackageReviewNeeded\n            | WorkflowStage::ExamPackageIncomplete\n            | WorkflowStage::ExamPackageReadyForQep\n    ) {\n        return (\n            project.workflow.current_stage.clone(),\n            project.workflow.blocking_reasons.clone(),\n            project.workflow.next_actions.clone(),\n            project.workflow.summary.text.clone(),\n        );\n    }"
)

# 3. Replace all return WorkflowSnapshot { ... }
pattern = re.compile(
    r"return WorkflowSnapshot \{\s*current_stage:\s*(.*?),\s*blocking_reasons(,?)\s*next_actions(,?)\s*summary:\s*(.*?),\s*\};",
    re.DOTALL
)

def replacer(match):
    stage = match.group(1).strip()
    summary = match.group(4).strip()
    return f"return ({stage}, blocking_reasons, next_actions, {summary});"

content = pattern.sub(replacer, content)

with open("src-tauri/src/services/workflow_engine.rs", "w") as f:
    f.write(content)
