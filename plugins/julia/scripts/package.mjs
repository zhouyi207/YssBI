import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { packagePlugin } from "../../../scripts/package-plugin.mjs";

const root = resolve(fileURLToPath(new URL("../../..", import.meta.url)));
const release = process.argv.includes("--release");
if (!process.argv.includes("--skip-build")) {
  execFileSync(
    process.execPath,
    [
      resolve(root, "node_modules/vite/bin/vite.js"),
      "build",
      "--config",
      "plugins/julia/web/vite.config.ts",
    ],
    { cwd: root, stdio: "inherit", windowsHide: true },
  );
  execFileSync(
    "cargo",
    [
      "build",
      "--manifest-path",
      "plugins/julia/native/crates/yss-julia-extension/Cargo.toml",
      "-p",
      "yss-julia-extension",
      ...(release ? ["--release"] : []),
    ],
    { cwd: root, stdio: "inherit", windowsHide: true },
  );
}
const metadata = JSON.parse(
  execFileSync(
    "cargo",
    [
      "metadata",
      "--no-deps",
      "--format-version",
      "1",
      "--manifest-path",
      "plugins/julia/native/crates/yss-julia-extension/Cargo.toml",
    ],
    { cwd: root, encoding: "utf8", windowsHide: true },
  ),
);
console.log(
  packagePlugin({
    manifestPath: resolve(root, "plugins/julia/plugin.json"),
    executablePath: resolve(
      metadata.target_directory,
      `${release ? "release" : "debug"}/yss-julia-extension.exe`,
    ),
    webRoot: resolve(root, "plugins/julia/web/dist"),
    output: resolve(root, "target/plugin-packages"),
    development: process.argv.includes("--dev"),
  }),
);
