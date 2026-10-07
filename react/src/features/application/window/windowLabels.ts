export function createEphemeralWindowLabel(prefix: string): string {
  return `${prefix}-${Math.random().toString(36).substring(7)}`;
}
