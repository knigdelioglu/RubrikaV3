f = "src/components/workflow/WorkflowPanel.tsx"
with open(f, "r") as file:
    lines = file.readlines()

# The extra crap starts at line 504 `}  const expectedQuestionCount = project?.questions.length ?? 0;\n`
# Let's truncate everything after line 504 and remove the extra text on line 504.
lines = lines[:505]
lines[504] = "}\n"

with open(f, "w") as file:
    file.writelines(lines)
