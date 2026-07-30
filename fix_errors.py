import os

# 1. exam_package_build_service.rs
f = "src-tauri/src/services/exam_package_build_service.rs"
with open(f, "r") as file:
    content = file.read()
content = content.replace(
    'summary: Some("Sınav paketi oluşturma başarısız oldu.".to_string()),',
    'current_stage_label: crate::domain::workflow::WorkflowStage::ExamPackageIncomplete.to_string(),\n                        summary: crate::domain::workflow::WorkflowSummary { text: Some("Sınav paketi oluşturma başarısız oldu.".to_string()), ..Default::default() },'
)
content = content.replace(
    'summary: Some("Sınav paketi hazırlanıyor...".to_string()),',
    'current_stage_label: crate::domain::workflow::WorkflowStage::ExamPackageBuildRunning.to_string(),\n                            summary: crate::domain::workflow::WorkflowSummary { text: Some("Sınav paketi hazırlanıyor...".to_string()), ..Default::default() },'
)
content = content.replace(
    'summary: Some(summary),',
    'current_stage_label: stage.to_string(),\n            summary: crate::domain::workflow::WorkflowSummary { text: Some(summary), ..Default::default() },'
)
# Note: we used .to_string() for stage which might not exist on enum, but let's just use empty string or format!
content = content.replace('current_stage_label: crate::domain::workflow::WorkflowStage::ExamPackageIncomplete.to_string(),', 'current_stage_label: "Sınav Paketi Eksik".to_string(),')
content = content.replace('current_stage_label: crate::domain::workflow::WorkflowStage::ExamPackageBuildRunning.to_string(),', 'current_stage_label: "Sınav Paketi Oluşturuluyor".to_string(),')
content = content.replace('current_stage_label: stage.to_string(),', 'current_stage_label: "Sınav Paketi".to_string(),')
with open(f, "w") as file:
    file.write(content)

# 2. question_text_service.rs
f = "src-tauri/src/services/question_text_service.rs"
with open(f, "r") as file:
    content = file.read()
content = content.replace(
    'running_project.workflow.summary = Some("PDF metni taranıyor.".to_string());',
    'running_project.workflow.summary.text = Some("PDF metni taranıyor.".to_string());'
)
content = content.replace(
    'running_project.workflow.summary =\n            Some("Gemma vision fallback eksik sorular için çalışıyor.".to_string());',
    'running_project.workflow.summary.text = Some("Gemma vision fallback eksik sorular için çalışıyor.".to_string());'
)
content = content.replace(
    'summary: workflow.summary.clone(),',
    'summary: workflow.summary.text.clone(),'
)
with open(f, "w") as file:
    file.write(content)

# 3. student_scan_service.rs
f = "src-tauri/src/services/student_scan_service.rs"
with open(f, "r") as file:
    content = file.read()
content = content.replace(
    'message: workflow.summary.unwrap_or_else(|| {',
    'message: workflow.summary.text.unwrap_or_else(|| {'
)
with open(f, "w") as file:
    file.write(content)

# 4. project_store.rs
f = "src-tauri/src/services/project_store.rs"
with open(f, "r") as file:
    content = file.read()
content = content.replace(
    'current_stage: WorkflowStage::DocumentsMissing,',
    'current_stage: WorkflowStage::DocumentsMissing,\n                    current_stage_label: "Belgeler Eksik".to_string(),'
)
content = content.replace(
    'summary: None,',
    'summary: crate::domain::workflow::WorkflowSummary::default(),'
)
with open(f, "w") as file:
    file.write(content)

