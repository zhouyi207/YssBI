import { MATH_FUNCTIONS, type MathFunctionDTO, type RawExpressionDTO } from '@/shared/types/bayes';

export interface ParseRawExpressionResult {
  expression: RawExpressionDTO;
}

type Token =
  | { type: 'number'; value: number }
  | { type: 'identifier'; value: string }
  | { type: 'operator'; value: '+' | '-' | '*' | '/' | '^' }
  | { type: 'paren'; value: '(' | ')' }
  | { type: 'comma' }
  | { type: 'eof' };

const FUNCTION_NAMES = new Set<string>(MATH_FUNCTIONS);

export function parseRawExpression(input: string): ParseRawExpressionResult {
  const parser = new Parser(tokenize(normalizeLatexExpression(input)));
  const expression = parser.parseExpression();
  parser.expectEnd();
  return { expression };
}

export function normalizeLatexExpression(input: string): string {
  return input
    .replace(/\\cdot/g, '*')
    .replace(/\\times/g, '*')
    .replace(/\\sigma/g, 'sigma')
    .replace(/\\left/g, '')
    .replace(/\\right/g, '')
    .replace(/\\operatorname\{([A-Za-z]+)\}/g, '$1')
    .trim();
}

function tokenize(input: string): Token[] {
  const tokens: Token[] = [];
  let index = 0;

  while (index < input.length) {
    const char = input[index];
    if (/\s/.test(char)) {
      index += 1;
      continue;
    }

    if (/[0-9.]/.test(char)) {
      const start = index;
      index += 1;
      while (index < input.length && /[0-9.eE+-]/.test(input[index])) {
        const previous = input[index - 1];
        const current = input[index];
        if ((current === '+' || current === '-') && previous !== 'e' && previous !== 'E') break;
        index += 1;
      }
      const value = Number(input.slice(start, index));
      if (!Number.isFinite(value)) throw new Error(`Invalid number near "${input.slice(start, index)}"`);
      tokens.push({ type: 'number', value });
      continue;
    }

    if (/[A-Za-z_]/.test(char)) {
      const start = index;
      index += 1;
      while (index < input.length && /[A-Za-z0-9_]/.test(input[index])) index += 1;
      tokens.push({ type: 'identifier', value: input.slice(start, index) });
      continue;
    }

    if (char === '+' || char === '-' || char === '*' || char === '/' || char === '^') {
      tokens.push({ type: 'operator', value: char });
      index += 1;
      continue;
    }

    if (char === '(' || char === ')') {
      tokens.push({ type: 'paren', value: char });
      index += 1;
      continue;
    }

    if (char === ',') {
      tokens.push({ type: 'comma' });
      index += 1;
      continue;
    }

    throw new Error(`Unsupported character "${char}"`);
  }

  tokens.push({ type: 'eof' });
  return tokens;
}

class Parser {
  private position = 0;

  constructor(private readonly tokens: Token[]) {}

  parseExpression(): RawExpressionDTO {
    return this.parseAddSub();
  }

  expectEnd(): void {
    if (this.current().type !== 'eof') throw new Error('Unexpected trailing input');
  }

  private parseAddSub(): RawExpressionDTO {
    let left = this.parseMulDiv();
    while (this.isOperator('+') || this.isOperator('-')) {
      const operator = this.current();
      this.advance();
      const right = this.parseMulDiv();
      left = { type: 'binary', op: operator.type === 'operator' && operator.value === '+' ? 'add' : 'sub', left, right };
    }
    return left;
  }

  private parseMulDiv(): RawExpressionDTO {
    let left = this.parsePower();
    while (this.isOperator('*') || this.isOperator('/')) {
      const operator = this.current();
      this.advance();
      const right = this.parsePower();
      left = { type: 'binary', op: operator.type === 'operator' && operator.value === '*' ? 'mul' : 'div', left, right };
    }
    return left;
  }

  private parsePower(): RawExpressionDTO {
    const left = this.parseUnary();
    if (!this.isOperator('^')) return left;
    this.advance();
    return { type: 'binary', op: 'pow', left, right: this.parsePower() };
  }

  private parseUnary(): RawExpressionDTO {
    if (this.isOperator('-')) {
      this.advance();
      return { type: 'unary', op: 'neg', arg: this.parseUnary() };
    }
    if (this.isOperator('+')) {
      this.advance();
      return this.parseUnary();
    }
    return this.parsePrimary();
  }

  private parsePrimary(): RawExpressionDTO {
    const token = this.current();
    if (token.type === 'number') {
      this.advance();
      return { type: 'number', value: token.value };
    }

    if (token.type === 'identifier') {
      this.advance();
      if (this.isParen('(')) {
        return this.parseCall(token.value);
      }
      return { type: 'symbol', name: token.value };
    }

    if (this.isParen('(')) {
      this.advance();
      const expression = this.parseExpression();
      if (!this.isParen(')')) throw new Error('Expected closing parenthesis');
      this.advance();
      return expression;
    }

    throw new Error('Expected number, symbol, function call, or parenthesized expression');
  }

  private parseCall(name: string): RawExpressionDTO {
    if (!FUNCTION_NAMES.has(name)) throw new Error(`Unsupported function "${name}"`);
    this.advance();
    const args: RawExpressionDTO[] = [];
    if (!this.isParen(')')) {
      do {
        args.push(this.parseExpression());
        if (this.current().type !== 'comma') break;
        this.advance();
      } while (true);
    }
    if (!this.isParen(')')) throw new Error(`Expected closing parenthesis for ${name}`);
    this.advance();
    return { type: 'call', function: name as MathFunctionDTO, args };
  }

  private current(): Token {
    return this.tokens[this.position] ?? { type: 'eof' };
  }

  private advance(): Exclude<Token, { type: 'eof' }> {
    const token = this.current();
    if (token.type === 'eof') throw new Error('Unexpected end of input');
    this.position += 1;
    return token;
  }

  private isOperator(value: '+' | '-' | '*' | '/' | '^'): boolean {
    const token = this.current();
    return token.type === 'operator' && token.value === value;
  }

  private isParen(value: '(' | ')'): boolean {
    const token = this.current();
    return token.type === 'paren' && token.value === value;
  }
}
