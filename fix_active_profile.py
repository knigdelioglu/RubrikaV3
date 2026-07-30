f = "src-tauri/src/services/model_config_service.rs"
with open(f, "r") as file:
    content = file.read()

import re

# Fix active_profile_id
content = re.sub(
    r"pub fn active_profile_id\(\&self\) -> String \{.*?\}\)\?;",
    "pub fn active_profile_id(&self) -> String {\n        let store = self.store.lock().unwrap_or_else(|e| e.into_inner());",
    content,
    flags=re.DOTALL
)

with open(f, "w") as file:
    file.write(content)
