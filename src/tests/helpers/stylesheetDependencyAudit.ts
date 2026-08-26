import { existsSync } from 'node:fs';
import { posix, resolve } from 'node:path';
import {
  parseExternalDependencySpecifier,
  type ResolvedModuleDependency,
  type StylesheetDependencyOrigin,
} from './moduleDependencyAudit';

export type StylesheetDependencyKind =
  | 'stylesheet-import'
  | 'stylesheet-url';

export interface ResolvedStylesheetDependency {
  repositoryRelativeSourceFile: string;
  fullyQualifiedOwner: string;
  kind: StylesheetDependencyKind;
  mode: 'build-style';
  origin: StylesheetDependencyOrigin;
  canonicalOriginTarget: string;
  writtenSpecifier: string;
  line: number;
  column: number;
}

export type StylesheetResolutionError =
  | { readonly kind: 'stylesheet-parse-failure'; readonly sourceFile: string; readonly line: number; readonly column: number }
  | { readonly kind: 'stylesheet-cycle'; readonly cycle: readonly string[] }
  | { readonly kind: 'stylesheet-path-escapes-repository'; readonly sourceFile: string; readonly writtenSpecifier: string }
  | { readonly kind: 'stylesheet-target-missing'; readonly sourceFile: string; readonly canonicalTarget: string }
  | { readonly kind: 'unsupported-stylesheet-target'; readonly sourceFile: string; readonly writtenSpecifier: string };

export interface ResolvedStylesheetGraph {
  readonly repositoryStylesheets: readonly string[];
  readonly dependencies: readonly ResolvedStylesheetDependency[];
  readonly errors: readonly StylesheetResolutionError[];
}

export interface RepositoryTextReader {
  readRepositoryText(repositoryRelativePath: string): string | null;
}

interface StylesheetReference {
  readonly kind: StylesheetDependencyKind;
  readonly specifier: string;
  readonly offset: number;
}

interface StylesheetLexResult {
  readonly references: readonly StylesheetReference[];
  readonly errorOffsets: readonly number[];
}

interface CursorResult {
  readonly next: number;
  readonly errorOffset: number | null;
}

interface LiteralResult extends CursorResult {
  readonly value: string | null;
}

function isWhitespace(value: string | undefined): boolean {
  return value === ' ' || value === '\t' || value === '\r' || value === '\n' || value === '\f';
}

function isIdentifierCharacter(value: string | undefined): boolean {
  if (!value) return false;
  const code = value.charCodeAt(0);
  return (code >= 48 && code <= 57)
    || (code >= 65 && code <= 90)
    || (code >= 97 && code <= 122)
    || value === '-'
    || value === '_';
}

function startsWord(source: string, offset: number, word: string): boolean {
  if (source.slice(offset, offset + word.length).toLowerCase() !== word) return false;
  return !isIdentifierCharacter(source[offset - 1])
    && !isIdentifierCharacter(source[offset + word.length]);
}

function skipComment(source: string, offset: number): CursorResult {
  const end = source.indexOf('*/', offset + 2);
  return end < 0
    ? { next: source.length, errorOffset: offset }
    : { next: end + 2, errorOffset: null };
}

function skipTrivia(source: string, offset: number): CursorResult {
  let cursor = offset;
  while (cursor < source.length) {
    if (isWhitespace(source[cursor])) {
      cursor += 1;
      continue;
    }
    if (source[cursor] === '/' && source[cursor + 1] === '*') {
      const comment = skipComment(source, cursor);
      if (comment.errorOffset !== null) return comment;
      cursor = comment.next;
      continue;
    }
    break;
  }
  return { next: cursor, errorOffset: null };
}

function readQuotedLiteral(source: string, offset: number): LiteralResult {
  const quote = source[offset];
  if (quote !== '"' && quote !== "'") {
    return { value: null, next: offset, errorOffset: offset };
  }
  let value = '';
  let cursor = offset + 1;
  while (cursor < source.length) {
    const character = source[cursor];
    if (character === quote) {
      return { value, next: cursor + 1, errorOffset: null };
    }
    if (character === '\n' || character === '\r') {
      return { value: null, next: cursor, errorOffset: cursor };
    }
    if (character === '\\') {
      if (cursor + 1 >= source.length) {
        return { value: null, next: source.length, errorOffset: cursor };
      }
      value += source[cursor + 1];
      cursor += 2;
      continue;
    }
    value += character;
    cursor += 1;
  }
  return { value: null, next: source.length, errorOffset: offset };
}

