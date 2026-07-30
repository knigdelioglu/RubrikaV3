import re

f = "src/pages/ModelStatusPage.tsx"
with open(f, "r") as file:
    content = file.read()

# Change showTechnicalDetails={true} to {false}
content = content.replace("showTechnicalDetails={true}", "showTechnicalDetails={false}")

with open(f, "w") as file:
    file.write(content)
