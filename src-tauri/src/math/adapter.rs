use std::collections::HashMap;

use mathlex::{BinaryOp as LexBinaryOp, ExprKind, Expression, UnaryOp as LexUnaryOp};

use super::{
    BinaryOp, ComparisonOp, MAX_RELATIONS, MathError, MathErrorKind, MathExpr, MathInputFormat,
    MathRelation, ParseOptions, UnaryOp,
};

const MAX_INPUT_BYTES: usize = 16 * 1024;
const MAX_NODES: usize = 256;
const MAX_DEPTH: usize = 32;

pub(super) fn parse_expression(
    input: &str,
    options: ParseOptions<'_>,
) -> Result<MathExpr, MathError> {
    let mut budget = ParseBudget::new(input)?;
    parse_expression_with_budget(input, options, &mut budget)
}

fn parse_expression_with_budget(
    input: &str,
    options: ParseOptions<'_>,
    budget: &mut ParseBudget,
) -> Result<MathExpr, MathError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(MathError::new(
            MathErrorKind::EmptyInput,
            "数学表达式不能为空",
        ));
    }
    if options.format == MathInputFormat::Latex {
        if let Some(identifier) = exact_mathrm_identifier(input)? {
            budget.add_node()?;
            return Ok(MathExpr::Symbol(identifier.to_string()));
        }
        if let Some(call) = parse_latex_operator_call(input, options, budget)? {
            return Ok(call);
        }
    }
    let normalized_latex;
    let mut protected_symbols = ProtectedLatexSymbols {
        text: String::new(),
        names: HashMap::new(),
    };
    let parser_input = match options.format {
        MathInputFormat::Plain => input,
        MathInputFormat::Latex => {
            normalized_latex = normalize_latex(input)?;
            protected_symbols = prepare_latex_symbols(&normalized_latex, options.known_symbols)?;
            &protected_symbols.text
        }
    };
    let parsed = match options.format {
        MathInputFormat::Plain => mathlex::parse(parser_input),
        MathInputFormat::Latex => mathlex::parse_latex(parser_input),
    }
    .map_err(|error| {
        MathError::new(MathErrorKind::Parse, format!("数学表达式解析失败: {error}"))
    })?;
    let mut converted = convert(&parsed, options, 1, budget)?;
    if options.format == MathInputFormat::Latex {
        restore_protected_symbols(&mut converted, &protected_symbols.names);
    }
    Ok(converted)
}

pub(super) fn parse_relations(
    input: &str,
    options: ParseOptions<'_>,
) -> Result<Vec<MathRelation>, MathError> {
    let mut budget = ParseBudget::new(input)?;
    if input.trim().is_empty() {
        return Err(MathError::new(MathErrorKind::EmptyInput, "关系式不能为空"));
    }
    let mut relations = Vec::new();
    for segment in split_top_level(input, ',')? {
        let parts = scan_relation_parts(segment)?;
        if parts.operators.is_empty() {
            return Err(MathError::new(
                MathErrorKind::MissingRelation,
                "每条约束必须包含比较运算符",
            ));
        }
        budget.add_relations(parts.operators.len())?;
        for (index, op) in parts.operators.into_iter().enumerate() {
            relations.push(MathRelation {
                left: parse_expression_with_budget(parts.expressions[index], options, &mut budget)?,
                op,
                right: parse_expression_with_budget(
                    parts.expressions[index + 1],
                    options,
                    &mut budget,
                )?,
            });
        }
    }
    Ok(relations)
}

struct ParseBudget {
    nodes: usize,
    relations: usize,
}

impl ParseBudget {
    fn new(input: &str) -> Result<Self, MathError> {
        if input.len() > MAX_INPUT_BYTES {
            return Err(MathError::new(
                MathErrorKind::InputLimit,
                format!("数学输入不能超过 {MAX_INPUT_BYTES} 字节"),
            ));
        }
        Ok(Self {
            nodes: 0,
            relations: 0,
        })
    }

    fn add_node(&mut self) -> Result<(), MathError> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(MathError::new(
                MathErrorKind::NodeLimit,
                format!("数学表达式总节点数不能超过 {MAX_NODES}"),
            ));
        }
        Ok(())
    }

    fn add_relations(&mut self, count: usize) -> Result<(), MathError> {
        self.relations = self.relations.saturating_add(count);
        ensure_relation_count(self.relations)
    }
}

