import { spawnSync } from "node:child_process";
import { existsSync, lstatSync, readFileSync, readdirSync, readlinkSync, statSync } from "node:fs";
import { dirname, extname, relative, resolve } from "node:path";
import { describe, expect, it } from "vitest";

const REPOSITORY_ROOT = resolve(".");
const DOCS_ROOT = resolve(REPOSITORY_ROOT, "docs");

function markdownFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) return markdownFiles(path);
      return entry.isFile() && extname(entry.name) === ".md" ? [path] : [];
    })
    .sort();
}

function repositoryPath(path: string): string {
  return relative(REPOSITORY_ROOT, path).replace(/\\/gu, "/");
}

function documentStatus(source: string): string | null {
  return /^> Status: (.+)$/mu.exec(source)?.[1]?.trim() ?? null;
}

function markdownLinkTargets(source: string): string[] {
  return [...source.matchAll(/!?\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/gu)].map((match) =>
    match[1]!.replace(/^<|>$/gu, ""),
  );
}

function currentDocumentation(): string[] {
  return markdownFiles(DOCS_ROOT).filter(
    (path) => documentStatus(readFileSync(path, "utf8")) === "Current",
  );
}

describe("documentation contract", () => {
  it("indexes every current architecture and development document", () => {
    const indexPath = resolve(DOCS_ROOT, "README.md");
    const indexedTargets = new Set(
      markdownLinkTargets(readFileSync(indexPath, "utf8"))
        .filter((target) => !target.startsWith("#") && !/^[a-z]+:/iu.test(target))
        .map((target) => resolve(dirname(indexPath), decodeURI(target.split("#", 1)[0]!))),
    );
    const maintained = ["architecture", "development"].flatMap((directory) =>
      markdownFiles(resolve(DOCS_ROOT, directory)),
    );

    const wrongStatus = maintained
      .filter((path) => documentStatus(readFileSync(path, "utf8")) !== "Current")
      .map(repositoryPath);
    const unindexed = maintained.filter((path) => !indexedTargets.has(path)).map(repositoryPath);

    expect(wrongStatus).toEqual([]);
    expect(unindexed).toEqual([]);
  });

  it("keeps maintained relative Markdown links resolvable", () => {
    const maintained = markdownFiles(DOCS_ROOT).filter(
      (path) => !repositoryPath(path).startsWith("docs/version/"),
    );
    const missing: string[] = [];

    for (const documentPath of maintained) {
      const source = readFileSync(documentPath, "utf8");
      for (const rawTarget of markdownLinkTargets(source)) {
        if (rawTarget.startsWith("#") || /^[a-z]+:/iu.test(rawTarget)) continue;
        const targetWithoutFragment = rawTarget.split("#", 1)[0];
        if (!targetWithoutFragment) continue;
        const target = resolve(dirname(documentPath), decodeURI(targetWithoutFragment));
        if (!existsSync(target)) {
          missing.push(`${repositoryPath(documentPath)} -> ${rawTarget}`);
        }
      }
    }

    expect(missing).toEqual([]);
  });

  it("keeps explicit source paths in current documents valid", () => {
    const missing: string[] = [];
    for (const documentPath of currentDocumentation()) {
      const source = readFileSync(documentPath, "utf8");
      const codeSpans = [...source.matchAll(/(?<!`)`([^`\r\n]+)`(?!`)/gu)].map(
        (match) => match[1]!,
      );
      for (const value of codeSpans) {
        if (!/^(?:src|src-tauri|scripts)\//u.test(value)) continue;
        if (/[<>*|]/u.test(value) || /\s/u.test(value)) continue;
        const normalized = value.split("#", 1)[0]!.replace(/[,:;.]$/u, "");
        if (normalized === "src-tauri/target/") continue;
        const target = resolve(REPOSITORY_ROOT, normalized);
        if (!existsSync(target) || !(statSync(target).isFile() || statSync(target).isDirectory())) {
          missing.push(`${repositoryPath(documentPath)} -> ${value}`);
        }
      }
    }

    expect(missing).toEqual([]);
  });

  it("keeps documented pnpm commands backed by package scripts or pnpm builtins", () => {
    const packageManifest = JSON.parse(
      readFileSync(resolve(REPOSITORY_ROOT, "package.json"), "utf8"),
    ) as { scripts?: Record<string, string> };
    const scripts = new Set(Object.keys(packageManifest.scripts ?? {}));
    const builtins = new Set(["add", "ci", "dlx", "exec", "install", "remove", "update"]);
    const unknown: string[] = [];

    for (const documentPath of currentDocumentation()) {
      const source = readFileSync(documentPath, "utf8");
      for (const match of source.matchAll(/\bpnpm\s+(?:run\s+)?([a-z0-9:_-]+)/giu)) {
        const command = match[1]!;
        if (!scripts.has(command) && !builtins.has(command)) {
          unknown.push(`${repositoryPath(documentPath)} -> pnpm ${command}`);
        }
      }
    }

    expect(unknown).toEqual([]);
  });

  it("keeps every agent entry file as a redirect to .rules", () => {
    for (const name of ["AGENTS.md", "CLAUDE.md", "GEMINI.md"]) {
      const path = resolve(REPOSITORY_ROOT, name);
      const target = lstatSync(path).isSymbolicLink()
        ? readlinkSync(path)
        : readFileSync(path, "utf8").trim();
      expect(target, name).toBe(".rules");
    }
  });

  it("separates document lifecycles and keeps the generated module map current", () => {
    const wrongStatus = [
      ["decisions", "Accepted Decision"],
      ["roadmap", "Planned"],
      ["reference", "Current"],
      ["version", "Historical"],
    ].flatMap(([directory, expectedStatus]) =>
      markdownFiles(resolve(DOCS_ROOT, directory))
        .filter((path) => documentStatus(readFileSync(path, "utf8")) !== expectedStatus)
        .map(repositoryPath),
    );
    expect(wrongStatus).toEqual([]);

    const generationCheck = spawnSync(
      process.execPath,
      [resolve(REPOSITORY_ROOT, "scripts/generate-module-map.mjs"), "--check"],
      { cwd: REPOSITORY_ROOT, encoding: "utf8" },
    );
    expect(generationCheck.status, generationCheck.stderr || generationCheck.stdout).toBe(0);
  });
});
