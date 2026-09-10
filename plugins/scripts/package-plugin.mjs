import { execFileSync } from "node:child_process";
import {
  createHash,
  createPrivateKey,
  createPublicKey,
  generateKeyPairSync,
  sign,
} from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync, renameSync, rmSync } from "node:fs";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";

export const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
export function canonical(value) {
  if (typeof value === "number" && !Number.isFinite(value))
    throw new Error("Non-finite package metadata");
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object")
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`)
      .join(",")}}`;
  return JSON.stringify(value);
}
export function validateVersion(version) {
  const match =
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/.exec(
      version,
    );
  if (
    !match ||
    match[4]
      ?.split(".")
      .some((part) => /^\d+$/.test(part) && part.length > 1 && part.startsWith("0"))
  )
    throw new Error("Plugin source manifest requires a valid release version");
}
function childPath(root, name) {
  if (isAbsolute(name) || name.includes("\\") || name.includes(":"))
    throw new Error("Invalid package relative path");
  const path = resolve(root, name);
  const part = relative(resolve(root), path);
  if (
    !part ||
    part === ".." ||
    part.startsWith("../") ||
    part.startsWith("..\\") ||
    /^[a-z]:/i.test(part)
  )
    throw new Error("Package asset escapes its root");
  return path;
}
export function signedPayload(manifest, files, keyId) {
  return Buffer.from(canonical({ schemaVersion: 1, keyId, manifest, files }));
}
function htmlAsset(webRoot, entry) {
  let html = readFileSync(childPath(webRoot, entry), "utf8");
  html = html.replace(
    /<script[^>]+src="([^"]+)"[^>]*><\/script>/g,
    (_tag, path) =>
      `<script type="module">${readFileSync(childPath(webRoot, path.replace(/^\//, "")), "utf8").replaceAll("</script", "<\\/script")}</script>`,
  );
  html = html.replace(
    /<link[^>]+href="([^"]+\.css)"[^>]*>/g,
    (_tag, path) =>
      `<style>${readFileSync(childPath(webRoot, path.replace(/^\//, "")), "utf8")}</style>`,
  );
  return html.replace(
    "<head>",
    "<head><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; font-src data:; img-src data:; connect-src 'none'; base-uri 'none'; form-action 'none'\">",
  );
}

export function packagePlugin({
  manifestPath,
  executablePath,
  webRoot,
  output,
  development = false,
  signingKeyPath = process.env.YSSBI_PLUGIN_SIGNING_KEY,
}) {
  if (process.platform !== "win32" || process.arch !== "x64")
    throw new Error("The current archive target requires Windows x64");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  validateVersion(manifest.version);
  if (!/^[a-z0-9][a-z0-9._-]{0,95}$/.test(manifest.id) || manifest.id.includes(".."))
    throw new Error("Invalid plugin identity");
  if (manifest.target !== "x86_64-pc-windows-msvc") throw new Error("Unsupported package target");
  manifest.cacheDirectories ??= [];
  const assets = new Map([[manifest.executable, readFileSync(executablePath)]]);
  for (const view of manifest.contributes.views) {
    if (!view.entry.startsWith("web/")) throw new Error("View entry must be under web/");
    assets.set(view.entry, Buffer.from(htmlAsset(webRoot, view.entry.slice(4))));
  }
  const files = [...assets]
    .sort(([left], [right]) => Buffer.compare(Buffer.from(left), Buffer.from(right)))
    .map(([path, bytes]) => ({ path, size: String(bytes.length), sha256: hash(bytes) }));
  if (development) {
    const content = hash(Buffer.from(canonical({ manifest, files })));
    const base = manifest.version.split("+")[0];
    manifest.version = `${base}${base.includes("-") ? "." : "-"}dev.${Date.now()}.${content.slice(0, 12)}`;
  }
  output = resolve(output);
  mkdirSync(output, { recursive: true });
  const keyPath = signingKeyPath ?? join(output, "development-signing-key.pem");
  if (!existsSync(keyPath)) {
    if (signingKeyPath) throw new Error("Signing key file does not exist");
    const pair = generateKeyPairSync("ed25519");
    writeFileSync(keyPath, pair.privateKey.export({ format: "pem", type: "pkcs8" }), {
      mode: 0o600,
    });
  }
  const key = createPrivateKey(readFileSync(keyPath));
  const publicKey = createPublicKey(key).export({ format: "der", type: "spki" }).subarray(-32);
  const keyId = hash(publicKey);
  const signed = signedPayload(manifest, files, keyId);
  const digest = hash(signed);
  const archive = childPath(
    output,
    `${manifest.id}-${manifest.version}-${manifest.target}.yssplugin`,
  );
  const receipt = `${archive}.json`;
  if (existsSync(archive)) {
    const previous = existsSync(receipt) ? JSON.parse(readFileSync(receipt, "utf8")) : null;
    if (previous?.packageDigest === digest && previous.archiveHash === hash(readFileSync(archive)))
      return archive;
    throw new Error(
      "This release version already has different content. Bump plugin.json version or use --dev for a development package.",
    );
  }
  const stage = childPath(output, `.stage-${manifest.id}-${process.pid}-${Date.now()}`);
  mkdirSync(stage);
  const temporary = `${archive}.${process.pid}.zip`;
  try {
    for (const [path, bytes] of assets) {
      const target = childPath(stage, path);
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, bytes);
    }
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
    const quote = (value) => "'" + value.replaceAll("'", "''") + "'";
    const names = [
      ...files.map((file) => file.path),
      "plugin.json",
      "files.json",
      "signature.json",
    ];
    const script =
      "Add-Type -AssemblyName System.IO.Compression; Add-Type -AssemblyName System.IO.Compression.FileSystem; $pluginZip=[System.IO.Compression.ZipFile]::Open(" +
      quote(temporary) +
      ",[System.IO.Compression.ZipArchiveMode]::Create); try { " +
      names
        .map(
          (name) =>
            "[void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($pluginZip," +
            quote(childPath(stage, name)) +
            "," +
            quote(name) +
            ",[System.IO.Compression.CompressionLevel]::Optimal);",
        )
        .join(" ") +
      " } finally { $pluginZip.Dispose() }";
    execFileSync("powershell", ["-NoProfile", "-NonInteractive", "-Command", script], {
      stdio: "inherit",
      windowsHide: true,
    });
    renameSync(temporary, archive);
    writeFileSync(
      receipt,
      JSON.stringify(
        {
          version: manifest.version,
          packageDigest: digest,
          archiveHash: hash(readFileSync(archive)),
        },
        null,
        2,
      ),
    );
    return archive;
  } finally {
    rmSync(childPath(output, basename(stage)), { recursive: true, force: true });
    if (existsSync(temporary)) rmSync(childPath(output, basename(temporary)));
  }
}
