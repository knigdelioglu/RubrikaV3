import glob
import re

files = glob.glob("src-tauri/src/**/*.rs", recursive=True)

pattern = re.compile(
    r"WorkflowSnapshot \{\s*current_stage:\s*(.*?),\s*blocking_reasons(,?)\s*next_actions(,?)\s*summary:\s*None,\s*\}",
    re.DOTALL
)

def replacer(match):
    stage = match.group(1).strip()
    return f"WorkflowSnapshot {{\n            current_stage: {stage},\n            current_stage_label: \"Test\".to_string(),\n            blocking_reasons{match.group(2)}\n            next_actions{match.group(3)}\n            summary: crate::domain::workflow::WorkflowSummary::default(),\n        }}"

for f in files:
    with open(f, "r") as file:
        content = file.read()
    
    new_content = pattern.sub(replacer, content)
    
    if new_content != content:
        with open(f, "w") as file:
            file.write(new_content)
