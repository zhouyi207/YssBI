import { dirname, resolve } from "node:path";
import type { Node, SourceFile } from "typescript/unstable/ast";
import { createVirtualFileSystem, type FileSystem } from "typescript/unstable/fs";
import { API, type Project, type Snapshot } from "typescript/unstable/sync";
import { afterAll } from "vitest";

export type TypeScriptSourceMap = Readonly<Record<string, string>> | ReadonlyMap<string, string>;

export interface TypeScriptAuditProject {
  readonly project: Project;
  readonly checker: Project["checker"];
  readonly sourceRoot: string;
  sourceFile(path: string): SourceFile;
}

export function normalizeTypeScriptPath(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  return /^[A-Z]:\//.test(normalized)
    ? `${normalized[0].toLowerCase()}${normalized.slice(1)}`
    : normalized;
}

function sourceEntries(sources: TypeScriptSourceMap): [string, string][] {
  return sources instanceof Map ? [...sources.entries()] : Object.entries(sources);
}

function fixtureRelativePath(path: string): string {
  const normalized = normalizeTypeScriptPath(path)
    .replace(/^[a-z]:\//, "")
    .replace(/^\.\//, "")
    .replace(/^\/+/, "");
  if (!normalized || normalized.split("/").includes("..")) {
    throw new Error(`Invalid isolated TypeScript source path: ${path}`);
  }
  return normalized;
}

function requiredSourceFile(project: Project, path: string): SourceFile {
  const sourceFile = project.program.getSourceFile(path);
  if (!sourceFile) throw new Error(`TypeScript project does not contain ${path}`);
  return sourceFile;
}

function releaseProject(api: API, configPath: string): void {
  const released = api.updateSnapshot({ closeProjects: [configPath] });
  released.dispose();
}

class IsolatedProjectHost {
  private readonly root = "c:/yssbi-typescript-audit/isolated";
  private readonly configPath = `${this.root}/tsconfig.json`;
  private readonly memory = createVirtualFileSystem({});
  private readonly api: API;
  private sourcePaths = new Set<string>();
  private runId = 0;
  private opened = false;
  private running = false;

  constructor() {
    const fs: FileSystem = {
      directoryExists: this.memory.directoryExists,
      fileExists: this.memory.fileExists,
      getAccessibleEntries: this.memory.getAccessibleEntries,
      realpath: this.memory.realpath,
      readFile: (path) => this.memory.readFile?.(normalizeTypeScriptPath(path)) ?? null,
    };
    this.api = new API({ cwd: this.root, fs });
  }

  run<T>(sources: TypeScriptSourceMap, callback: (context: TypeScriptAuditProject) => T): T {
    if (this.running) throw new Error("Nested isolated TypeScript audits are not supported");
    const entries = sourceEntries(sources);
    if (entries.length === 0) throw new Error("An isolated TypeScript project requires a source");

    this.runId += 1;
    const runRoot = `${this.root}/run-${this.runId}`;
    const relativeSources = entries.map(
      ([path, source]) => [fixtureRelativePath(path), source] as const,
    );
    const nextSourcePaths = new Set(relativeSources.map(([path]) => `${runRoot}/${path}`));
    const previousSourcePaths = this.sourcePaths;
    for (const oldPath of previousSourcePaths) {
      if (!nextSourcePaths.has(oldPath)) this.memory.removeFile?.(oldPath);
    }
    for (const [path, source] of relativeSources) {
      this.memory.writeFile?.(`${runRoot}/${path}`, source);
    }
    this.memory.writeFile?.(
      this.configPath,
      JSON.stringify({
        compilerOptions: {
          jsx: "preserve",
          noLib: true,
          strict: true,
          target: "esnext",
        },
        files: relativeSources.map(([path]) => `run-${this.runId}/${path}`),
      }),
    );
    this.sourcePaths = nextSourcePaths;

    this.running = true;
    let snapshot: Snapshot | undefined;
    try {
      snapshot = this.opened
        ? this.api.updateSnapshot({
            fileChanges: {
              changed: [this.configPath],
              created: [...nextSourcePaths],
              deleted: [...previousSourcePaths],
            },
          })
        : this.api.updateSnapshot({ openProjects: [this.configPath] });
      this.opened = true;
      const project = snapshot.getProject(this.configPath);
      if (!project) throw new Error(`TypeScript project did not open ${this.configPath}`);
      return callback({
        project,
        checker: project.checker,
        sourceRoot: runRoot,
        sourceFile: (path) =>
          requiredSourceFile(project, `${runRoot}/${fixtureRelativePath(path)}`),
      });
    } finally {
      snapshot?.dispose();
      this.running = false;
    }
  }

  close(): void {
    try {
      if (this.opened) releaseProject(this.api, this.configPath);
    } finally {
      this.opened = false;
      this.api.close();
    }
  }
}

class ProductionProjectHost {
  private readonly api: API;
  private snapshot: Snapshot | undefined;
  private context: TypeScriptAuditProject | undefined;

  constructor(private readonly configPath: string) {
    this.api = new API({ cwd: dirname(configPath) });
  }

  run<T>(callback: (context: TypeScriptAuditProject) => T): T {
    if (!this.snapshot) {
      this.snapshot = this.api.updateSnapshot({ openProjects: [this.configPath] });
      const project = this.snapshot.getProject(this.configPath);
      if (!project) throw new Error(`TypeScript project did not open ${this.configPath}`);
      this.context = {
        project,
        checker: project.checker,
        sourceRoot: normalizeTypeScriptPath(dirname(this.configPath)),
        sourceFile: (path) => requiredSourceFile(project, normalizeTypeScriptPath(resolve(path))),
      };
    }
    return callback(this.context!);
  }

  close(): void {
    const opened = this.snapshot !== undefined;
    this.snapshot?.dispose();
    this.snapshot = undefined;
    this.context = undefined;
    try {
      if (opened) releaseProject(this.api, this.configPath);
    } finally {
      this.api.close();
    }
  }
}

let isolatedHost: IsolatedProjectHost | undefined;
const productionHosts = new Map<string, ProductionProjectHost>();

export function withIsolatedTypeScriptProject<T>(
  sources: TypeScriptSourceMap,
  callback: (context: TypeScriptAuditProject) => T,
): T {
  isolatedHost ??= new IsolatedProjectHost();
  return isolatedHost.run(sources, callback);
}

export function withProductionTypeScriptProject<T>(
  callback: (context: TypeScriptAuditProject) => T,
  configPath = resolve("tsconfig.json"),
): T {
  const absoluteConfigPath = normalizeTypeScriptPath(resolve(configPath));
  let host = productionHosts.get(absoluteConfigPath);
  if (!host) {
    host = new ProductionProjectHost(absoluteConfigPath);
    productionHosts.set(absoluteConfigPath, host);
  }
  return host.run(callback);
}

export function closeTypeScriptAuditResources(): void {
  isolatedHost?.close();
  isolatedHost = undefined;
  for (const host of productionHosts.values()) host.close();
  productionHosts.clear();
}

export function visitTypeScriptAst(root: Node, visitor: (node: Node) => void): void {
  visitor(root);
  root.forEachChild((child) => visitTypeScriptAst(child, visitor));
}

afterAll(closeTypeScriptAuditResources);
