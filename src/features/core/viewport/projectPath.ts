let currentProjectPath: string | null = null;

/** Application publishes the current project path; viewport code only reads this snapshot. */
export function setProjectPathForViewport(path: string | null): void {
  currentProjectPath = path;
}

export function projectPathForViewport(): string | null {
  return currentProjectPath;
}
