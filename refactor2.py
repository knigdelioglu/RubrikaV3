import re

with open("src-tauri/src/services/workflow_engine.rs", "r") as f:
    content = f.read()

pattern = re.compile(
    r"WorkflowSnapshot \{\s*current_stage:\s*(.*?),\s*blocking_reasons(,?)\s*next_actions(,?)\s*summary:\s*(.*?),\s*\}",
    re.DOTALL
)

def replacer(match):
    stage = match.group(1).strip()
    summary = match.group(4).strip()
    return f"({stage}, blocking_reasons, next_actions, {summary})"

content = pattern.sub(replacer, content)

with open("src-tauri/src/services/workflow_engine.rs", "w") as f:
    f.write(content)
