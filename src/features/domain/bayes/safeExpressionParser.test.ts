import { describe, expect, it } from 'vitest';
import { parseRawExpression } from './safeExpressionParser';

describe('safe Bayesian expression parser', () => {
  it('parses a simple linear predictor', () => {
    expect(parseRawExpression('a * x + b').expression).toEqual({
      type: 'binary',
      op: 'add',
      left: {
        type: 'binary',
        op: 'mul',
        left: { type: 'symbol', name: 'a' },
        right: { type: 'symbol', name: 'x' },
      },
      right: { type: 'symbol', name: 'b' },
    });
  });

  it('normalizes common LaTeX operator syntax', () => {
    expect(parseRawExpression('a \\cdot x + b').expression).toEqual(parseRawExpression('a * x + b').expression);
  });
});
