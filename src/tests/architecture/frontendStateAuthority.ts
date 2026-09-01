import * as ts from "typescript/unstable/ast";

import type { ArchitectureSource } from "@/tests/helpers/moduleDependencyAudit";
import type { TypeScriptAuditProject } from "@/tests/helpers/typescriptAudit";

export type FrontendStateAuthorityClass =
  | "backend-base"
  | "optimistic-overlay"
  | "local-draft"
  | "frontend-ui";

export type FrontendWriterLayer = "Application" | "CoreUi";
export type FrontendStateMemberKind = "field" | "action";
export type FrontendStateReaderLayer = "Views" | "Application" | "Core";

export interface FrontendStateAuthorityEntry {
  readonly storeModule: string;
  readonly member: string;
  readonly memberKind: FrontendStateMemberKind;
  readonly authority: FrontendStateAuthorityClass;
  readonly writes: readonly string[];
  readonly writerModule: string | null;
  readonly writerLayer: FrontendWriterLayer | null;
  readonly readerLayers: readonly FrontendStateReaderLayer[];
}

export interface FrontendStateAuthorityMember extends FrontendStateAuthorityEntry {
  readonly sourceLayer?: FrontendStateReaderLayer;
  readonly delegatesTo?: string | null;
  readonly discovered?: boolean;
  readonly line?: number;
  readonly column?: number;
}

export interface FrontendStateAuthorityFinding {
  readonly ruleId:
    | "frontend-state-authority-missing-member"
    | "frontend-state-authority-writer"
    | "frontend-state-authority-action-writer"
    | "frontend-state-authority-action-cycle"
    | "frontend-state-authority-unresolved-delegate";
  readonly storeModule: string;
  readonly member: string;
  readonly memberKind: FrontendStateMemberKind;
  readonly canonicalWritePath: string | null;
  readonly line?: number;
  readonly column?: number;
}

const memberKey = (
  member: Pick<FrontendStateAuthorityEntry, "storeModule" | "member" | "memberKind">,
): string => `${member.storeModule}::${member.memberKind}::${member.member}`;

const canonicalPath = (path: string): string =>
  path.replace(/\[[^\]]+\]/g, ".*").replace(/\b(?:[A-Za-z_$][\w$]*|\d+)\b(?=\.dirty$)/, "*");

function propertyName(node: ts.PropertyName | undefined): string | null {
  if (!node) return null;
  return ts.isIdentifier(node) || ts.isStringLiteralLikeNode(node) ? node.text : null;
}

function typeReferenceNames(node: ts.TypeNode): readonly string[] {
  const names: string[] = [];
  const visit = (current: ts.Node): void => {
    if (ts.isTypeReferenceNode(current)) {
      const name = ts.isIdentifier(current.typeName) ? current.typeName.text : null;
      if (name) names.push(name);
    }
    current.forEachChild(visit);
  };
  visit(node);
  return names;
}

function unwrapExpression(expression: ts.Expression): ts.Expression {
  if (ts.isParenthesizedExpression(expression)) return unwrapExpression(expression.expression);
  if (ts.isAsExpression(expression)) return unwrapExpression(expression.expression);
  if (ts.isSatisfiesExpression(expression)) return unwrapExpression(expression.expression);
  return expression;
}

function objectProperty(
  object: ts.ObjectLiteralExpression | undefined,
  name: string,
): ts.ObjectLiteralElementLike | null {
  return (
    object?.properties.find(
      (property) => !ts.isSpreadAssignment(property) && propertyName(property.name) === name,
    ) ?? null
  );
}

function typeElementName(member: ts.TypeElement): string | null {
  if (!ts.isPropertySignatureDeclaration(member) && !ts.isMethodSignatureDeclaration(member))
    return null;
  return propertyName(member.name);
}

function typeElementType(member: ts.TypeElement): ts.TypeNode | undefined {
  if (!ts.isPropertySignatureDeclaration(member) && !ts.isMethodSignatureDeclaration(member))
    return undefined;
  return member.type;
}

function declaredMember(
  declaration: ts.InterfaceDeclaration | ts.TypeLiteralNode,
  name: string,
): ts.TypeElement | null {
  return declaration.members.find((member) => typeElementName(member) === name) ?? null;
}

function typeDeclarations(
  sourceFile: ts.SourceFile,
): ReadonlyMap<string, ts.InterfaceDeclaration | ts.TypeLiteralNode> {
  const declarations = new Map<string, ts.InterfaceDeclaration | ts.TypeLiteralNode>();
  for (const statement of sourceFile.statements) {
    if (ts.isInterfaceDeclaration(statement)) {
      declarations.set(statement.name.text, statement);
      continue;
    }
    if (ts.isTypeAliasDeclaration(statement) && ts.isTypeLiteralNode(statement.type)) {
      declarations.set(statement.name.text, statement.type);
    }
  }
  return declarations;
}

