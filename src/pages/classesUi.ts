export function getClassesSetupTargetId(setupParam: string | null): string | null {
  switch (setupParam) {
    case 'course':
      return 'setup-step-course';
    case 'classes':
      return 'setup-step-classes';
    case 'assignments':
      return 'setup-step-assignments';
    default:
      return null;
  }
}
