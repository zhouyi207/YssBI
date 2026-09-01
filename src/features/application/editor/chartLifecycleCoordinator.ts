const lifecycleTokensByProject = new Map<string, Map<string, number>>();
let nextLifecycleToken = Date.now() * 1_000;

function projectTokens(projectInstanceId: string): Map<string, number> {
  const existing = lifecycleTokensByProject.get(projectInstanceId);
  if (existing) return existing;
  const created = new Map<string, number>();
  lifecycleTokensByProject.set(projectInstanceId, created);
  return created;
}

export function beginChartRenameLifecycle(projectInstanceId: string, chartPath: string): number {
  const lifecycleToken = ++nextLifecycleToken;
  projectTokens(projectInstanceId).set(chartPath, lifecycleToken);
  return lifecycleToken;
}

export function isChartLifecycleCurrent(
  projectInstanceId: string,
  chartPath: string,
  lifecycleToken: number,
): boolean {
  return lifecycleTokensByProject.get(projectInstanceId)?.get(chartPath) === lifecycleToken;
}

export function clearChartLifecycleProject(projectInstanceId: string): void {
  lifecycleTokensByProject.delete(projectInstanceId);
}

export function clearChartLifecycleProjects(): void {
  lifecycleTokensByProject.clear();
}
