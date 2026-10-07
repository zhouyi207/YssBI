export type ArchitectureView =
  | "overview"
  | "frontend"
  | "backend"
  | "communication"
  | "dependencies";

export function readArchitectureView(search: string): ArchitectureView | null {
  const view = new URLSearchParams(search).get("architecture");
  return view === "overview" ||
    view === "frontend" ||
    view === "backend" ||
    view === "communication" ||
    view === "dependencies"
    ? view
    : null;
}

export function architectureSearch(search: string, view: ArchitectureView | null): string {
  const params = new URLSearchParams(search);
  if (view) params.set("architecture", view);
  else params.delete("architecture");
  if (view !== "dependencies") params.delete("crate");
  const query = params.toString();
  return query ? `?${query}` : "";
}
