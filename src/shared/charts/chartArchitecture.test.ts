import { readdirSync, readFileSync } from 'node:fs';
import { posix, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

const repositoryRoot = fileURLToPath(new URL('../../../', import.meta.url));
const sourceRoot = resolve(repositoryRoot, 'src');

const forbiddenRules = [
  { from: /^src\/shared\/charts\//, to: /^@\/(?:views|services|features)\// },
  { from: /^src\/shared\/charts\/cartesian\//, to: /^@\/shared\/charts\/statistical(?:\/|$)/ },
  { from: /^src\/shared\/charts\/statistical\//, to: /^@\/shared\/charts\/cartesian(?:\/|$)/ },
  { from: /^src\/shared\/charts\/(?:core|cartesian|statistical)\//, to: /^@\/shared\/charts$/ },
  { from: /^src\/views\/(?!PlotView\/PlotWindow\.tsx)/, to: /^@\/views\/PlotView\// },
];

type SourceKind = 'ts' | 'tsx';

interface ProductionSource {
  path: string;
  source: string;
  sourceKind: SourceKind;
}

function toRepositoryPath(filePath: string): string {
  return relative(repositoryRoot, filePath).split(sep).join('/');
}

function collectProductionFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))
    .flatMap(entry => {
      if (entry.name === '.superpowers' || entry.name === '__tests__') return [];

      const entryPath = resolve(directory, entry.name);
      if (entry.isDirectory()) return collectProductionFiles(entryPath);
      if (!entry.isFile() || !/\.tsx?$/.test(entry.name)) return [];
      if (/\.(?:test|spec)\.tsx?$/.test(entry.name)) return [];
      return [entryPath];
    });
}

function parseProductionSource(filePath: string): ProductionSource {
  return {
    path: toRepositoryPath(filePath),
    source: readFileSync(filePath, 'utf8'),
    sourceKind: filePath.endsWith('.tsx') ? 'tsx' : 'ts',
  };
}

type SourceTokenKind = 'identifier' | 'number' | 'punctuator' | 'string';
type LifecycleOperationKind = 'resize-observer' | 'whole-svg-clear';

interface SourceToken {
  kind: SourceTokenKind;
  value: string;
  index: number;
}

interface LifecycleOperation {
  kind: LifecycleOperationKind;
  index: number;
}

const CONTROL_CONDITION_KEYWORDS = new Set(['catch', 'for', 'if', 'switch', 'while', 'with']);
const REGEX_PREFIX_KEYWORDS = new Set([
  'await',
  'case',
  'delete',
  'do',
  'else',
  'in',
  'instanceof',
  'new',
  'of',
  'return',
  'throw',
  'typeof',
  'void',
  'yield',
]);
const REGEX_PREFIX_PUNCTUATORS = new Set([
  '!',
  '!=',
  '!==',
  '%',
  '%=',
  '&',
  '&&',
  '&&=',
  '(',
  '*',
  '**',
  '**=',
  '*=',
  '+',
  '+=',
  ',',
  '-',
  '-=',
  '/',
  '/=',
  ':',
  ';',
  '=',
  '==',
  '===',
  '=>',
  '?',
  '??',
  '??=',
  '[',
  '^',
  '^=',
  '{',
  '|',
  '||',
  '||=',
  '~',
]);
const PUNCTUATORS = [
  '===',
  '!==',
  '>>>',
  '**=',
  '&&=',
  '||=',
  '??=',
  '...',
  '=>',
  '==',
  '!=',
  '<=',
  '>=',
  '&&',
  '||',
  '??',
  '++',
  '--',
  '+=',
  '-=',
  '*=',
  '/=',
  '%=',
  '^=',
  '&=',
  '|=',
  '**',
  '<<',
  '>>',
  '?.',
];

function tokenizeSource(source: string, sourceKind: SourceKind): SourceToken[] {
  const tokens: SourceToken[] = [];
  let index = 0;

  function readString(): SourceToken {
    const start = index;
    const quote = source[index];
    let value = '';
    index += 1;
    while (index < source.length) {
      const character = source[index];
      if (character === '\\' && index + 1 < source.length) {
        value += source[index + 1];
        index += 2;
      } else if (character === quote) {
        index += 1;
        break;
      } else {
        value += character;
        index += 1;
      }
    }
    return { kind: 'string', value, index: start };
  }

  function skipRegexLiteral(): void {
    index += 1;
    let inCharacterClass = false;
    while (index < source.length) {
      const character = source[index];
      if (character === '\\') {
        index += 2;
      } else if (character === '[') {
        inCharacterClass = true;
        index += 1;
      } else if (character === ']' && inCharacterClass) {
        inCharacterClass = false;
        index += 1;
      } else if (character === '/' && !inCharacterClass) {
        index += 1;
        while (/[A-Za-z]/.test(source[index] ?? '')) index += 1;
        return;
      } else if (character === '\n' || character === '\r') {
        return;
      } else {
        index += 1;
      }
    }
  }

  function scanTemplate(): void {
    index += 1;
    while (index < source.length) {
      if (source[index] === '\\') {
        index += 2;
      } else if (source[index] === '`') {
        index += 1;
        return;
      } else if (source[index] === '$' && source[index + 1] === '{') {
        index += 2;
        scanCode(true);
      } else {
        index += 1;
      }
    }
  }

  function skipLookaheadTrivia(start: number): number {
    let cursor = start;
    while (cursor < source.length) {
      if (/\s/.test(source[cursor])) {
        cursor += 1;
      } else if (source[cursor] === '/' && source[cursor + 1] === '/') {
        cursor += 2;
        while (cursor < source.length && source[cursor] !== '\n' && source[cursor] !== '\r') cursor += 1;
      } else if (source[cursor] === '/' && source[cursor + 1] === '*') {
        cursor += 2;
        while (cursor < source.length && !(source[cursor] === '*' && source[cursor + 1] === '/')) cursor += 1;
        cursor = Math.min(source.length, cursor + 2);
      } else {
        return cursor;
      }
    }
    return cursor;
  }

  function skipLookaheadQuoted(start: number): number {
    const quote = source[start];
    let cursor = start + 1;
    while (cursor < source.length) {
      if (source[cursor] === '\\') cursor += 2;
      else if (source[cursor] === quote) return cursor + 1;
      else cursor += 1;
    }
    return cursor;
  }

  function canStartFunctionTypeParameters(start: number, end: number): boolean {
    const cursor = skipLookaheadTrivia(start);
    if (cursor >= end) return true;
    if (source.startsWith('...', cursor) || source[cursor] === '{' || source[cursor] === '[') return true;
    if (!/[A-Za-z_$]/.test(source[cursor] ?? '')) return false;

    let wordEnd = cursor + 1;
    while (/[A-Za-z0-9_$]/.test(source[wordEnd] ?? '')) wordEnd += 1;
    const firstWord = source.slice(cursor, wordEnd);
    return firstWord !== 'abstract' && firstWord !== 'new';
  }

  function hasArrowAfterReturnType(start: number): boolean {
    let cursor = skipLookaheadTrivia(start);
    if (source.startsWith('=>', cursor)) return true;
    if (source[cursor] !== ':') return false;

    cursor = skipLookaheadTrivia(cursor + 1);
    const delimiters: Array<{ closing: string; parameterStart?: number }> = [];
    let hasTypeContent = false;
    let closedFunctionParameters = false;

    while (cursor < source.length) {
      const afterTrivia = skipLookaheadTrivia(cursor);
      if (afterTrivia !== cursor) {
        cursor = afterTrivia;
        continue;
      }
      const character = source[cursor];
      if (character === "'" || character === '"' || character === '`') {
        cursor = skipLookaheadQuoted(cursor);
        hasTypeContent = true;
        closedFunctionParameters = false;
        continue;
      }
      if (source.startsWith('=>', cursor)) {
        if (delimiters.length > 0) {
          cursor += 2;
          continue;
        }
        if (!hasTypeContent) return false;
        if (!closedFunctionParameters) return true;

        cursor += 2;
        hasTypeContent = false;
        closedFunctionParameters = false;
        continue;
      }

      const closing =
        character === '(' ? ')' : character === '[' ? ']' : character === '{' ? '}' : character === '<' ? '>' : undefined;
      if (closing) {
        delimiters.push({
          closing,
          parameterStart: delimiters.length === 0 && character === '(' ? cursor + 1 : undefined,
        });
        cursor += 1;
        hasTypeContent = true;
        closedFunctionParameters = false;
        continue;
      }
      if (character === ')' || character === ']' || character === '}' || character === '>') {
        const delimiter = delimiters.pop();
        if (delimiter?.closing !== character) return false;

        cursor += 1;
        hasTypeContent = true;
        closedFunctionParameters =
          delimiters.length === 0 &&
          character === ')' &&
          delimiter.parameterStart !== undefined &&
          canStartFunctionTypeParameters(delimiter.parameterStart, cursor - 1);
        continue;
      }
      if (delimiters.length === 0 && (character === ';' || character === ',' || character === '=')) return false;

      cursor += 1;
      hasTypeContent = true;
      closedFunctionParameters = false;
    }
    return false;
  }

  function startsGenericSignature(tokenStart: number): boolean {
    let angleDepth = 1;
    let cursor = index + 1;
    let hasGenericDisambiguator = false;

    while (cursor < source.length && angleDepth > 0) {
      const afterTrivia = skipLookaheadTrivia(cursor);
      if (afterTrivia !== cursor) {
        cursor = afterTrivia;
        continue;
      }
      if (source[cursor] === "'" || source[cursor] === '"' || source[cursor] === '`') {
        cursor = skipLookaheadQuoted(cursor);
        continue;
      }
      if (source.startsWith('=>', cursor)) {
        cursor += 2;
        continue;
      }
      if (source[cursor] === '<') {
        angleDepth += 1;
      } else if (source[cursor] === '>') {
        angleDepth -= 1;
        if (angleDepth === 0) break;
      } else if (angleDepth === 1 && source[cursor] === ',') {
        hasGenericDisambiguator = true;
      } else if (/[A-Za-z_$]/.test(source[cursor])) {
        const wordStart = cursor;
        cursor += 1;
        while (/[A-Za-z0-9_$]/.test(source[cursor] ?? '')) cursor += 1;
        if (angleDepth === 1 && source.slice(wordStart, cursor) === 'extends') {
          hasGenericDisambiguator = true;
        }
        continue;
      }
      cursor += 1;
    }
    if (angleDepth !== 0) return false;

    const previousIndex = tokens.length - 1;
    const inTypeAlias =
      previousIndex - 2 >= tokenStart &&
      tokens[previousIndex].value === '=' &&
      tokens[previousIndex - 1].kind === 'identifier' &&
      tokens[previousIndex - 2].value === 'type';
    if (!hasGenericDisambiguator && !inTypeAlias) return false;

    cursor = skipLookaheadTrivia(cursor + 1);
    if (source[cursor] !== '(') return false;

    let parenthesisDepth = 0;
    while (cursor < source.length) {
      const afterTrivia = skipLookaheadTrivia(cursor);
      if (afterTrivia !== cursor) {
        cursor = afterTrivia;
        continue;
      }
      if (source[cursor] === "'" || source[cursor] === '"' || source[cursor] === '`') {
        cursor = skipLookaheadQuoted(cursor);
        continue;
      }
      if (source[cursor] === '(') parenthesisDepth += 1;
      if (source[cursor] === ')') {
        parenthesisDepth -= 1;
        cursor += 1;
        if (parenthesisDepth === 0) break;
        continue;
      }
      cursor += 1;
    }
    if (parenthesisDepth !== 0) return false;

    return hasArrowAfterReturnType(cursor);
  }

  function startsJsxElement(canStartExpression: boolean, tokenStart: number): boolean {
    if (sourceKind !== 'tsx' || !canStartExpression || source[index] !== '<') return false;
    if (startsGenericSignature(tokenStart)) return false;
    if (source[index + 1] === '>') return true;

    let cursor = index + 1;
    if (!/[A-Za-z_$]/.test(source[cursor] ?? '')) return false;
    cursor += 1;
    while (/[A-Za-z0-9_$:.-]/.test(source[cursor] ?? '')) cursor += 1;
    return /[\s/>]/.test(source[cursor] ?? '');
  }

  function scanJsxElement(): void {
    let elementDepth = 0;
    while (index < source.length) {
      if (source[index] === '{') {
        index += 1;
        scanCode(true);
        continue;
      }
      if (source[index] !== '<') {
        index += 1;
        continue;
      }

      const closingTag = source[index + 1] === '/';
      let selfClosingTag = false;
      index += closingTag ? 2 : 1;
      while (index < source.length) {
        if (source[index] === "'" || source[index] === '"') {
          readString();
        } else if (source[index] === '{') {
          index += 1;
          scanCode(true);
        } else if (source[index] === '/' && source[index + 1] === '>') {
          selfClosingTag = true;
          index += 2;
          break;
        } else if (source[index] === '>') {
          index += 1;
          break;
        } else {
          index += 1;
        }
      }

      if (closingTag) elementDepth -= 1;
      else if (!selfClosingTag) elementDepth += 1;
      if (elementDepth === 0) return;
    }
  }

  function scanCode(stopAtExpressionEnd = false): void {
    const tokenStart = tokens.length;
    const controlConditionStack: boolean[] = [];
    let braceDepth = 0;
    let canStartRegex = true;

    const startsControlCondition = (): boolean => {
      const previousIndex = tokens.length - 1;
      if (previousIndex < tokenStart) return false;

      const previous = tokens[previousIndex];
      const beforePrevious = previousIndex > tokenStart ? tokens[previousIndex - 1] : undefined;
      if (
        previous.kind === 'identifier' &&
        CONTROL_CONDITION_KEYWORDS.has(previous.value) &&
        beforePrevious?.value !== '.' &&
        beforePrevious?.value !== '?.'
      ) {
        return true;
      }
      return previous.value === 'await' && beforePrevious?.value === 'for';
    };

    const emit = (kind: SourceTokenKind, value: string, start: number): void => {
      let closesControlCondition = false;
      if (kind === 'punctuator' && value === '(') {
        controlConditionStack.push(startsControlCondition());
      } else if (kind === 'punctuator' && value === ')') {
        closesControlCondition = controlConditionStack.pop() ?? false;
      }

      const token = { kind, value, index: start };
      tokens.push(token);
      canStartRegex = closesControlCondition || (kind === 'punctuator'
        ? REGEX_PREFIX_PUNCTUATORS.has(value)
        : kind === 'identifier' && REGEX_PREFIX_KEYWORDS.has(value));
    };

    while (index < source.length) {
      const character = source[index];
      const next = source[index + 1];
      if (/\s/.test(character)) {
        index += 1;
        continue;
      }
      if (character === '/' && next === '/') {
        index += 2;
        while (index < source.length && source[index] !== '\n' && source[index] !== '\r') index += 1;
        continue;
      }
      if (character === '/' && next === '*') {
        index += 2;
        while (index < source.length && !(source[index] === '*' && source[index + 1] === '/')) index += 1;
        index = Math.min(source.length, index + 2);
        continue;
      }
      if (stopAtExpressionEnd && character === '}' && braceDepth === 0) {
        index += 1;
        return;
      }
      if (character === '<' && startsJsxElement(canStartRegex, tokenStart)) {
        scanJsxElement();
        canStartRegex = false;
        continue;
      }
      if (character === "'" || character === '"') {
        const token = readString();
        emit(token.kind, token.value, token.index);
        continue;
      }
      if (character === '`') {
        scanTemplate();
        canStartRegex = false;
        continue;
      }
      if (character === '/' && canStartRegex) {
        skipRegexLiteral();
        canStartRegex = false;
        continue;
      }
      if (/[A-Za-z_$]/.test(character)) {
        const start = index;
        index += 1;
        while (/[A-Za-z0-9_$]/.test(source[index] ?? '')) index += 1;
        emit('identifier', source.slice(start, index), start);
        continue;
      }
      if (/[0-9]/.test(character)) {
        const start = index;
        index += 1;
        while (/[A-Za-z0-9_.]/.test(source[index] ?? '')) index += 1;
        emit('number', source.slice(start, index), start);
        continue;
      }
      if (character === '{') braceDepth += 1;
      if (character === '}' && braceDepth > 0) braceDepth -= 1;
      const punctuator = PUNCTUATORS.find(candidate => source.startsWith(candidate, index)) ?? character;
      emit('punctuator', punctuator, index);
      index += punctuator.length;
    }
  }

  if (source.startsWith('#!')) {
    while (index < source.length && source[index] !== '\n') index += 1;
  }
  scanCode();
  return tokens;
}

function tokenIs(
  tokens: readonly SourceToken[],
  index: number,
  kind: SourceTokenKind,
  value?: string,
): boolean {
  const token = tokens[index];
  return token?.kind === kind && (value === undefined || token.value === value);
}

function findFromSpecifier(tokens: readonly SourceToken[], start: number): SourceToken | undefined {
  for (let index = start; index < tokens.length && !tokenIs(tokens, index, 'punctuator', ';'); index += 1) {
    if (tokenIs(tokens, index, 'identifier', 'from') && tokenIs(tokens, index + 1, 'string')) {
      return tokens[index + 1];
    }
  }
  return undefined;
}

function collectModuleSpecifiers(source: string, sourceKind: SourceKind): string[] {
  const tokens = tokenizeSource(source, sourceKind);
  const specifiers: SourceToken[] = [];

  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (token.kind === 'identifier' && token.value === 'import') {
      if (tokenIs(tokens, index - 1, 'punctuator', '.') || tokenIs(tokens, index - 1, 'punctuator', '?.')) continue;
      if (tokenIs(tokens, index + 1, 'punctuator', '(')) {
        if (tokenIs(tokens, index + 2, 'string')) specifiers.push(tokens[index + 2]);
        continue;
      }
      if (tokenIs(tokens, index + 1, 'string')) {
        specifiers.push(tokens[index + 1]);
        continue;
      }

      let cursor = index + 1;
      if (tokenIs(tokens, cursor, 'identifier', 'type')) cursor += 1;
      if (
        tokenIs(tokens, cursor, 'identifier') &&
        tokenIs(tokens, cursor + 1, 'punctuator', '=') &&
        tokenIs(tokens, cursor + 2, 'identifier', 'require') &&
        tokenIs(tokens, cursor + 3, 'punctuator', '(') &&
        tokenIs(tokens, cursor + 4, 'string')
      ) {
        specifiers.push(tokens[cursor + 4]);
        continue;
      }
      if (
        tokenIs(tokens, cursor, 'identifier') ||
        tokenIs(tokens, cursor, 'punctuator', '{') ||
        tokenIs(tokens, cursor, 'punctuator', '*')
      ) {
        const specifier = findFromSpecifier(tokens, cursor + 1);
        if (specifier) specifiers.push(specifier);
      }
    } else if (token.kind === 'identifier' && token.value === 'export') {
      let cursor = index + 1;
      if (tokenIs(tokens, cursor, 'identifier', 'type')) cursor += 1;
      if (!tokenIs(tokens, cursor, 'punctuator', '{') && !tokenIs(tokens, cursor, 'punctuator', '*')) continue;
      const specifier = findFromSpecifier(tokens, cursor + 1);
      if (specifier) specifiers.push(specifier);
    }
  }

  return specifiers.sort((left, right) => left.index - right.index).map(token => token.value);
}

function canonicalizeModuleSpecifier(sourcePath: string, specifier: string): string {
  if (!specifier.startsWith('.')) return specifier;

  const targetPath = posix.normalize(posix.join(posix.dirname(sourcePath), specifier));
  if (targetPath === 'src') return '@';
  return targetPath.startsWith('src/')
    ? `@/${targetPath.slice('src/'.length)}`
    : targetPath;
}

function collectForbiddenDependencyViolations(
  sources: readonly ProductionSource[],
): string[] {
  return sources.flatMap(source =>
    collectModuleSpecifiers(source.source, source.sourceKind).flatMap(specifier => {
      const canonicalSpecifier = canonicalizeModuleSpecifier(source.path, specifier);
      const diagnostic = canonicalSpecifier === specifier
        ? specifier
        : `${specifier} (resolved as ${canonicalSpecifier})`;
      return forbiddenRules
        .filter(rule => rule.from.test(source.path) && rule.to.test(canonicalSpecifier))
        .map(() => `${source.path} imports ${diagnostic}`);
    }),
  );
}

function collectLifecycleOperations(source: string, sourceKind: SourceKind): LifecycleOperation[] {
  const tokens = tokenizeSource(source, sourceKind);
  const operations: LifecycleOperation[] = [];
  for (let index = 0; index < tokens.length; index += 1) {
    if (
      tokenIs(tokens, index, 'identifier', 'selectAll') &&
      tokenIs(tokens, index + 1, 'punctuator', '(') &&
      tokenIs(tokens, index + 2, 'string', '*') &&
      tokenIs(tokens, index + 3, 'punctuator', ')') &&
      tokenIs(tokens, index + 4, 'punctuator', '.') &&
      tokenIs(tokens, index + 5, 'identifier', 'remove') &&
      tokenIs(tokens, index + 6, 'punctuator', '(') &&
      tokenIs(tokens, index + 7, 'punctuator', ')')
    ) {
      operations.push({ kind: 'whole-svg-clear', index: tokens[index].index });
    }
    if (
      tokenIs(tokens, index, 'identifier', 'new') &&
      tokenIs(tokens, index + 1, 'identifier', 'ResizeObserver') &&
      tokenIs(tokens, index + 2, 'punctuator', '(')
    ) {
      operations.push({ kind: 'resize-observer', index: tokens[index].index });
    }
  }
  return operations;
}

function sourceLocation(source: ProductionSource, index: number): string {
  const line = source.source.slice(0, index).split(/\r?\n/).length;
  return `${source.path}:${line}`;
}

const productionSources = collectProductionFiles(sourceRoot).map(parseProductionSource);

describe('chart package architecture', () => {
  it('keeps dependencies inside the approved package boundaries', () => {
    const harmlessModuleText = [
      `const quoted = "import('@/features/quoted-text')";`,
      "const template = `import('@/services/template-text')`;",
      "const matcher = /don't/;",
      "// import('@/views/comment-text');",
    ].join('\n');
    expect.soft(collectModuleSpecifiers(harmlessModuleText, 'ts')).toEqual([]);

    const supportedModuleSyntax = [
      "import '@/shared/charts/core/theme';",
      'import {',
      '  ChartRenderer,',
      "} from '@/features/static';",
      "import type FeaturePort = require('@/features/type-port');",
      "import ServicePort = require('@/services/service-port');",
      'export {',
      '  ChartRenderer as PublicChartRenderer,',
      "} from '@/shared/charts';",
      "const lazy = import('@/features/lazy', { with: { type: 'json' } });",
      "const template = `load: ${import('@/services/template-expression')}`;",
    ].join('\n');
    expect.soft(collectModuleSpecifiers(supportedModuleSyntax, 'ts')).toEqual([
      '@/shared/charts/core/theme',
      '@/features/static',
      '@/features/type-port',
      '@/services/service-port',
      '@/shared/charts',
      '@/features/lazy',
      '@/services/template-expression',
    ]);

    const jsxModuleSyntax = [
      "const text = <code>import('@/features/jsx-text') don't</code>;",
      "const expression = <span>{import('@/services/jsx-expression')}</span>;",
      'const compared = left < right;',
      'const identity = <T,>(value: T) => value;',
      "const afterJsx = import('@/views/after-jsx');",
    ].join('\n');
    expect.soft(collectModuleSpecifiers(jsxModuleSyntax, 'tsx')).toEqual([
      '@/services/jsx-expression',
      '@/views/after-jsx',
    ]);

    const statementRegexModules = [
      "if (ready) /don't/.test(text);",
      "import '@/features/after-statement-regex';",
    ].join('\n');
    expect.soft(collectModuleSpecifiers(statementRegexModules, 'ts')).toEqual([
      '@/features/after-statement-regex',
    ]);

    const tsAssertionModules = [
      'const model = <ChartModel>input;',
      "import '@/features/after-ts-assertion';",
    ].join('\n');
    expect.soft(collectModuleSpecifiers(tsAssertionModules, 'ts')).toEqual([
      '@/features/after-ts-assertion',
    ]);

    const tsxGenericModules = [
      'type Mapper = <T>(value: T) => T;',
      'const identity = <T extends unknown>(value: T) => value;',
      "import '@/features/after-tsx-generics';",
    ].join('\n');
    expect.soft(collectModuleSpecifiers(tsxGenericModules, 'tsx')).toEqual([
      '@/features/after-tsx-generics',
    ]);

    const tsxAnnotatedGenericModules = [
      'const identity = <T extends unknown>(value: T): T => value;',
      "import '@/features/after-tsx-return-annotation';",
    ].join('\n');
    expect.soft(collectModuleSpecifiers(tsxAnnotatedGenericModules, 'tsx')).toEqual([
      '@/features/after-tsx-return-annotation',
    ]);

    const relativeImportFixture: ProductionSource = {
      path: 'src/shared/charts/cartesian/RelativeImportFixture.ts',
      source: "import '../statistical/RelativeChart';",
      sourceKind: 'ts',
    };
    const relativeFixtureViolations = collectForbiddenDependencyViolations([
      relativeImportFixture,
    ]);
    expect.soft(
      relativeFixtureViolations,
      'Relative imports must not bypass chart dependency rules',
    ).toEqual([
      'src/shared/charts/cartesian/RelativeImportFixture.ts imports ../statistical/RelativeChart (resolved as @/shared/charts/statistical/RelativeChart)',
    ]);

    const sharedDtoFeatureImports = productionSources
      .filter(source => source.path.startsWith('src/shared/types/dto/'))
      .flatMap(source =>
        collectModuleSpecifiers(source.source, source.sourceKind)
          .filter(specifier => specifier.startsWith('@/features/'))
          .map(specifier => `${source.path} imports ${specifier}`),
      );
    expect.soft(
      sharedDtoFeatureImports,
      `Shared DTOs must not import features:\n${sharedDtoFeatureImports.join('\n')}`,
    ).toEqual([]);

    const violations = collectForbiddenDependencyViolations(productionSources);

    expect(violations, `Forbidden chart imports:\n${violations.join('\n')}`).toEqual([]);
  });

  it('keeps SVG lifecycle and resize observation in chart core', () => {
    const wholeSvgClear = ['selectAll(', "'*'", ').remove()'].join('');
    const resizeObserver = ['new', 'ResizeObserver(callback)'].join(' ');
    const templateExpressionClear = ['${svg.', wholeSvgClear, '}'].join('');
    const lifecycleSyntax = [
      `const quoted = "${wholeSvgClear}; ${resizeObserver}";`,
      `const template = \`${wholeSvgClear}; ${templateExpressionClear}\`;`,
      `const lifecyclePattern = /${['new', 'ResizeObserver'].join(' ')}|selectAll/;`,
      "const matcher = /don't/;",
      `// ${resizeObserver};`,
      `${resizeObserver};`,
    ].join('\n');
    const fixtureOperations = collectLifecycleOperations(lifecycleSyntax, 'ts').map(operation => operation.kind);
    expect(fixtureOperations).toEqual(['whole-svg-clear', 'resize-observer']);

    const jsxLifecycleSyntax = [
      `<code>${wholeSvgClear} don't ${resizeObserver}</code>`,
      `<span>{svg.${wholeSvgClear}}</span>`,
      `<span>{${resizeObserver}}</span>`,
    ].join('\n');
    expect.soft(collectLifecycleOperations(jsxLifecycleSyntax, 'tsx').map(operation => operation.kind)).toEqual([
      'whole-svg-clear',
      'resize-observer',
    ]);

    const statementRegexLifecycle = [
      "if (ready) /don't/.test(text);",
      `${wholeSvgClear};`,
      `${resizeObserver};`,
    ].join('\n');
    expect.soft(collectLifecycleOperations(statementRegexLifecycle, 'ts').map(operation => operation.kind)).toEqual([
      'whole-svg-clear',
      'resize-observer',
    ]);

    const tsAssertionLifecycle = [
      'const model = <ChartModel>input;',
      `${wholeSvgClear};`,
      `${resizeObserver};`,
    ].join('\n');
    expect.soft(collectLifecycleOperations(tsAssertionLifecycle, 'ts').map(operation => operation.kind)).toEqual([
      'whole-svg-clear',
      'resize-observer',
    ]);

    const tsxGenericLifecycle = [
      'type Mapper = <T>(value: T) => T;',
      'const identity = <T extends unknown>(value: T) => value;',
      `${wholeSvgClear};`,
      `${resizeObserver};`,
    ].join('\n');
    expect.soft(collectLifecycleOperations(tsxGenericLifecycle, 'tsx').map(operation => operation.kind)).toEqual([
      'whole-svg-clear',
      'resize-observer',
    ]);

    const tsxAnnotatedGenericLifecycle = [
      'const identity = <T extends unknown>(value: T): T => value;',
      `${wholeSvgClear};`,
      `${resizeObserver};`,
    ].join('\n');
    expect.soft(
      collectLifecycleOperations(tsxAnnotatedGenericLifecycle, 'tsx').map(operation => operation.kind),
    ).toEqual(['whole-svg-clear', 'resize-observer']);

    const violations: string[] = [];

    for (const source of productionSources.filter(({ path }) => path.startsWith('src/shared/charts/'))) {
      for (const operation of collectLifecycleOperations(source.source, source.sourceKind)) {
        if (operation.kind === 'whole-svg-clear') {
          violations.push(`${sourceLocation(source, operation.index)} clears the entire SVG`);
        } else if (source.path !== 'src/shared/charts/core/useChartContainerSize.ts') {
          violations.push(`${sourceLocation(source, operation.index)} creates ResizeObserver outside chart core`);
        }
      }
    }

    expect(violations, `Forbidden chart lifecycle operations:\n${violations.join('\n')}`).toEqual([]);
  });

  it('keeps legacy PlotView and shared plot compatibility paths removed', () => {
    const violations = productionSources
      .map(source => source.path)
      .filter(path =>
        path.startsWith('src/shared/plot/') ||
        (path.startsWith('src/views/PlotView/') && path !== 'src/views/PlotView/PlotWindow.tsx'),
      );

    expect(violations, `Dead chart compatibility paths:\n${violations.join('\n')}`).toEqual([]);
  });
});