pub(super) fn ensure_relation_count(count: usize) -> Result<(), MathError> {
    if count > MAX_RELATIONS {
        return Err(MathError::new(
            MathErrorKind::RelationLimit,
            format!("关系数量不能超过 {MAX_RELATIONS}"),
        ));
    }
    Ok(())
}

fn convert(
    expression: &Expression,
    options: ParseOptions<'_>,
    depth: usize,
    budget: &mut ParseBudget,
) -> Result<MathExpr, MathError> {
    if depth > MAX_DEPTH {
        return Err(MathError::new(
            MathErrorKind::DepthLimit,
            "数学表达式深度不能超过 32",
        ));
    }
    budget.add_node()?;

    match &expression.kind {
        ExprKind::Integer(value) => number(*value as f64),
        ExprKind::Float(value) => number(value.value()),
        ExprKind::Variable(name) => resolve_symbol(name, options, depth, budget),
        ExprKind::Unary {
            op: LexUnaryOp::Neg,
            operand,
        } => Ok(MathExpr::Unary {
            op: UnaryOp::Neg,
            operand: Box::new(convert(operand, options, depth + 1, budget)?),
        }),
        ExprKind::Unary {
            op: LexUnaryOp::Pos,
            operand,
        } => convert(operand, options, depth + 1, budget),
        ExprKind::Binary { op, left, right } => {
            let op = match op {
                LexBinaryOp::Add => BinaryOp::Add,
                LexBinaryOp::Sub => BinaryOp::Sub,
                LexBinaryOp::Mul => BinaryOp::Mul,
                LexBinaryOp::Div => BinaryOp::Div,
                LexBinaryOp::Pow => BinaryOp::Pow,
                _ => return unsupported("不支持该二元运算符"),
            };
            Ok(MathExpr::Binary {
                op,
                left: Box::new(convert(left, options, depth + 1, budget)?),
                right: Box::new(convert(right, options, depth + 1, budget)?),
            })
        }
        ExprKind::Function { name, args }
            if name.starts_with("q_9")
                || matches!(
                    name.as_str(),
                    "exp"
                        | "ln"
                        | "sqrt"
                        | "abs"
                        | "sin"
                        | "cos"
                        | "min"
                        | "max"
                        | "Normal"
                        | "Bernoulli"
                        | "BernoulliLogit"
                        | "Poisson"
                        | "PoissonLog"
                ) =>
        {
            Ok(MathExpr::Call {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| convert(arg, options, depth + 1, budget))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }
        ExprKind::Function { name, .. } => Err(MathError::new(
            MathErrorKind::UnknownFunction,
            format!("不支持函数 {name}()"),
        )),
        _ => unsupported("表达式超出项目支持的数学子集"),
    }
}

fn parse_latex_operator_call(
    input: &str,
    options: ParseOptions<'_>,
    budget: &mut ParseBudget,
) -> Result<Option<MathExpr>, MathError> {
    let Some(rest) = input.strip_prefix("\\operatorname{") else {
        return Ok(None);
    };
    let name_end = rest
        .find('}')
        .ok_or_else(|| MathError::new(MathErrorKind::Parse, "\\operatorname 的花括号不匹配"))?;
    let name = &rest[..name_end];
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_')
    {
        return Err(MathError::new(
            MathErrorKind::Parse,
            "\\operatorname 仅支持函数或分布名称",
        ));
    }
    if !is_allowed_call(name) {
        return Err(MathError::new(
            MathErrorKind::UnknownFunction,
            format!("不支持函数或分布 {name}()"),
        ));
    }
    let call = rest[name_end + 1..].trim();
    let call = call.strip_prefix("\\left").unwrap_or(call).trim();
    let Some(arguments) = call.strip_prefix('(') else {
        return Err(MathError::new(
            MathErrorKind::Parse,
            "函数或分布名称后需要参数列表",
        ));
    };
    let arguments = arguments
        .strip_suffix("\\right)")
        .or_else(|| arguments.strip_suffix(')'))
        .ok_or_else(|| MathError::new(MathErrorKind::Parse, "函数参数括号不匹配"))?;
    let args = if arguments.trim().is_empty() {
        Vec::new()
    } else {
        split_top_level(arguments, ',')?
            .into_iter()
            .map(|argument| parse_expression_with_budget(argument, options, budget))
            .collect::<Result<Vec<_>, _>>()?
    };
    budget.add_node()?;
    Ok(Some(MathExpr::Call {
        name: name.to_string(),
        args,
    }))
}

fn is_allowed_call(name: &str) -> bool {
    matches!(
        name,
        "exp"
            | "ln"
            | "sqrt"
            | "abs"
            | "sin"
            | "cos"
            | "min"
            | "max"
            | "Normal"
            | "Bernoulli"
            | "BernoulliLogit"
            | "Poisson"
            | "PoissonLog"
    )
}

struct ProtectedLatexSymbols {
    text: String,
    names: HashMap<String, String>,
}

fn prepare_latex_symbols(
    input: &str,
    known: &[String],
) -> Result<ProtectedLatexSymbols, MathError> {
    let mut candidates = known
        .iter()
        .filter(|name| name.chars().count() > 1)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|name| std::cmp::Reverse(name.len()));
    let mut text = String::with_capacity(input.len());
    let mut names: HashMap<String, String> = HashMap::new();
    let mut index = 0;
    while index < input.len() {
        let rest = &input[index..];
        let matched = candidates.iter().find(|name| {
            rest.starts_with(name.as_str())
                && input[..index]
                    .chars()
                    .next_back()
                    .is_none_or(|ch| !ch.is_alphanumeric() && ch != '_' && ch != '\\')
                && rest[name.len()..]
                    .chars()
                    .next()
                    .is_none_or(|ch| !ch.is_alphanumeric() && ch != '_')
        });
        if let Some(name) = matched {
            let placeholder = format!("q_{}", names.len() + 900_000);
            text.push_str(&placeholder);
            names.insert(placeholder, (*name).clone());
            index += name.len();
            continue;
        }
        let ch = rest.chars().next().expect("valid character boundary");
        if ch.is_ascii_alphabetic()
            && input[..index].chars().next_back() != Some('\\')
            && input[..index]
                .chars()
                .next_back()
                .is_none_or(|previous| !previous.is_ascii_alphanumeric() && previous != '_')
        {
            let run_len = rest
                .char_indices()
                .take_while(|(_, character)| character.is_ascii_alphabetic())
                .map(|(offset, character)| offset + character.len_utf8())
                .last()
                .unwrap_or(0);
            let run = &rest[..run_len];
            let followed_by_call = rest[run_len..].trim_start().starts_with('(');
            if run.chars().count() > 1 && followed_by_call {
                let placeholder = format!("q_{}", names.len() + 900_000);
                text.push_str(&placeholder);
                names.insert(placeholder, run.to_string());
                index += run_len;
                continue;
            }
            if run.chars().count() > 1 {
                let splits = unique_segmentations(run, known, 2);
                match splits.as_slice() {
                    [parts] if parts.len() > 1 => {
                        text.push_str(&parts.join("\\cdot "));
                        index += run_len;
                        continue;
                    }
                    _ if splits.len() > 1 => {
                        return Err(MathError::new(
                            MathErrorKind::AmbiguousSymbol,
                            format!("标识符 '{run}' 可按已知符号进行多种分词"),
                        ));
                    }
                    _ => {}
                }
            }
        }
        text.push(ch);
        index += ch.len_utf8();
    }
    Ok(ProtectedLatexSymbols { text, names })
}

