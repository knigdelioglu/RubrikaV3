import re

with open("src/pages/ExamPackageReviewPage.tsx", "r") as f:
    content = f.read()

pattern = r"  const questionTextReadyCount =.*?\n  }\)\.length;\n  const questionTextReady =.*?\n  const questionTextPartial =.*?\n\n  const rubricMissingNumbers =.*?\n    return status === 'missing';\n  }\);\n  const rubricFailedNumbers =.*?\n    return status === 'invalid' \|\| status === 'legacy';\n  }\);\n  const rubricReadyCount =.*?\n    return status === 'imported' \|\| status === 'manual' \|\| status === 'confirmed' \|\| status === 'suggested';\n  }\)\.length;\n  const rubricReady =.*?\n  const rubricPartial =.*?\n"

replacement = """  const qStep = project.workflow.summary.steps.find((s) => s.code === 'question_text_extraction');
  const rStep = project.workflow.summary.steps.find((s) => s.code === 'rubric_pdf_import');
  const questionTextReady = qStep?.status === 'succeeded';
  const questionTextPartial = qStep?.status === 'partial';
  const rubricReady = rStep?.status === 'succeeded';
  const rubricPartial = rStep?.status === 'partial';

  const rubricMissingNumbers = questionNumbers.filter((number) => {
    const status = questionByNumber.get(number)?.rubric.status ?? 'missing';
    return status === 'missing';
  });
  const rubricFailedNumbers = questionNumbers.filter((number) => {
    const status = questionByNumber.get(number)?.rubric.status ?? 'missing';
    return status === 'invalid' || status === 'legacy';
  });
"""

# wait, I also need to replace the packageLabel:
pattern2 = r"  const packageLabel = questionTextReady && rubricReady\n    \? 'Sınav paketi — Hazır'\n    : questionTextPartial && rubricPartial\n      \? 'Sınav paketi — Kısmi, inceleme gerekli'\n      : questionTextPartial\n        \? 'Sınav paketi — Kısmi, soru metni eksikleri var'\n        : rubricPartial\n          \? 'Sınav paketi — Kısmi, rubrik eksikleri var'\n          : 'Sınav paketi — Kısmi, inceleme gerekli';"
replacement2 = """  const packageLabel = project.workflow.summary.text || (questionTextReady && rubricReady
    ? 'Sınav paketi — Hazır'
    : questionTextPartial && rubricPartial
      ? 'Sınav paketi — Kısmi, inceleme gerekli'
      : questionTextPartial
        ? 'Sınav paketi — Kısmi, soru metni eksikleri var'
        : rubricPartial
          ? 'Sınav paketi — Kısmi, rubrik eksikleri var'
          : 'Sınav paketi — Kısmi, inceleme gerekli');"""

new_content = re.sub(pattern, replacement, content, flags=re.DOTALL)
new_content = new_content.replace(
    "  const packageLabel = questionTextReady && rubricReady\n    ? 'Sınav paketi — Hazır'\n    : questionTextPartial && rubricPartial\n      ? 'Sınav paketi — Kısmi, inceleme gerekli'\n      : questionTextPartial\n        ? 'Sınav paketi — Kısmi, soru metni eksikleri var'\n        : rubricPartial\n          ? 'Sınav paketi — Kısmi, rubrik eksikleri var'\n          : 'Sınav paketi — Kısmi, inceleme gerekli';",
    replacement2
)

with open("src/pages/ExamPackageReviewPage.tsx", "w") as f:
    f.write(new_content)
