import { afterEach, expect, it, vi } from "vitest";
import {
  ModuleDependencyResolutionError,
  resolvedModuleDependencies,
} from "@/tests/helpers/moduleDependencyAudit";
import { withIsolatedTypeScriptProject } from "@/tests/helpers/typescriptAudit";

afterEach(() => vi.restoreAllMocks());

it("shares the path inventory across sources and avoids resolving the same dependencies twice", () => {
  const path = "src/views/consumer.ts";
  const source = "import { first, second } from './values'; void first; void second;";
  const otherPath = "src/views/other.ts";
  const otherSource = "export { first } from './values';";

  withIsolatedTypeScriptProject(
    {
      [path]: source,
      [otherPath]: otherSource,
      "src/views/values.ts": "export const first = 1; export const second = 2;",
    },
    (context) => {
      const inventory = vi.spyOn(context.project.program, "getSourceFileNames");
      const symbols = vi.spyOn(context.checker, "getSymbolAtLocation");
      const dependencies = resolvedModuleDependencies(context, { path, source });
      expect(dependencies.map((dependency) => dependency.canonicalOriginTarget)).toEqual([
        "src/views/values.ts::first",
        "src/views/values.ts::second",
      ]);

      const symbolQueries = symbols.mock.calls.length;
      expect(resolvedModuleDependencies(context, { path, source })).toEqual(dependencies);
      expect.soft(symbols).toHaveBeenCalledTimes(symbolQueries);
      expect(
        resolvedModuleDependencies(context, { path: otherPath, source: otherSource }).map(
          (dependency) => dependency.canonicalOriginTarget,
        ),
      ).toEqual(["src/views/values.ts::first"]);
      expect(inventory).toHaveBeenCalledTimes(1);
    },
  );
});

it("refreshes cached paths and dependencies when a later snapshot moves or removes a module", () => {
  const path = "src/views/consumer.ts";
  const source = "import './side-effect';";

  for (const target of ["src/views/side-effect.ts", "src/views/side-effect/index.ts"]) {
    withIsolatedTypeScriptProject({ [path]: source, [target]: "void 0;" }, (context) => {
      expect(
        resolvedModuleDependencies(context, { path, source }).map(
          (dependency) => dependency.canonicalOriginTarget,
        ),
      ).toEqual([target]);
    });
  }

  withIsolatedTypeScriptProject({ [path]: source }, (context) => {
    expect(() => resolvedModuleDependencies(context, { path, source })).toThrowError(
      ModuleDependencyResolutionError,
    );
  });
});
