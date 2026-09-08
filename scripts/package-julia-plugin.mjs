import { execFileSync } from "node:child_process";
import {
  createHash,
  createPrivateKey,
  createPublicKey,
  generateKeyPairSync,
  sign,
} from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync, renameSync } from "node:fs";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
if (process.platform !== "win32" || process.arch !== "x64")
  throw new Error("This release target requires Windows x64.");
const run = (name, args) => execFileSync(name, args, { cwd: root, stdio: "inherit" });
const release = process.argv.includes("--release");
if (!process.argv.includes("--skip-build")) {
  run(process.execPath, [
    resolve(root, "node_modules/vite/bin/vite.js"),
    "build",
    "--config",
    "plugins/julia/web/vite.config.ts",
  ]);
  run("cargo", [
    "build",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    "-p",
    "yss-julia-extension",
    ...(release ? ["--release"] : []),
  ]);
}
const target = "x86_64-pc-windows-msvc";
const output = resolve(root, "target/plugin-packages");
mkdirSync(output, { recursive: true });
const keyPath = process.env.YSSBI_PLUGIN_SIGNING_KEY ?? join(output, "development-signing-key.pem");
if (!existsSync(keyPath)) {
  if (process.env.YSSBI_PLUGIN_SIGNING_KEY) throw new Error("Signing key file does not exist.");
  const pair = generateKeyPairSync("ed25519");
  writeFileSync(keyPath, pair.privateKey.export({ format: "pem", type: "pkcs8" }), { mode: 0o600 });
}
const key = createPrivateKey(readFileSync(keyPath));
const publicKey = createPublicKey(key).export({ format: "der", type: "spki" }).subarray(-32);
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const dist = resolve(root, "plugins/julia/web/dist");
let html = readFileSync(join(dist, "index.html"), "utf8");
html = html.replace(
  /<script[^>]+src="([^"]+)"[^>]*><\/script>/g,
  (_match, path) =>
    `<script type="module">${readFileSync(join(dist, path.replace(/^\//, "")), "utf8").replaceAll("</script", "<\\/script")}</script>`,
);
html = html.replace(
  /<link[^>]+href="([^"]+\.css)"[^>]*>/g,
  (_match, path) => `<style>${readFileSync(join(dist, path.replace(/^\//, "")), "utf8")}</style>`,
);
html = html.replace(
  "<head>",
  `<head><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; font-src data:; img-src data:; connect-src 'none'; base-uri 'none'; form-action 'none'">`,
);
const executable = readFileSync(
  resolve(root, `src-tauri/target/${release ? "release" : "debug"}/yss-julia-extension.exe`),
);
const version = `0.1.0+${hash(Buffer.concat([executable, Buffer.from(html)])).slice(0, 12)}`;
const stage = join(output, `yssbi.julia-${version}`);
mkdirSync(join(stage, "bin", target), { recursive: true });
mkdirSync(join(stage, "web"), { recursive: true });
writeFileSync(join(stage, "bin", target, "yss-julia-extension.exe"), executable);
writeFileSync(join(stage, "web/index.html"), html);
const commands = [
  "parse_bayes_expression",
  "validate_bayes_model",
  "read_bayes_trace_plot_data",
  "read_bayes_density_plot_data",
  "read_bayes_autocorrelation_data",
  "read_bayes_posterior_predictive",
  "export_bayes_artifact_csv",
];
const manifest = {
  schemaVersion: 1,
  id: "yssbi.julia",
  name: "Julia 计算支持",
  description: "Julia 科学计算、贝叶斯建模与结果分析。",
  publisher: "yssbi",
  version,
  hostApi: "^1.0",
  protocol: { major: 1, minMinor: 0, maxMinor: 0, requiredFeatures: [] },
  target,
  executable: `bin/${target}/yss-julia-extension.exe`,
  execution: "trustedNative",
  contributes: {
    views: [
      {
        id: "runtime",
        title: "Julia",
        entry: "web/index.html",
        location: "sidebar",
        scope: "application",
      },
      {
        id: "analysis",
        title: "贝叶斯分析",
        entry: "web/index.html",
        location: "editor",
        scope: "project",
      },
    ],
    commands: commands.map((id) => ({ id, title: id })),
    taskTypes: [
      { id: "bayes.inference", producesArtifacts: true },
      { id: "runtime.prepare", producesArtifacts: false },
    ],
  },
  permissions: ["data.read", "results.write", "files.export"],
  uiMethods: [
    "commands.execute",
    "tasks.start",
    "tasks.get",
    "tasks.cancel",
    "tasks.result",
    "dependencies.inspect",
    "data.list",
    "views.open",
    "system.save_file",
    "system.reveal_artifact",
  ],
  resourceBudget: {
    frameBytes: 2097152,
    pendingRequests: 32,
    activeTasks: 4,
    queuedBytes: 8388608,
    snapshotBytes: 134217728,
    privateStorageBytes: 1073741824,
    views: 4,
  },
};
const files = [manifest.executable, "web/index.html"].sort().map((path) => {
  const bytes = readFileSync(join(stage, path));
  return { path, size: String(bytes.length), sha256: hash(bytes) };
});
const canonical = (value) =>
  JSON.stringify(
    value && typeof value === "object"
      ? Array.isArray(value)
        ? value.map((item) => JSON.parse(canonical(item)))
        : Object.fromEntries(
            Object.keys(value)
              .sort()
              .map((name) => [name, JSON.parse(canonical(value[name]))]),
          )
      : value,
  );
const keyId = hash(publicKey);
const signed = Buffer.from(canonical({ schemaVersion: 1, keyId, manifest, files }));
for (const [name, value] of Object.entries({
  "plugin.json": manifest,
  "files.json": files,
  "signature.json": {
    keyId,
    publicKey: publicKey.toString("hex"),
    signature: sign(null, signed, key).toString("hex"),
  },
}))
  writeFileSync(join(stage, name), JSON.stringify(value, null, 2));
const archive = join(output, `yssbi.julia-${version}-${target}.yssplugin`);
{
  const quote = (value) => "'" + value.replaceAll("'", "''") + "'";
  const temporary = archive + "." + Date.now() + ".zip";
  const entries = [
    ...files.map((file) => file.path),
    "plugin.json",
    "files.json",
    "signature.json",
  ];
  const script =
    "Add-Type -AssemblyName System.IO.Compression; Add-Type -AssemblyName System.IO.Compression.FileSystem; $pluginZip=[System.IO.Compression.ZipFile]::Open(" +
    quote(temporary) +
    ",[System.IO.Compression.ZipArchiveMode]::Create); try { " +
    entries
      .map(
        (name) =>
          "[void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($pluginZip," +
          quote(join(stage, name)) +
          "," +
          quote(name) +
          ",[System.IO.Compression.CompressionLevel]::Optimal);",
      )
      .join(" ") +
    " } finally { $pluginZip.Dispose() }";
  run("powershell", ["-NoProfile", "-NonInteractive", "-Command", script]);
  renameSync(temporary, archive);
}
console.log(archive);
