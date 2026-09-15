import { readFile, writeFile, mkdir } from "node:fs/promises";
import { createRequire } from "node:module";
import { resolve, dirname } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { createHash } from "node:crypto";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../../../..");
const require = createRequire(resolve(root, "package.json"));
process.env.NODE_ENV = "production";
const experimentalModules = process.argv[2];
if (!experimentalModules)
  throw new Error(
    "Pass the isolated probe's node_modules directory. See the implementation review.",
  );
const experimental = createRequire(resolve(experimentalModules, "../package.json"));
const { build } = createRequire(require.resolve("vite/package.json"))("esbuild");
const { Window } = require("happy-dom");
const window = new Window({ url: "http://localhost:1420" });
window.document.write("<!DOCTYPE html><html><head></head><body></body></html>");
for (const key of [
  "window",
  "document",
  "navigator",
  "HTMLElement",
  "SVGElement",
  "Element",
  "Node",
  "ResizeObserver",
  "MutationObserver",
  "getComputedStyle",
  "requestAnimationFrame",
  "cancelAnimationFrame",
  "localStorage",
]) {
  Object.defineProperty(globalThis, key, {
    configurable: true,
    value:
      typeof window[key] === "function" && /^[a-z]/.test(key)
        ? window[key].bind(window)
        : window[key],
  });
}
const { createElement } = require("react");
const { createRoot } = require("react-dom/client");
const { flushSync } = require("react-dom");
const outputDirectory = resolve(root, "node_modules/.cache/ols-renderer-comparison");
await mkdir(outputDirectory, { recursive: true });
const rawSample = JSON.parse(
  await readFile(resolve(directory, "../ols-layout-result.json"), "utf8"),
);
const source = rawSample.report.resultRef;
const spec = {
  schemaVersion: 1,
  type: "regressionReport",
  source,
  sections: ["modelSummary", "coefficientTable", "residualPlot"].map((kind) => ({
    id: kind,
    kind,
    visible: true,
  })),
};
const failures = {
  version: { ...spec, schemaVersion: 2 },
  result: { ...spec, source: { ...source, resultId: "999999" } },
  numericFact: { ...spec, r_squared: 0.99 },
  action: { ...spec, actions: [{ command: "execute_graph" }] },
  unknownKind: { ...spec, sections: [{ id: "a", kind: "Metric", visible: true }] },
  duplicateId: {
    ...spec,
    sections: spec.sections.map((section) => ({ ...section, id: "duplicate" })),
  },
  nested: { ...spec, sections: [{ ...spec.sections[0], children: ["modelSummary"] }] },
  count: { ...spec, sections: Array(11).fill(spec.sections[0]) },
};
const quantiles = (values) => {
  values.sort((a, b) => a - b);
  return Object.fromEntries(
    [50, 95, 99].map((p) => [
      `p${p}Ms`,
      values[Math.min(values.length - 1, Math.ceil((values.length * p) / 100) - 1)],
    ]),
  );
};
const results = {};
for (const name of ["typed", "catalog"]) {
  const entry = resolve(directory, `${name}.jsx`);
  const bundle = await build({
    entryPoints: [entry],
    bundle: true,
    write: false,
    format: "esm",
    platform: "browser",
    minify: true,
    jsx: "automatic",
    tsconfig: resolve(root, "tsconfig.json"),
    define: { "process.env.NODE_ENV": '"production"', "import.meta.env.DEV": "false" },
    plugins: [
      {
        name: "isolated-probe-dependencies",
        setup(builder) {
          builder.onResolve({ filter: /^(react|react-dom)(\/.*)?$/ }, (args) => ({
            path: args.path,
            external: true,
          }));
          builder.onResolve({ filter: /^(@json-render\/|zod)/ }, (args) => ({
            path: experimental.resolve(args.path),
          }));
        },
      },
    ],
  });
  const bytes = bundle.outputFiles[0].contents;
  const moduleFile = resolve(outputDirectory, `${name}.mjs`);
  await writeFile(
    moduleFile,
    'import { createRequire } from "node:module"; const require = createRequire(import.meta.url);\n' +
      Buffer.from(bytes).toString(),
  );
  const candidate = await import(pathToFileURL(moduleFile));
  const sample = candidate.prepareSample(rawSample);
  const validated = candidate.validate(spec, source);
  if (!validated.ok) throw new Error(JSON.stringify(validated));
  const validationTimes = [];
  for (let i = 0; i < 2_000; i++) {
    const start = performance.now();
    candidate.validate(spec, source);
    if (i >= 100) validationTimes.push(performance.now() - start);
  }
  const host = document.createElement("main");
  document.body.append(host);
  let renderError;
  const reactRoot = createRoot(host, {
    onUncaughtError(error) {
      renderError = error;
    },
  });
  flushSync(() =>
    reactRoot.render(createElement(candidate.Render, { spec: validated.value, sample })),
  );
  if (renderError || !host.textContent.includes("R-squared"))
    throw new Error(`Probe did not render ${name}: ${renderError?.message ?? "empty report"}`);
  const markup = host.innerHTML.replace(/_r_[a-z0-9]+_/g, "_id_");
  const updateTimes = [];
  for (let i = 0; i < 150; i++) {
    // A single visibility edit leaves the Result DTO and other section identities intact.
    const next = candidate.validate(
      {
        ...spec,
        sections: spec.sections.map((section, index) =>
          index === 0 ? { ...section, visible: i % 2 === 0 } : section,
        ),
      },
      source,
    );
    if (!next.ok) throw new Error(JSON.stringify(next));
    const start = performance.now();
    flushSync(() =>
      reactRoot.render(createElement(candidate.Render, { spec: next.value, sample })),
    );
    if (renderError) throw new Error(`Probe update failed for ${name}: ${renderError.message}`);
    if (i >= 20) updateTimes.push(performance.now() - start);
  }
  flushSync(() => reactRoot.unmount());
  host.remove();
  const adapter = await readFile(entry, "utf8");
  results[name] = {
    adapterLines: adapter.trim().split(/\r?\n/).length,
    bundledBytes: bytes.length,
    gzipBytes: gzipSync(bytes).length,
    validation: quantiles(validationTimes),
    emulatedDomUpdate: quantiles(updateTimes),
    markupHash: createHash("sha256").update(markup).digest("hex"),
    invalidSpecs: Object.fromEntries(
      Object.entries(failures).map(([key, raw]) => [key, candidate.validate(raw, source)]),
    ),
  };
}
const report = {
  environment: {
    node: process.version,
    react: require("react/package.json").version,
    jsonRender: JSON.parse(
      await readFile(resolve(experimentalModules, "@json-render/react/package.json"), "utf8"),
    ).version,
    zod: JSON.parse(await readFile(resolve(experimentalModules, "zod/package.json"), "utf8"))
      .version,
  },
  scope:
    "Real Rust OLS DTO; production section components; minified bundles excluding shared React. DOM updates use happy-dom, without real layout/paint, IPC or desktop latency.",
  results,
};
await writeFile(
  resolve(directory, "../report-layout-comparison.json"),
  JSON.stringify(report, null, 2) + "\n",
);
console.log(JSON.stringify(report, null, 2));
await window.happyDOM.abort();