function readUrlLiteral(source: string, offset: number): LiteralResult {
  const openParenthesis = offset + 3;
  if (source[openParenthesis] !== '(') {
    return { value: null, next: offset + 3, errorOffset: offset };
  }
  const trivia = skipTrivia(source, openParenthesis + 1);
  if (trivia.errorOffset !== null) {
    return { value: null, next: trivia.next, errorOffset: trivia.errorOffset };
  }
  let cursor = trivia.next;
  let value: string | null;
  if (source[cursor] === '"' || source[cursor] === "'") {
    const quoted = readQuotedLiteral(source, cursor);
    if (quoted.errorOffset !== null) return quoted;
    value = quoted.value;
    cursor = quoted.next;
  } else {
    const start = cursor;
    while (cursor < source.length && source[cursor] !== ')') {
      if (source[cursor] === '(' || source[cursor] === '"' || source[cursor] === "'") {
        return { value: null, next: cursor, errorOffset: cursor };
      }
      cursor += 1;
    }
    value = source.slice(start, cursor).trim();
    if (!value || [...value].some(isWhitespace)) {
      return { value: null, next: cursor, errorOffset: start };
    }
  }
  const closingTrivia = skipTrivia(source, cursor);
  if (closingTrivia.errorOffset !== null || source[closingTrivia.next] !== ')') {
    return {
      value: null,
      next: closingTrivia.next,
      errorOffset: closingTrivia.errorOffset ?? closingTrivia.next,
    };
  }
  return { value, next: closingTrivia.next + 1, errorOffset: null };
}

function lexStylesheet(source: string): StylesheetLexResult {
  const references: StylesheetReference[] = [];
  const errorOffsets: number[] = [];
  let cursor = 0;
  while (cursor < source.length) {
    if (source[cursor] === '/' && source[cursor + 1] === '*') {
      const comment = skipComment(source, cursor);
      if (comment.errorOffset !== null) {
        errorOffsets.push(comment.errorOffset);
        break;
      }
      cursor = comment.next;
      continue;
    }
    if (source[cursor] === '"' || source[cursor] === "'") {
      const quoted = readQuotedLiteral(source, cursor);
      if (quoted.errorOffset !== null) {
        errorOffsets.push(quoted.errorOffset);
        break;
      }
      cursor = quoted.next;
      continue;
    }
    if (source[cursor] === '@' && startsWord(source, cursor + 1, 'import')) {
      const valueStart = skipTrivia(source, cursor + 7);
      if (valueStart.errorOffset !== null) {
        errorOffsets.push(valueStart.errorOffset);
        break;
      }
      const literal = source[valueStart.next] === '"' || source[valueStart.next] === "'"
        ? readQuotedLiteral(source, valueStart.next)
        : startsWord(source, valueStart.next, 'url')
          ? readUrlLiteral(source, valueStart.next)
          : { value: null, next: valueStart.next, errorOffset: valueStart.next };
      if (literal.errorOffset !== null || literal.value === null) {
        errorOffsets.push(literal.errorOffset ?? valueStart.next);
        cursor = Math.max(literal.next, valueStart.next + 1);
        continue;
      }
      references.push({ kind: 'stylesheet-import', specifier: literal.value, offset: valueStart.next });
      cursor = literal.next;
      continue;
    }
    if (startsWord(source, cursor, 'url') && source[cursor + 3] === '(') {
      const literal = readUrlLiteral(source, cursor);
      if (literal.errorOffset !== null || literal.value === null) {
        errorOffsets.push(literal.errorOffset ?? cursor);
        cursor = Math.max(literal.next, cursor + 1);
        continue;
      }
      references.push({ kind: 'stylesheet-url', specifier: literal.value, offset: cursor });
      cursor = literal.next;
      continue;
    }
    cursor += 1;
  }
  return { references, errorOffsets };
}

function lineAndColumn(source: string, offset: number): { line: number; column: number } {
  let line = 1;
  let column = 1;
  for (let cursor = 0; cursor < offset; cursor += 1) {
    if (source[cursor] === '\n') {
      line += 1;
      column = 1;
    } else {
      column += 1;
    }
  }
  return { line, column };
}

function hasForbiddenEncodedSeparator(specifier: string): boolean {
  const lower = specifier.toLowerCase();
  return lower.includes('%2f') || lower.includes('%5c');
}

function externalTarget(origin: Extract<StylesheetDependencyOrigin, { kind: 'external' }>): string {
  const subpath = origin.dependency.canonicalSubpath;
  return `external:${origin.dependency.packageName}${subpath === null ? '' : `::${subpath}`}`;
}

function packageExists(repositoryRoot: string, packageName: string): boolean {
  return existsSync(resolve(repositoryRoot, 'node_modules', ...packageName.split('/')));
}

type StylesheetOriginResult =
  | { readonly origin: StylesheetDependencyOrigin; readonly canonicalTarget: string }
  | { readonly error: StylesheetResolutionError };

