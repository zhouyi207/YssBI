import { execFileSync } from "node:child_process";
import { readdirSync, statSync } from "node:fs";
import { resolve } from "node:path";
const directory = resolve("target/plugin-packages");
const packages = process.env.YSSBI_PLUGIN_TEST_PACKAGE
  ? [resolve(process.env.YSSBI_PLUGIN_TEST_PACKAGE)]
  : readdirSync(directory)
      .filter((name) => name.startsWith("yssbi.julia-") && name.endsWith(".yssplugin"))
      .map((name) => resolve(directory, name))
      .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
if (!packages[0]) throw new Error("Build a Julia plugin package before running this check.");
execFileSync(
  "cargo",
  [
    "test",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    "-p",
    "yss-plugin-runtime",
    "--test",
    "native_extension",
    "--",
    "--ignored",
    "--nocapture",
  ],
  { stdio: "inherit", env: { ...process.env, YSSBI_PLUGIN_TEST_PACKAGE: packages[0] } },
);