fn restore_protected_symbols(expr: &mut MathExpr, names: &HashMap<String, String>) {
    match expr {
        MathExpr::Symbol(name) => {
            if let Some(original) = names.get(name) {
                *name = original.clone();
            }
        }
        MathExpr::Unary { operand, .. } => restore_protected_symbols(operand, names),
        MathExpr::Binary { left, right, .. } => {
            restore_protected_symbols(left, names);
            restore_protected_symbols(right, names);
        }
        MathExpr::Call { name, args } => {
            if let Some(original) = names.get(name) {
                *name = original.clone();
            }
            for arg in args {
                restore_protected_symbols(arg, names);
            }
        }
        MathExpr::Number(_) => {}
    }
}

fn exact_mathrm_identifier(input: &str) -> Result<Option<&str>, MathError> {
    let Some(content) = input
        .strip_prefix("\\mathrm{")
        .and_then(|value| value.strip_suffix('}'))
    else {
        return Ok(None);
    };
    if content.is_empty()
        || !content
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_')
    {
        return Err(MathError::new(
            MathErrorKind::Unsupported,
            "\\mathrm 仅支持标识符",
        ));
    }
    Ok(Some(content))
}

fn normalize_latex(input: &str) -> Result<String, MathError> {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(index) = rest.find('\\') {
        output.push_str(&rest[..index]);
        let command = &rest[index..];
        if let Some(after_sizing) = strip_delimiter_sizing(command) {
            rest = after_sizing;
            continue;
        }
        let (prefix, label) = if command.starts_with("\\mathrm{") {
            ("\\mathrm{", "\\mathrm")
        } else if command.starts_with("\\operatorname{") {
            ("\\operatorname{", "\\operatorname")
        } else {
            let ch = command
                .chars()
                .next()
                .expect("command starts with backslash");
            output.push(ch);
            rest = &command[ch.len_utf8()..];
            continue;
        };
        let after_start = &command[prefix.len()..];
        let end = after_start.find('}').ok_or_else(|| {
            MathError::new(MathErrorKind::Parse, format!("{label} 的花括号不匹配"))
        })?;
        let content = &after_start[..end];
        if content.is_empty()
            || !content
                .chars()
                .all(|character| character.is_alphanumeric() || character == '_')
        {
            return Err(MathError::new(
                MathErrorKind::Unsupported,
                format!("{label} 仅支持标识符"),
            ));
        }
        output.push_str(content);
        rest = &after_start[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

fn strip_delimiter_sizing(command: &str) -> Option<&str> {
    ["\\left", "\\right", "\\middle"]
        .into_iter()
        .find_map(|prefix| command.strip_prefix(prefix))
}

fn number(value: f64) -> Result<MathExpr, MathError> {
    if value.is_finite() {
        Ok(MathExpr::Number(value))
    } else {
        Err(MathError::new(
            MathErrorKind::NonFiniteNumber,
            "数值必须是有限数",
        ))
    }
}

fn unsupported<T>(message: &str) -> Result<T, MathError> {
    Err(MathError::new(MathErrorKind::Unsupported, message))
}

fn resolve_symbol(
    name: &str,
    options: ParseOptions<'_>,
    depth: usize,
    budget: &mut ParseBudget,
) -> Result<MathExpr, MathError> {
    if options.format == MathInputFormat::Plain
        || options.known_symbols.iter().any(|known| known == name)
    {
        return Ok(MathExpr::Symbol(name.to_string()));
    }
    let splits = unique_segmentations(name, options.known_symbols, 2);
    match splits.as_slice() {
        [] => Ok(MathExpr::Symbol(name.to_string())),
        [parts] if parts.len() == 1 => Ok(MathExpr::Symbol(name.to_string())),
        [parts] => {
            let mut expressions = parts
                .iter()
                .map(|part| MathExpr::Symbol((*part).to_string()));
            let first = expressions.next().expect("segmentation is non-empty");
            expressions.try_fold(first, |left, right| {
                budget.add_node()?;
                if depth + 1 > MAX_DEPTH {
                    return Err(MathError::new(
                        MathErrorKind::DepthLimit,
                        "数学表达式深度不能超过 32",
                    ));
                }
                Ok(MathExpr::Binary {
                    op: BinaryOp::Mul,
                    left: Box::new(left),
                    right: Box::new(right),
                })
            })
        }
        _ => Err(MathError::new(
            MathErrorKind::AmbiguousSymbol,
            format!("标识符 '{name}' 可按已知符号进行多种分词"),
        )),
    }
}

fn unique_segmentations<'a>(name: &str, known: &'a [String], limit: usize) -> Vec<Vec<&'a str>> {
    fn visit<'a>(
        rest: &str,
        known: &'a [String],
        current: &mut Vec<&'a str>,
        output: &mut Vec<Vec<&'a str>>,
        limit: usize,
    ) {
        if output.len() >= limit {
            return;
        }
        if rest.is_empty() {
            output.push(current.clone());
            return;
        }
        for symbol in known {
            if !symbol.is_empty() && rest.starts_with(symbol) {
                current.push(symbol.as_str());
                visit(&rest[symbol.len()..], known, current, output, limit);
                current.pop();
            }
        }
    }
    let mut output = Vec::new();
    visit(name, known, &mut Vec::new(), &mut output, limit);
    output
}

