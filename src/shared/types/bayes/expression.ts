export type UnaryOpDTO = 'neg';

export type BinaryOpDTO = 'add' | 'sub' | 'mul' | 'div' | 'pow';

export type MathFunctionDTO = 'exp' | 'log' | 'sqrt' | 'abs' | 'sin' | 'cos' | 'min' | 'max';

export type RawExpressionDTO =
  | { type: 'number'; value: number }
  | { type: 'symbol'; name: string }
  | { type: 'unary'; op: UnaryOpDTO; arg: RawExpressionDTO }
  | { type: 'binary'; op: BinaryOpDTO; left: RawExpressionDTO; right: RawExpressionDTO }
  | { type: 'call'; function: MathFunctionDTO; args: RawExpressionDTO[] };

export type ExpressionDTO =
  | { type: 'number'; value: number }
  | { type: 'data_variable'; name: string }
  | { type: 'column'; name: string }
  | { type: 'parameter'; name: string }
  | { type: 'unary'; op: UnaryOpDTO; arg: ExpressionDTO }
  | { type: 'binary'; op: BinaryOpDTO; left: ExpressionDTO; right: ExpressionDTO }
  | { type: 'call'; function: MathFunctionDTO; args: ExpressionDTO[] };

export const MATH_FUNCTIONS: readonly MathFunctionDTO[] = ['exp', 'log', 'sqrt', 'abs', 'sin', 'cos', 'min', 'max'];

export const BINARY_OPERATOR_LABELS: Record<BinaryOpDTO, string> = {
  add: '+',
  sub: '-',
  mul: '*',
  div: '/',
  pow: '^',
};
