const lifecycleTokensByProject = new Map<string, Map<string, number>>();
let nextLifecycleToken = Date.now() * 1_000;

function projectTokens(projectInstanceId: string): Map<string, number> {
  const existing = lifecycleTokensByProject.get(projectInstanceId);
  if (existing) return existing;
  const created = new Map<string, number>();
  lifecycleTokensByProject.set(projectInstanceId, created);
  return created;
}

export function beginWorksheetRenameLifecycle(
  projectInstanceId: string,
  worksheetPath: string,
): number {
  const lifecycleToken = ++nextLifecycleToken;
  projectTokens(projectInstanceId).set(worksheetPath, lifecycleToken);
  return lifecycleToken;
}

export function isWorksheetLifecycleCurrent(
  projectInstanceId: string,
  worksheetPath: string,
  lifecycleToken: number,
): boolean {
  return lifecycleTokensByProject.get(projectInstanceId)?.get(worksheetPath) === lifecycleToken;
}

export function clearWorksheetLifecycleProject(projectInstanceId: string): void {
  lifecycleTokensByProject.delete(projectInstanceId);
}

export function clearWorksheetLifecycleProjects(): void {
  lifecycleTokensByProject.clear();
}

