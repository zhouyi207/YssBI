import {
  BINARY_OPERATOR_LABELS,
  type ExpressionDTO,
  type RawExpressionDTO,
} from "@/shared/types/bayes";

export function formatExpression(expression: ExpressionDTO | null): string {
  if (!expression) return "";
  switch (expression.type) {
    case "number":
      return Number.isInteger(expression.value)
        ? String(expression.value)
        : String(expression.value);
    case "column":
    case "data_variable":
    case "parameter":
      return expression.name;
    case "unary":
      return `-${formatExpressionWithParens(expression.arg)}`;
    case "binary":
      return `${formatExpressionWithParens(expression.left)} ${BINARY_OPERATOR_LABELS[expression.op]} ${formatExpressionWithParens(expression.right)}`;
    case "call":
      return `${expression.function}(${expression.args.map(formatExpression).join(", ")})`;
  }
}

export function formatRawExpression(expression: RawExpressionDTO | null): string {
  if (!expression) return "";
  switch (expression.type) {
    case "number":
      return Number.isInteger(expression.value)
        ? String(expression.value)
        : String(expression.value);
    case "symbol":
      return expression.name;
    case "unary":
      return `-${formatRawExpressionWithParens(expression.arg)}`;
    case "binary":
      return `${formatRawExpressionWithParens(expression.left)} ${BINARY_OPERATOR_LABELS[expression.op]} ${formatRawExpressionWithParens(expression.right)}`;
    case "call":
      return `${expression.function}(${expression.args.map(formatRawExpression).join(", ")})`;
  }
}

export function formatRawExpressionLatex(expression: RawExpressionDTO | null): string {
  if (!expression) return "";
  switch (expression.type) {
    case "number":
      return String(expression.value);
    case "symbol":
      return expression.name;
    case "unary":
      return `-${formatRawExpressionLatexGrouped(expression.arg)}`;
    case "binary": {
      const left = formatRawExpressionLatexGrouped(expression.left);
      const right = formatRawExpressionLatexGrouped(expression.right);
      switch (expression.op) {
        case "add":
          return `${left} + ${right}`;
        case "sub":
          return `${left} - ${right}`;
        case "mul":
          return `${left} \\cdot ${right}`;
        case "div":
          return `\\frac{${formatRawExpressionLatex(expression.left)}}{${formatRawExpressionLatex(expression.right)}}`;
        case "pow":
          return `${left}^{${formatRawExpressionLatex(expression.right)}}`;
      }
    }
    case "call": {
      const args = expression.args.map(formatRawExpressionLatex);
      if (expression.function === "sqrt" && args.length === 1) return `\\sqrt{${args[0]}}`;
      if (expression.function === "abs" && args.length === 1) return `\\left|${args[0]}\\right|`;
      const name =
        expression.function === "min" || expression.function === "max"
          ? `\\operatorname{${expression.function}}`
          : `\\${expression.function}`;
      return `${name}\\left(${args.join(", ")}\\right)`;
    }
  }
}

function formatRawExpressionLatexGrouped(expression: RawExpressionDTO): string {
  return expression.type === "binary"
    ? `\\left(${formatRawExpressionLatex(expression)}\\right)`
    : formatRawExpressionLatex(expression);
}

function formatExpressionWithParens(expression: ExpressionDTO): string {
  if (expression.type === "binary") {
    return `(${formatExpression(expression)})`;
  }
  return formatExpression(expression);
}

function formatRawExpressionWithParens(expression: RawExpressionDTO): string {
  if (expression.type === "binary") {
    return `(${formatRawExpression(expression)})`;
  }
  return formatRawExpression(expression);
}
