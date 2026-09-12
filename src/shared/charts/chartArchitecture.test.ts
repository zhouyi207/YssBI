import { posix } from "node:path";
import * as ts from "typescript/unstable/ast";
import { describe, expect, it } from "vitest";
import {
  resolvedModuleDependencies,
  type ArchitectureSource,
} from "@/tests/helpers/moduleDependencyAudit";
import { productionTypeScriptSources } from "@/tests/helpers/productionSourceAudit";
import {
  visitTypeScriptAst,
  withIsolatedTypeScriptProject,
  withProductionTypeScriptProject,
  type TypeScriptAuditProject,
} from "@/tests/helpers/typescriptAudit";

const forbiddenRules = [
  { from: /^src\/shared\/charts\//, to: /^@\/(?:views|modules|services|features)\// },
  { from: /^src\/shared\/charts\//, to: /^@\/shared\/types(?:\/|$)/ },
  { from: /^src\/shared\/charts\/cartesian\//, to: /^@\/shared\/charts\/statistical(?:\/|$)/ },
  { from: /^src\/shared\/charts\/statistical\//, to: /^@\/shared\/charts\/cartesian(?:\/|$)/ },
  {
    from: /^src\/shared\/charts\/(?:core|cartesian|statistical)\//,
    to: /^@\/shared\/charts(?:$|\/index(?:\.tsx?)?$)/,
  },
  { from: /^src\/shared\/types\/dto\//, to: /^@\/features\// },
];

function dependencyViolations(
  context: TypeScriptAuditProject,
  sources: readonly ArchitectureSource[],
): string[] {
  return sources.flatMap((source) => {
    const rules = forbiddenRules.filter((rule) => rule.from.test(source.path));
    if (!rules.length) return [];
    return resolvedModuleDependencies(context, source).flatMap((dependency) => {
      const written = dependency.writtenModuleSpecifier;
      const specifier = written.startsWith(".")
        ? `@/${posix.normalize(posix.join(posix.dirname(source.path), written)).slice(4)}`
        : written;
      const origin =
        dependency.origin.kind === "repository-module"
          ? `@/${dependency.canonicalOriginTarget.split("::")[0].slice(4)}`
          : specifier;
      return rules.some((rule) => rule.to.test(specifier) || rule.to.test(origin))
        ? [`${source.path} imports ${written}`]
        : [];
    });
  });
}

function lifecycleViolations(path: string, sourceFile: ts.SourceFile): string[] {
  const violations: string[] = [];
  visitTypeScriptAst(sourceFile, (node) => {
    let reason: string | null = null;
    if (
      ts.isNewExpression(node) &&
      ts.isIdentifier(node.expression) &&
      node.expression.text === "ResizeObserver" &&
      path !== "src/shared/charts/core/useChartContainerSize.ts"
    ) {
      reason = "creates ResizeObserver outside chart core";
    }
    if (
      ts.isCallExpression(node) &&
      ts.isPropertyAccessExpression(node.expression) &&
      node.expression.name.text === "remove" &&
      node.arguments.length === 0
    ) {
      const selection = node.expression.expression;
      if (ts.isCallExpression(selection) && selection.arguments.length === 1) {
        const method = selection.expression;
        const selector = selection.arguments[0];
        const selectsAll =
          (ts.isPropertyAccessExpression(method) && method.name.text === "selectAll") ||
          (ts.isIdentifier(method) && method.text === "selectAll");
        if (
          selectsAll &&
          (ts.isStringLiteral(selector) || ts.isNoSubstitutionTemplateLiteral(selector)) &&
          selector.text === "*"
        ) {
          reason = "clears the entire SVG";
        }
      }
    }
    if (reason) {
      const { line } = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
      violations.push(`${path}:${line + 1}: ${reason}`);
    }
  });
  return violations;
}

describe("chart package architecture", () => {
  it("keeps dependencies inside the approved package boundaries", () => {
    const path = "src/shared/charts/cartesian/Fixture.ts";
    const source = "import { value } from '../statistical/Fixture'; void value;";
    withIsolatedTypeScriptProject(
      {
        [path]: source,
        "src/shared/charts/statistical/Fixture.ts": "export const value = 1;",
      },
      (context) => {
        expect(dependencyViolations(context, [{ path, source }])).toEqual([
          `${path} imports ../statistical/Fixture`,
        ]);
      },
    );

    const violations = withProductionTypeScriptProject((context) =>
      dependencyViolations(context, productionTypeScriptSources(context)),
    );
    expect(violations, `Forbidden chart imports:\n${violations.join("\n")}`).toEqual([]);
  });

  it("keeps SVG lifecycle and resize observation in chart core", () => {
    const path = "src/shared/charts/cartesian/Fixture.ts";
    const source = [
      `const quoted = "new ResizeObserver(callback); svg.selectAll('*').remove()";`,
      "svg.selectAll('*').remove();",
      "new ResizeObserver(callback);",
    ].join("\n");
    withIsolatedTypeScriptProject({ [path]: source }, ({ sourceFile }) => {
      expect(lifecycleViolations(path, sourceFile(path))).toEqual([
        `${path}:2: clears the entire SVG`,
        `${path}:3: creates ResizeObserver outside chart core`,
      ]);
    });

    const violations = withProductionTypeScriptProject((context) =>
      productionTypeScriptSources(context)
        .filter(({ path }) => path.startsWith("src/shared/charts/"))
        .flatMap(({ path }) => lifecycleViolations(path, context.sourceFile(path))),
    );
    expect(violations, `Forbidden chart lifecycle operations:\n${violations.join("\n")}`).toEqual(
      [],
    );
  });
});
