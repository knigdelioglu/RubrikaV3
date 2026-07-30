import glob

files = glob.glob("src-tauri/src/**/*.rs", recursive=True)

for f in files:
    with open(f, "r") as file:
        content = file.read()
    
    # Simple replace since they are all exactly `summary: None,` within tests
    new_content = content.replace('summary: None,', 'current_stage_label: "Test".to_string(), summary: crate::domain::workflow::WorkflowSummary::default(),')
    
    if new_content != content:
        with open(f, "w") as file:
            file.write(new_content)
