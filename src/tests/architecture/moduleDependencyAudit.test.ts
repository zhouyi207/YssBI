import { expect, it } from "vitest";
import {
  ModuleDependencyResolutionError,
  resolvedModuleDependencies,
} from "@/tests/helpers/moduleDependencyAudit";
import { withIsolatedTypeScriptProject } from "@/tests/helpers/typescriptAudit";

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
