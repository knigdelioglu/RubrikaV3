export function teacherFacingRubricWarnings(warnings: string[]): string[] {
  return warnings.filter((warning) => !warning.includes('_alias:') && warning !== 'rubric_empty_content');
}
