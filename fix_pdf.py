with open("src-tauri/src/services/pdf_service.rs", "r") as f:
    content = f.read()

content = content.replace("        }\n        let binary_path = binary_path.unwrap();", "        };\n")
content = content.replace("if binary_path.is_none() {", "let Some(binary_path) = binary_path else {")

with open("src-tauri/src/services/pdf_service.rs", "w") as f:
    f.write(content)