interface StoreSnapshot {
  readonly sourceFile: ts.SourceFile;
  readonly storeType: ts.InterfaceDeclaration | ts.TypeLiteralNode | null;
  readonly storeObject: ts.ObjectLiteralExpression | null;
}

function storeSnapshot(
  context: TypeScriptAuditProject,
  source: ArchitectureSource,
): StoreSnapshot | null {
  const sourceFile = context.sourceFile(source.path);
  const declarations = typeDeclarations(sourceFile);
  let snapshot: StoreSnapshot | null = null;

  const visit = (node: ts.Node): void => {
    if (snapshot || !ts.isVariableDeclaration(node) || !node.initializer) return;
    const initializer = unwrapExpression(node.initializer);
    if (
      !ts.isCallExpression(initializer) ||
      !ts.isIdentifier(initializer.expression) ||
      !["create", "createStore"].includes(initializer.expression.text)
    )
      return;
    const typeArgument = initializer.typeArguments?.[0];
    const typeName =
      typeArgument && ts.isTypeReferenceNode(typeArgument) && ts.isIdentifier(typeArgument.typeName)
        ? typeArgument.typeName.text
        : null;
    const factory = initializer.arguments[0] ? unwrapExpression(initializer.arguments[0]) : null;
    const factoryBody =
      factory && (ts.isArrowFunction(factory) || ts.isFunctionExpression(factory))
        ? unwrapExpression(factory.body as ts.Expression)
        : null;
    snapshot = {
      sourceFile,
      storeType: typeName ? (declarations.get(typeName) ?? null) : null,
      storeObject: factoryBody && ts.isObjectLiteralExpression(factoryBody) ? factoryBody : null,
    };
  };
  const walk = (node: ts.Node): void => {
    visit(node);
    if (!snapshot) node.forEachChild(walk);
  };
  walk(sourceFile);
  return snapshot;
}

function nestedTypeHasMember(
  declarations: ReadonlyMap<string, ts.InterfaceDeclaration | ts.TypeLiteralNode>,
  type: ts.TypeNode,
  member: string,
  visited = new Set<string>(),
): ts.TypeElement | null {
  for (const typeName of typeReferenceNames(type)) {
    if (visited.has(typeName)) continue;
    const declaration = declarations.get(typeName);
    if (!declaration) continue;
    const nextVisited = new Set([...visited, typeName]);
    const found = declaredMember(declaration, member);
    if (found) return found;
    for (const candidate of declaration.members) {
      const candidateType = typeElementType(candidate);
      if (!candidateType) continue;
      const nested = nestedTypeHasMember(declarations, candidateType, member, nextVisited);
      if (nested) return nested;
    }
  }
  return null;
}

function memberNode(snapshot: StoreSnapshot, entry: FrontendStateAuthorityEntry): ts.Node | null {
  if (!snapshot.storeType || !snapshot.storeObject) return null;
  const path = entry.member.split(".");
  const root = path[0];
  const typeMember = declaredMember(snapshot.storeType, root);
  const objectMember = objectProperty(snapshot.storeObject, root);
  if (!typeMember || !objectMember) return null;
  if (entry.memberKind === "action") return objectMember;
  if (path.length === 1) return typeMember;
  const type = typeElementType(typeMember);
  if (path.length !== 3 || path[1] !== "*" || !type) return null;
  return nestedTypeHasMember(typeDeclarations(snapshot.sourceFile), type, path[2]) ?? null;
}

function nodeLocation(
  sourceFile: ts.SourceFile,
  node: ts.Node | null,
): { line: number; column: number } {
  if (!node) return { line: 1, column: 1 };
  const position = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
  return { line: position.line + 1, column: position.character + 1 };
}

export function discoverFrontendStateAuthorityMembers(
  context: TypeScriptAuditProject,
  sources: readonly ArchitectureSource[],
  manifest: readonly FrontendStateAuthorityEntry[] = FRONTEND_STATE_AUTHORITY,
): readonly FrontendStateAuthorityMember[] {
  const sourcePaths = new Set(sources.map(({ path }) => path));
  return manifest.map((entry) => {
    if (!sourcePaths.has(entry.storeModule)) {
      return { ...entry, discovered: false, line: 1, column: 1 };
    }
    const snapshot = storeSnapshot(context, { path: entry.storeModule, source: "" });
    const node = snapshot ? memberNode(snapshot, entry) : null;
    const location = nodeLocation(
      snapshot?.sourceFile ?? context.sourceFile(entry.storeModule),
      node,
    );
    return {
      ...entry,
      sourceLayer: "Core",
      discovered: node !== null,
      line: location.line,
      column: location.column,
    };
  });
}

