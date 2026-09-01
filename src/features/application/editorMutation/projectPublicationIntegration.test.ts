import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

function productionFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return productionFiles(path);
    if (!entry.isFile() || !/\.tsx?$/.test(entry.name) || /\.test\.tsx?$/.test(entry.name)) {
      return [];
    }
    return [path];
  });
}

describe("project publication integration boundary", () => {
  it("has a single project-event publication owner", () => {
    const owners = productionFiles("src/features/application").filter((path) =>
      readFileSync(path, "utf8").includes("publishResourceMutationCommitted:"),
    );

    expect(owners).toHaveLength(1);
  });
});
