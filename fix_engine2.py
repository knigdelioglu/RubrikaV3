f = "src-tauri/src/services/workflow_engine.rs"
with open(f, "r") as file:
    content = file.read()

import re

# We will replace the block:
# if active_student_scan.is_none() { ... return (...); }
# let active_student_scan = active_student_scan.unwrap();

pattern = re.compile(
    r"if active_student_scan\.is_none\(\) \{(.*?)\} \n\n        let active_student_scan = active_student_scan\.unwrap\(\);",
    re.DOTALL
)

def replacer(match):
    block = match.group(1)
    return f"let Some(active_student_scan) = active_student_scan else {{{block}}};"

new_content = pattern.sub(replacer, content)

with open(f, "w") as file:
    file.write(new_content)
