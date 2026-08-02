export const DEFAULT_COURSE_ID = 'tde';
export const DEFAULT_COURSE_NAME = 'Türk Dili ve Edebiyatı';

export type DefaultProjectPathQueryConfig = {
  queryKey: readonly ['default-project-path', string, string];
  enabled: boolean;
};

export function getDefaultAcademicYear(date: Date = new Date()): string {
  const currentYear = date.getFullYear();
  const academicYearStart = date.getMonth() >= 6 ? currentYear : currentYear - 1;
  return `${academicYearStart}-${academicYearStart + 1}`;
}

export function getDefaultProjectPathQueryConfig(
  projectName: string,
  academicYearId: string,
): DefaultProjectPathQueryConfig {
  return {
    queryKey: ['default-project-path', projectName, academicYearId],
    enabled: projectName.trim().length > 0 && academicYearId.trim().length > 0,
  };
}
