import { relative, resolve } from "node:path";
import type { ArchitectureSource } from "./moduleDependencyAudit";
import { normalizeTypeScriptPath, type TypeScriptAuditProject } from "./typescriptAudit";

function repositorySourcePath(context: TypeScriptAuditProject, fileName: string): string | null {
  const absolutePath = normalizeTypeScriptPath(resolve(fileName));
  const relativePath = normalizeTypeScriptPath(relative(context.sourceRoot, absolutePath));
  if (relativePath.startsWith("../") || relativePath.includes(":/")) return null;
  return relativePath.startsWith("src/") ? relativePath : null;
}

function isProductionModulePath(path: string): boolean {
  const lower = path.toLowerCase();
  if (
    (!lower.endsWith(".ts") && !lower.endsWith(".tsx") && !lower.endsWith(".json")) ||
    lower.endsWith(".d.ts")
  ) {
    return false;
  }
  const segments = lower.split("/");
  const fileName = segments[segments.length - 1];
  if (
    segments[1] === "tests" ||
    segments.includes("__tests__") ||
    segments.includes("fixtures") ||
    fileName.includes(".test.") ||
    fileName.includes(".spec.") ||
    fileName.includes(".fixture.") ||
    fileName.endsWith("fixture.ts") ||
    fileName.endsWith("fixture.tsx")
  )
    return false;
  return true;
}

export function productionTypeScriptSources(
  context: TypeScriptAuditProject,
): readonly ArchitectureSource[] {
  const sources = new Map<string, ArchitectureSource>();
  for (const fileName of context.project.program.getSourceFileNames()) {
    const path = repositorySourcePath(context, fileName);
    if (path === null || !isProductionModulePath(path)) continue;
    const sourceFile = context.project.program.getSourceFile(fileName);
    if (!sourceFile) throw new Error(`TypeScript program source disappeared: ${path}`);
    if (path.split("/").includes("..")) {
      throw new Error(`Production TypeScript source escapes repository: ${path}`);
    }
    sources.set(path, { path, source: sourceFile.text });
  }
  return [...sources.values()].sort((left, right) => left.path.localeCompare(right.path));
}