struct RelationParts<'a> {
    expressions: Vec<&'a str>,
    operators: Vec<ComparisonOp>,
}

fn scan_relation_parts(input: &str) -> Result<RelationParts<'_>, MathError> {
    let mut expressions = Vec::new();
    let mut operators = Vec::new();
    let mut start = 0;
    let mut nesting = 0_i32;
    let mut index = 0;
    while index < input.len() {
        let rest = &input[index..];
        let ch = rest.chars().next().expect("valid character boundary");
        match ch {
            '(' | '{' | '[' => nesting += 1,
            ')' | '}' | ']' => nesting -= 1,
            _ => {}
        }
        if nesting < 0 {
            return Err(MathError::new(MathErrorKind::Parse, "括号不匹配").at(index));
        }
        if nesting == 0 {
            if let Some((length, op)) = relation_operator(rest) {
                let expression = input[start..index].trim();
                if expression.is_empty() {
                    return Err(
                        MathError::new(MathErrorKind::Parse, "比较运算符两侧都需要表达式")
                            .at(index),
                    );
                }
                expressions.push(expression);
                operators.push(op);
                index += length;
                start = index;
                continue;
            }
        }
        index += ch.len_utf8();
    }
    if nesting != 0 {
        return Err(MathError::new(MathErrorKind::Parse, "括号不匹配"));
    }
    let last = input[start..].trim();
    if last.is_empty() && !operators.is_empty() {
        return Err(MathError::new(
            MathErrorKind::Parse,
            "比较运算符右侧需要表达式",
        ));
    }
    expressions.push(last);
    Ok(RelationParts {
        expressions,
        operators,
    })
}