export const FRONTEND_STATE_AUTHORITY: readonly FrontendStateAuthorityEntry[] = [
  {
    storeModule: "src/features/core/resource/documentStateStore.ts",
    member: "documents.*.dirty",
    memberKind: "field",
    authority: "local-draft",
    writes: ["documents.*.dirty"],
    writerModule: "@/features/core/resource/ui",
    writerLayer: "CoreUi",
    readerLayers: ["Views", "Application", "Core"],
  },
  {
    storeModule: "src/features/core/chart/chartDocumentStore.ts",
    member: "updateDocument",
    memberKind: "action",
    authority: "local-draft",
    writes: ["draftsByPath.*", "dirtyByPath.*"],
    writerModule: "@/features/core/chart/ui",
    writerLayer: "CoreUi",
    readerLayers: ["Views", "Application"],
  },
];

export function auditFrontendStateAuthority(
  members: readonly FrontendStateAuthorityMember[],
  manifest: readonly FrontendStateAuthorityEntry[] = FRONTEND_STATE_AUTHORITY,
): readonly FrontendStateAuthorityFinding[] {
  const entries = new Map(manifest.map((entry) => [memberKey(entry), entry]));
  const findings: FrontendStateAuthorityFinding[] = [];
  const actionMembers = new Map(
    members
      .filter((member) => member.memberKind === "action")
      .map((member) => [memberKey(member), member]),
  );

  for (const member of members) {
    const entry = entries.get(memberKey(member));
    if (!entry || member.discovered === false) {
      findings.push({
        ruleId: "frontend-state-authority-missing-member",
        storeModule: member.storeModule,
        member: member.member,
        memberKind: member.memberKind,
        canonicalWritePath: member.writes[0] ? canonicalPath(member.writes[0]) : null,
        line: member.line,
        column: member.column,
      });
      continue;
    }

    if (member.sourceLayer === "Views" && entry.writerLayer !== "CoreUi") {
      findings.push({
        ruleId: "frontend-state-authority-writer",
        storeModule: member.storeModule,
        member: member.member,
        memberKind: member.memberKind,
        canonicalWritePath: entry.writes[0] ? canonicalPath(entry.writes[0]) : null,
        line: member.line,
        column: member.column,
      });
    }
    if (member.memberKind === "action" && member.writerLayer !== entry.writerLayer) {
      findings.push({
        ruleId: "frontend-state-authority-action-writer",
        storeModule: member.storeModule,
        member: member.member,
        memberKind: member.memberKind,
        canonicalWritePath: entry.writes[0] ? canonicalPath(entry.writes[0]) : null,
        line: member.line,
        column: member.column,
      });
    }
  }

  const discoveredKeys = new Set(members.map(memberKey));
  for (const entry of manifest) {
    if (discoveredKeys.has(memberKey(entry))) continue;
    findings.push({
      ruleId: "frontend-state-authority-missing-member",
      storeModule: entry.storeModule,
      member: entry.member,
      memberKind: entry.memberKind,
      canonicalWritePath: entry.writes[0] ? canonicalPath(entry.writes[0]) : null,
      line: 1,
      column: 1,
    });
  }

  for (const member of actionMembers.values()) {
    const delegate = member.delegatesTo;
    if (!delegate) continue;
    if (!actionMembers.has(delegate)) {
      findings.push({
        ruleId: "frontend-state-authority-unresolved-delegate",
        storeModule: member.storeModule,
        member: member.member,
        memberKind: member.memberKind,
        canonicalWritePath: member.writes[0] ? canonicalPath(member.writes[0]) : null,
        line: member.line,
        column: member.column,
      });
      continue;
    }
    const visited = new Set<string>();
    let current: string | undefined = memberKey(member);
    let hops = 0;
    while (current) {
      if (++hops > actionMembers.size) {
        findings.push({
          ruleId: "frontend-state-authority-action-cycle",
          storeModule: member.storeModule,
          member: member.member,
          memberKind: member.memberKind,
          canonicalWritePath: member.writes[0] ? canonicalPath(member.writes[0]) : null,
          line: member.line,
          column: member.column,
        });
        break;
      }
      if (!visited.add(current)) {
        findings.push({
          ruleId: "frontend-state-authority-action-cycle",
          storeModule: member.storeModule,
          member: member.member,
          memberKind: member.memberKind,
          canonicalWritePath: member.writes[0] ? canonicalPath(member.writes[0]) : null,
          line: member.line,
          column: member.column,
        });
        break;
      }
      const next: string | null | undefined = actionMembers.get(current)?.delegatesTo;
      current = next ?? undefined;
    }
  }

  return findings;
}
