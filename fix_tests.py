f = "src-tauri/src/services/workflow_engine.rs"
with open(f, "r") as file:
    content = file.read()

# Fix `workflow_engine.rs` tests
content = content.replace(
    'summary: Some("Orijinal sınav PDF\'i bekleniyor.".to_string()),',
    'current_stage_label: "Belgeler Eksik".to_string(),\n            summary: crate::domain::workflow::WorkflowSummary { text: Some("Orijinal sınav PDF\'i bekleniyor.".to_string()), ..Default::default() },'
)
content = content.replace(
    'summary: Some("Sınav paketi oluşturma başarısız oldu.".to_string()),',
    'current_stage_label: "Sınav Paketi Eksik".to_string(),\n            summary: crate::domain::workflow::WorkflowSummary { text: Some("Sınav paketi oluşturma başarısız oldu.".to_string()), ..Default::default() },'
)
content = content.replace(
    'summary: Some("Rubrik hazırlığı bekleniyor.".to_string()),',
    'current_stage_label: "Rubrik Eksik".to_string(),\n            summary: crate::domain::workflow::WorkflowSummary { text: Some("Rubrik hazırlığı bekleniyor.".to_string()), ..Default::default() },'
)
content = content.replace(
    'snap.summary.as_deref()',
    'snap.summary.text.as_deref()'
)

# Replace remaining plain WorkflowSnapshot in tests
import re
pattern = re.compile(
    r"WorkflowSnapshot \{\s*current_stage:\s*(.*?),\s*blocking_reasons(,?)\s*next_actions(,?)\s*summary:\s*(.*?),\s*\}",
    re.DOTALL
)

def replacer(match):
    stage = match.group(1).strip()
    summary = match.group(4).strip()
    # Assuming tests only have simple summaries
    return f"WorkflowSnapshot {{\n            current_stage: {stage},\n            current_stage_label: \"Test\".to_string(),\n            blocking_reasons{match.group(2)}\n            next_actions{match.group(3)}\n            summary: crate::domain::workflow::WorkflowSummary {{ text: {summary}, ..Default::default() }},\n        }}"

content = pattern.sub(replacer, content)

with open(f, "w") as file:
    file.write(content)
