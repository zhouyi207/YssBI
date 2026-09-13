import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const metadata = JSON.parse(
  execFileSync(
    "cargo",
    [
      "metadata",
      "--manifest-path",
      resolve(root, "src-tauri/Cargo.toml"),
      "--no-deps",
      "--offline",
      "--format-version",
      "1",
    ],
    { cwd: root, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
  ),
);
const members = new Set(metadata.workspace_members);
const packages = metadata.packages.filter((item) => members.has(item.id));
const names = new Set(packages.map((item) => item.name));
const crates = packages
  .map((item) => ({
    name: item.name,
    manifest: relative(root, item.manifest_path).replaceAll("\\", "/"),
    dependencies: item.dependencies
      .filter((dep) => dep.path && names.has(dep.name) && dep.kind !== "dev")
      .map((dep) => ({
        name: dep.name,
        kind: dep.kind ?? "normal",
        optional: dep.optional,
        target: dep.target,
      }))
      .sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b), "en")),
  }))
  .sort((a, b) => a.name.localeCompare(b.name, "en"));
const output = resolve(root, "src/modules/workbench/internal/ui/menu/crateDependencies.json");
const text = JSON.stringify(crates, null, 2) + "\n";
if (process.argv.includes("--check")) {
  if (readFileSync(output, "utf8").replaceAll("\r\n", "\n") !== text)
    throw new Error("Crate dependency snapshot is stale; run pnpm docs:crate-dependencies");
} else writeFileSync(output, text);
console.log(
  `${crates.length} workspace crates; ${crates.reduce((n, item) => n + item.dependencies.length, 0)} dependency declarations`,
);
