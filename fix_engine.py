f = "src-tauri/src/services/workflow_engine.rs"
with open(f, "r") as file:
    content = file.read()

import re

# workflow_engine.rs active_student_scan.unwrap()
# Let's find it. It's likely inside a context where it's assumed to be Some.
content = content.replace("let active_student_scan = active_student_scan.unwrap();", "let active_student_scan = active_student_scan.unwrap_or_default();") # or better if let?
# Let's look at the context.