function resolveStylesheetOrigin(
  repositoryRoot: string,
  sourceFile: string,
  specifier: string,
  sourceReader: RepositoryTextReader,
): StylesheetOriginResult {
  if (!specifier
    || specifier.includes('\\')
    || specifier.includes('?')
    || specifier.includes('#')
    || hasForbiddenEncodedSeparator(specifier)
    || specifier.startsWith('//')
    || specifier.includes('://')
    || specifier.toLowerCase().startsWith('data:')) {
    return { error: { kind: 'unsupported-stylesheet-target', sourceFile, writtenSpecifier: specifier } };
  }
  if (specifier.startsWith('/') || specifier.startsWith('../')) {
    const escapedTarget = posix.normalize(posix.join(posix.dirname(sourceFile), specifier));
    if (!escapedTarget.startsWith('src/')) {
      return { error: { kind: 'stylesheet-path-escapes-repository', sourceFile, writtenSpecifier: specifier } };
    }
    return { error: { kind: 'unsupported-stylesheet-target', sourceFile, writtenSpecifier: specifier } };
  }
  if (specifier.startsWith('./')) {
    const canonicalTarget = posix.normalize(posix.join(posix.dirname(sourceFile), specifier));
    if (!canonicalTarget.startsWith('src/')) {
      return { error: { kind: 'stylesheet-path-escapes-repository', sourceFile, writtenSpecifier: specifier } };
    }
    if (!canonicalTarget.endsWith('.css')) {
      return { error: { kind: 'unsupported-stylesheet-target', sourceFile, writtenSpecifier: specifier } };
    }
    if (sourceReader.readRepositoryText(canonicalTarget) === null) {
      return { error: { kind: 'stylesheet-target-missing', sourceFile, canonicalTarget } };
    }
    return {
      origin: {
        kind: 'repository-asset',
        asset: { repositoryRelativeAssetPath: canonicalTarget, resourceKind: 'stylesheet' },
      },
      canonicalTarget: `repository-asset:${canonicalTarget}`,
    };
  }
  const dependency = parseExternalDependencySpecifier(specifier, 'stylesheet');
  if (!dependency) {
    return { error: { kind: 'unsupported-stylesheet-target', sourceFile, writtenSpecifier: specifier } };
  }
  const origin = { kind: 'external', dependency } as const;
  const canonicalTarget = externalTarget(origin);
  if (!packageExists(repositoryRoot, dependency.packageName)) {
    return { error: { kind: 'stylesheet-target-missing', sourceFile, canonicalTarget } };
  }
  return { origin, canonicalTarget };
}

export function resolvedStylesheetDependencies(
  repositoryRoot: string,
  moduleDependencies: readonly ResolvedModuleDependency[],
  sourceReader: RepositoryTextReader,
): ResolvedStylesheetGraph {
  const repositoryStylesheets = new Set<string>();
  const dependencies: ResolvedStylesheetDependency[] = [];
  const errors: StylesheetResolutionError[] = [];
  const visited = new Set<string>();
  const traversalStack: string[] = [];
  const roots = moduleDependencies.flatMap(({ origin }) => (
    origin.kind === 'repository-asset' ? [origin.asset.repositoryRelativeAssetPath] : []
  ));

  const visit = (sourceFile: string): void => {
    const cycleStart = traversalStack.indexOf(sourceFile);
    if (cycleStart >= 0) {
      errors.push({
        kind: 'stylesheet-cycle',
        cycle: [...traversalStack.slice(cycleStart), sourceFile],
      });
      return;
    }
    if (visited.has(sourceFile)) return;
    const source = sourceReader.readRepositoryText(sourceFile);
    if (source === null) {
      errors.push({ kind: 'stylesheet-target-missing', sourceFile, canonicalTarget: sourceFile });
      return;
    }
    visited.add(sourceFile);
    repositoryStylesheets.add(sourceFile);
    traversalStack.push(sourceFile);
    const lexed = lexStylesheet(source);
    for (const offset of lexed.errorOffsets) {
      errors.push({ kind: 'stylesheet-parse-failure', sourceFile, ...lineAndColumn(source, offset) });
    }
    for (const reference of lexed.references) {
      const resolvedOrigin = resolveStylesheetOrigin(
        repositoryRoot,
        sourceFile,
        reference.specifier,
        sourceReader,
      );
      if ('error' in resolvedOrigin) {
        errors.push(resolvedOrigin.error);
        continue;
      }
      dependencies.push({
        repositoryRelativeSourceFile: sourceFile,
        fullyQualifiedOwner: `stylesheet:${sourceFile}`,
        kind: reference.kind,
        mode: 'build-style',
        origin: resolvedOrigin.origin,
        canonicalOriginTarget: resolvedOrigin.canonicalTarget,
        writtenSpecifier: reference.specifier,
        ...lineAndColumn(source, reference.offset),
      });
      if (resolvedOrigin.origin.kind === 'repository-asset') {
        visit(resolvedOrigin.origin.asset.repositoryRelativeAssetPath);
      }
    }
    traversalStack.pop();
  };

  [...new Set(roots)].sort().forEach(visit);
  return {
    repositoryStylesheets: [...repositoryStylesheets].sort(),
    dependencies,
    errors,
  };
}
