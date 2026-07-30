import re

# 1. errors.rs
f = "src-tauri/src/domain/errors.rs"
with open(f, "r") as file:
    content = file.read()
content = content.replace(
    '    ModelServerNotStartedByApp,',
    '    ModelServerNotStartedByApp,\n    ModelStateAccessFailed,'
)
with open(f, "w") as file:
    file.write(content)

# 2. errors.ts
f = "src/api/errors.ts"
with open(f, "r") as file:
    content = file.read()
content = content.replace(
    "  | 'MODEL_SERVER_NOT_STARTED_BY_APP'",
    "  | 'MODEL_SERVER_NOT_STARTED_BY_APP'\n  | 'MODEL_STATE_ACCESS_FAILED'"
)
with open(f, "w") as file:
    file.write(content)

# 3. labels.ts
f = "src/utils/labels.ts"
with open(f, "r") as file:
    content = file.read()
content = content.replace(
    "  MODEL_SERVER_NOT_STARTED_BY_APP: 'Bu model sunucusu dışarıdan başlatılmış.',",
    "  MODEL_SERVER_NOT_STARTED_BY_APP: 'Bu model sunucusu dışarıdan başlatılmış.',\n  MODEL_STATE_ACCESS_FAILED: 'Model durumuna erişilemedi.',"
)
with open(f, "w") as file:
    file.write(content)

# 4. ERROR_CODES.md
f = "docs/ERROR_CODES.md"
with open(f, "r") as file:
    content = file.read()
content = content.replace(
    "| `MODEL_SERVER_NOT_STARTED_BY_APP` | Bu model sunucusu RubrikaV3 tarafından başlatılmadığı için kapatılmadı. |",
    "| `MODEL_SERVER_NOT_STARTED_BY_APP` | Bu model sunucusu RubrikaV3 tarafından başlatılmadığı için kapatılmadı. |\n| `MODEL_STATE_ACCESS_FAILED` | Model durumuna (mutex) erişilemedi. |"
)
with open(f, "w") as file:
    file.write(content)
