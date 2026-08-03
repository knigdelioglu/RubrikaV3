export type ClassesTab = 'classes' | 'roster';

export function getClassesTab(searchParams: URLSearchParams): ClassesTab {
  return searchParams.get('tab') === 'roster' ? 'roster' : 'classes';
}

export function setClassesTab(searchParams: URLSearchParams, tab: ClassesTab): URLSearchParams {
  const next = new URLSearchParams(searchParams);
  if (tab === 'roster') {
    next.set('tab', 'roster');
  } else {
    next.delete('tab');
  }
  return next;
}

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