fn relation_operator(input: &str) -> Option<(usize, ComparisonOp)> {
    [
        ("\\leq", ComparisonOp::Le),
        ("\\sim", ComparisonOp::DistributedAs),
        ("\\le", ComparisonOp::Le),
        ("\\geq", ComparisonOp::Ge),
        ("\\ge", ComparisonOp::Ge),
        ("<=", ComparisonOp::Le),
        (">=", ComparisonOp::Ge),
        ("==", ComparisonOp::Eq),
        ("=", ComparisonOp::Eq),
        ("<", ComparisonOp::Lt),
        (">", ComparisonOp::Gt),
        ("≤", ComparisonOp::Le),
        ("≥", ComparisonOp::Ge),
        ("~", ComparisonOp::DistributedAs),
    ]
    .into_iter()
    .find_map(|(token, op)| {
        if !input.starts_with(token) {
            return None;
        }
        let next = input[token.len()..].chars().next();
        let is_latex_command = token.starts_with('\\');
        (!is_latex_command || next.is_none_or(|character| !character.is_alphabetic()))
            .then_some((token.len(), op))
    })
}

fn split_top_level(input: &str, delimiter: char) -> Result<Vec<&str>, MathError> {
    let mut segments = Vec::new();
    let mut nesting = 0_i32;
    let mut start = 0;
    for (index, ch) in input.char_indices() {
        match ch {
            '(' | '{' | '[' => nesting += 1,
            ')' | '}' | ']' => nesting -= 1,
            _ => {}
        }
        if nesting < 0 {
            return Err(MathError::new(MathErrorKind::Parse, "括号不匹配").at(index));
        }
        if ch == delimiter && nesting == 0 {
            let segment = input[start..index].trim();
            if segment.is_empty() {
                return Err(MathError::new(MathErrorKind::Parse, "逗号之间缺少约束").at(index));
            }
            segments.push(segment);
            start = index + ch.len_utf8();
        }
    }
    if nesting != 0 {
        return Err(MathError::new(MathErrorKind::Parse, "括号不匹配"));
    }
    let last = input[start..].trim();
    if last.is_empty() {
        return Err(MathError::new(MathErrorKind::Parse, "逗号后缺少约束"));
    }
    segments.push(last);
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbols(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).into()).collect()
    }

    fn plain(input: &str, known: &[String]) -> MathExpr {
        parse_expression(input, ParseOptions::plain(known)).unwrap()
    }

    #[test]
    fn parses_explicit_and_implicit_products() {
        let known = symbols(&["a", "x"]);
        for input in ["a*x", "a x", "2x", "2(x+1)"] {
            assert!(matches!(
                plain(input, &known),
                MathExpr::Binary {
                    op: BinaryOp::Mul,
                    ..
                }
            ));
        }
    }

    #[test]
    fn parses_latex_products_fraction_and_subscript() {
        let known = symbols(&["a", "x", "beta_1"]);
        for input in [r"a\cdot x", r"ax"] {
            assert!(matches!(
                parse_expression(input, ParseOptions::latex(&known)).unwrap(),
                MathExpr::Binary {
                    op: BinaryOp::Mul,
                    ..
                }
            ));
        }
        assert!(matches!(
            parse_expression(r"\frac{x}{2}", ParseOptions::latex(&known)).unwrap(),
            MathExpr::Binary {
                op: BinaryOp::Div,
                ..
            }
        ));
        assert_eq!(
            parse_expression(r"\beta_1", ParseOptions::latex(&known)).unwrap(),
            MathExpr::Symbol("beta_1".into())
        );
    }

    #[test]
    fn protects_complete_and_plain_unknown_symbols() {
        let known = symbols(&["a", "x", "age"]);
        assert_eq!(plain("age", &known), MathExpr::Symbol("age".into()));
        assert_eq!(plain("ax", &known), MathExpr::Symbol("ax".into()));
        assert_eq!(plain("ax", &[]), MathExpr::Symbol("ax".into()));
        assert_eq!(
            parse_expression(r"\mathrm{age}", ParseOptions::latex(&known)).unwrap(),
            MathExpr::Symbol("age".into())
        );
    }

    #[test]
    fn rejects_ambiguous_symbol_segmentation() {
        let known = symbols(&["a", "ab", "b", "bc", "c"]);
        let error = parse_expression("abc", ParseOptions::latex(&known)).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::AmbiguousSymbol);
    }

    #[test]
    fn power_binds_before_unary_negation() {
        let known = symbols(&["x"]);
        assert!(
            matches!(plain("-x^2", &known), MathExpr::Unary { op: UnaryOp::Neg, operand } if matches!(*operand, MathExpr::Binary { op: BinaryOp::Pow, .. }))
        );
    }

    #[test]
    fn parses_latex_distribution_relation_and_call() {
        let known = symbols(&["y", "a", "x", "b", "sigma"]);
        let relations = parse_relations(
            r"y \sim \operatorname{Normal}\left(a \cdot x + b, \sigma\right)",
            ParseOptions::latex(&known),
        )
        .unwrap();
        assert_eq!(relations[0].op, ComparisonOp::DistributedAs);
        assert!(matches!(
            relations[0].right,
            MathExpr::Call { ref name, ref args } if name == "Normal" && args.len() == 2
        ));
    }

    #[test]
    fn protects_known_latex_multiletter_symbol() {
        let known = symbols(&["ax"]);
        assert_eq!(
            parse_expression("ax", ParseOptions::latex(&known)).unwrap(),
            MathExpr::Symbol("ax".into())
        );
    }

    #[test]
    fn does_not_treat_delimiter_sizing_as_a_relation_operator() {
        let known = symbols(&["y", "x", "sigma"]);
        let relations = parse_relations(
            r"y \sim \operatorname{Normal}\left(x, \sigma\right)",
            ParseOptions::latex(&known),
        )
        .unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].op, ComparisonOp::DistributedAs);
    }

    #[test]
    fn parses_comma_and_chained_relations() {
        let known = symbols(&["a", "x"]);
        let relations = parse_relations("0 < x <= 1, a == x", ParseOptions::plain(&known)).unwrap();
        assert_eq!(relations.len(), 3);
        assert_eq!(
            relations
                .iter()
                .map(|relation| relation.op)
                .collect::<Vec<_>>(),
            [ComparisonOp::Lt, ComparisonOp::Le, ComparisonOp::Eq]
        );
    }

    #[test]
    fn rejects_oversized_relation_input() {
        let input = "x".repeat(MAX_INPUT_BYTES + 1);
        let error = parse_relations(&input, ParseOptions::plain(&[])).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::InputLimit);
    }

    #[test]
    fn rejects_too_many_comma_separated_relations() {
        let input = std::iter::repeat_n("x = 0", MAX_RELATIONS + 1)
            .collect::<Vec<_>>()
            .join(", ");
        let error = parse_relations(&input, ParseOptions::plain(&[])).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::RelationLimit);
    }

    #[test]
    fn rejects_too_many_chained_relations() {
        let input = std::iter::repeat_n("x", MAX_RELATIONS + 2)
            .collect::<Vec<_>>()
            .join(" < ");
        let error = parse_relations(&input, ParseOptions::plain(&[])).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::RelationLimit);
    }

    #[test]
    fn shares_node_budget_across_relation_expressions() {
        let input = std::iter::repeat_n("x+x+x = x+x+x", 30)
            .collect::<Vec<_>>()
            .join(", ");
        let error = parse_relations(&input, ParseOptions::plain(&[])).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::NodeLimit);
    }

    #[test]
    fn counts_chained_middle_expressions_on_both_relation_sides() {
        fn balanced_sum(leaves: usize) -> String {
            if leaves == 1 {
                return "x".to_string();
            }
            let half = leaves / 2;
            format!("({} + {})", balanced_sum(half), balanced_sum(leaves - half))
        }

        let term = balanced_sum(64);
        let input = format!("0 < {term} < {term} < 1");
        let error = parse_relations(&input, ParseOptions::plain(&[])).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::NodeLimit);
    }
}
