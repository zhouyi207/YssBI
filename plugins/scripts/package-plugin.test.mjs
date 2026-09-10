import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { hash, packagePlugin, signedPayload, validateVersion } from "./package-plugin.mjs";

const fixture = new URL("../julia/plugin.json", import.meta.url);
test("signed identity includes permissions and granted-budget requests", () => {
  const manifest = JSON.parse(readFileSync(fixture, "utf8"));
  const files = [{ path: manifest.executable, size: "1", sha256: "a".repeat(64) }];
  const before = hash(signedPayload(manifest, files, "key"));
  manifest.permissions.push("test.permission");
  assert.notEqual(hash(signedPayload(manifest, files, "key")), before);
  manifest.permissions.pop();
  manifest.resourceBudget.privateStorageBytes -= 1;
  assert.notEqual(hash(signedPayload(manifest, files, "key")), before);
  assert.throws(() => validateVersion("1.0.0-01"));
  assert.doesNotThrow(() => validateVersion("1.2.3+trace"));
});

test(
  "release packaging rejects a manifest-only change until version advances",
  { skip: process.platform !== "win32" },
  () => {
    const root = mkdtempSync(join(tmpdir(), "yss-package-version-"));
    try {
      const manifest = JSON.parse(readFileSync(fixture, "utf8"));
      manifest.version = "9.9.0";
      const manifestPath = join(root, "plugin.json");
      const executablePath = join(root, "plugin.exe");
      const webRoot = join(root, "web");
      mkdirSync(webRoot);
      writeFileSync(executablePath, "fixture");
      writeFileSync(join(webRoot, "index.html"), "<html><head></head><body>fixture</body></html>");
      writeFileSync(manifestPath, JSON.stringify(manifest));
      const options = { manifestPath, executablePath, webRoot, output: join(root, "packages") };
      const first = packagePlugin(options);
      assert.equal(packagePlugin(options), first);
      manifest.resourceBudget.snapshotBytes -= 1;
      writeFileSync(manifestPath, JSON.stringify(manifest));
      assert.throws(() => packagePlugin(options), /Bump plugin.json version/);
      manifest.version = "9.9.1";
      writeFileSync(manifestPath, JSON.stringify(manifest));
      assert.notEqual(packagePlugin(options), first);
    } finally {
      assert.ok(resolve(root).startsWith(resolve(tmpdir())));
      rmSync(root, { recursive: true, force: true });
    }
  },
);
