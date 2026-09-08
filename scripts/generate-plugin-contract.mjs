import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { format } from "oxfmt";

const schemas = JSON.parse(
  execFileSync(
    "cargo",
    [
      "run",
      "--quiet",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "-p",
      "yss-plugin-protocol",
      "--bin",
      "plugin-schema",
    ],
    { encoding: "utf8" },
  ),
);
const definitions = {};
for (const [name, schema] of Object.entries(schemas)) {
  Object.assign(definitions, schema.$defs ?? {});
  const { $defs: _defs, $schema: _schema, ...definition } = schema;
  definitions[name] = definition;
}
function type(schema) {
  if (schema === true || schema === undefined) return "unknown";
  if (schema === false) return "never";
  if (schema.$ref) return schema.$ref.split("/").at(-1);
  if (schema.const !== undefined) return JSON.stringify(schema.const);
  if (schema.enum) return schema.enum.map((value) => JSON.stringify(value)).join(" | ");
  if (schema.anyOf || schema.oneOf) return (schema.anyOf ?? schema.oneOf).map(type).join(" | ");
  if (Array.isArray(schema.type))
    return schema.type.map((value) => type({ ...schema, type: value })).join(" | ");
  switch (schema.type) {
    case "null":
      return "null";
    case "boolean":
      return "boolean";
    case "number":
    case "integer":
      return "number";
    case "string":
      return "string";
    case "array":
      return `Array<${type(schema.items)}>`;
    case "object":
      return `{ ${Object.entries(schema.properties ?? {})
        .map(
          ([name, value]) =>
            `${JSON.stringify(name)}${schema.required?.includes(name) ? "" : "?"}: ${type(value)};`,
        )
        .join(
          " ",
        )} ${schema.additionalProperties && typeof schema.additionalProperties === "object" ? `[key: string]: ${type(schema.additionalProperties)};` : ""} }`;
    default:
      return "unknown";
  }
}
const directory = resolve("src/shared/types/plugins");
mkdirSync(directory, { recursive: true });
const outputs = {
  "generated.ts":
    "// Generated from yss-plugin-protocol. Do not edit.\n" +
    Object.entries(definitions)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([name, schema]) => `export type ${name} = ${type(schema)};`)
      .join("\n") +
    "\n",
  "schema.json": JSON.stringify({ $defs: definitions }, null, 2) + "\n",
};
for (const [name, raw] of Object.entries(outputs)) {
  const formatted = await format(name, raw);
  if (formatted.errors.length) throw new Error(`Cannot format generated plugin contract: ${name}`);
  const content = formatted.code;
  const path = resolve(directory, name);
  if (process.argv.includes("--check")) {
    if (readFileSync(path, "utf8") !== content) throw new Error(`Plugin contract drift: ${name}`);
  } else writeFileSync(path, content);
}
