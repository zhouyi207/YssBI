import * as ts from 'typescript/unstable/ast';
import type {
  Checker,
  Project,
  Symbol as TypeScriptSymbol,
} from 'typescript/unstable/sync';
import { productionTypeScriptSources } from './productionSourceAudit';
import type { TypeScriptAuditProject } from './typescriptAudit';

export interface SourceOccurrence {
  readonly repositoryRelativeSourceFile: string;
  readonly fullyQualifiedOwner: string;
  readonly line: number;
  readonly column: number;
}

function importDeclarationOf(
  binding: ts.ImportSpecifier | ts.NamespaceImport,
): ts.ImportDeclaration | null {
  const importClause = ts.isImportSpecifier(binding)
    ? binding.parent.parent
    : binding.parent;
  return ts.isImportClause(importClause)
    && ts.isImportDeclaration(importClause.parent)
    ? importClause.parent
    : null;
}

function isTauriCoreImport(
  binding: ts.ImportSpecifier | ts.NamespaceImport,
): boolean {
  const declaration = importDeclarationOf(binding);
  return declaration !== null
    && ts.isStringLiteral(declaration.moduleSpecifier)
    && declaration.moduleSpecifier.text === '@tauri-apps/api/core';
}

function symbolHasTauriInvokeImport(
  symbol: TypeScriptSymbol | undefined,
  project: Project,
): boolean {
  return symbol?.declarations.some((handle) => {
    const declaration = handle.resolve(project);
    return declaration !== undefined
      && ts.isImportSpecifier(declaration)
      && (declaration.propertyName ?? declaration.name).text === 'invoke'
      && isTauriCoreImport(declaration);
  }) ?? false;
}

function symbolHasTauriNamespaceImport(
  symbol: TypeScriptSymbol | undefined,
  project: Project,
): boolean {
  return symbol?.declarations.some((handle) => {
    const declaration = handle.resolve(project);
    return declaration !== undefined
      && ts.isNamespaceImport(declaration)
      && isTauriCoreImport(declaration);
  }) ?? false;
}

export function isTauriInvokeCall(
  expression: ts.Expression,
  checker: Checker,
  project: Project,
): boolean {
  if (ts.isIdentifier(expression)) {
    return symbolHasTauriInvokeImport(checker.getSymbolAtLocation(expression), project);
  }
  return ts.isPropertyAccessExpression(expression)
    && expression.name.text === 'invoke'
    && symbolHasTauriNamespaceImport(
      checker.getSymbolAtLocation(expression.expression),
      project,
    );
}

export function rawTauriInvokeOccurrences(
  context: TypeScriptAuditProject,
): readonly SourceOccurrence[] {
  return productionTypeScriptSources(context).flatMap((source) => {
    if (!source.source.includes('@tauri-apps/api/core')) return [];
    const sourceFile = context.sourceFile(source.path);
    const occurrences: SourceOccurrence[] = [];
    const visit = (node: ts.Node): void => {
      if (ts.isCallExpression(node)
        && isTauriInvokeCall(node.expression, context.checker, context.project)) {
        const position = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
        occurrences.push({
          repositoryRelativeSourceFile: source.path,
          fullyQualifiedOwner: `${source.path}::<module>`,
          line: position.line + 1,
          column: position.character + 1,
        });
      }
      node.forEachChild(visit);
    };
    visit(sourceFile);
    return occurrences;
  });
}
