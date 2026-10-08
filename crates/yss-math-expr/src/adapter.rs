use std::collections::HashMap;

use mathlex::parser::{LatexToken, tokenizer::Token};
use mathlex::{BinaryOp as LexBinaryOp, ExprKind, Expression, UnaryOp as LexUnaryOp};

use super::{
    BinaryOp, ComparisonOp, MAX_RELATIONS, MathError, MathErrorKind, MathExpr, MathInputFormat,
    MathRelation, ParseOptions, UnaryOp,
};

const MAX_INPUT_BYTES: usize = 16 * 1024;
const MAX_NODES: usize = 256;
const MAX_DEPTH: usize = 32;
// Braced powers require both an operator and a group without doubling AST depth.
const MAX_PARSER_DEPTH: usize = 2 * MAX_DEPTH;

pub(super) fn parse_expression(
    input: &str,
    options: ParseOptions<'_>,
) -> Result<MathExpr, MathError> {
    let mut budget = ParseBudget::new(input)?;
    parse_expression_with_budget(input, options, 1, &mut budget)
}

fn parse_expression_with_budget(
    input: &str,
    options: ParseOptions<'_>,
    depth: usize,
    budget: &mut ParseBudget,
) -> Result<MathExpr, MathError> {
    ensure_depth(depth)?;
    let input = input.trim();
    if input.is_empty() {
        return Err(MathError::new(
            MathErrorKind::EmptyInput,
            "数学表达式不能为空",
        ));
    }
    let normalized_latex;
    let mut protected_input = ProtectedLatexInput {
        text: String::new(),
        names: HashMap::new(),
    };
    let parser_input = match options.format {
        MathInputFormat::Plain => input,
        MathInputFormat::Latex => {
            normalized_latex = normalize_latex(input);
            protected_input = prepare_latex_input(&normalized_latex, options.known_symbols)?;
            &protected_input.text
        }
    };
    ensure_parser_depth(parser_input, options.format, depth)?;
    let parsed = match options.format {
        MathInputFormat::Plain => mathlex::parse(parser_input),
        MathInputFormat::Latex => mathlex::parse_latex(parser_input),
    }
    .map_err(parser_error)?;
    convert(&parsed, options, depth, budget, &protected_input.names)
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
                left: parse_expression_with_budget(
                    parts.expressions[index],
                    options,
                    1,
                    &mut budget,
                )?,
                op,
                right: parse_expression_with_budget(
                    parts.expressions[index + 1],
                    options,
                    1,
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

fn ensure_depth(depth: usize) -> Result<(), MathError> {
    if depth > MAX_DEPTH {
        return Err(MathError::new(
            MathErrorKind::DepthLimit,
            "数学表达式深度不能超过 32",
        ));
    }
    Ok(())
}

fn parser_error(error: mathlex::ParseError) -> MathError {
    MathError::new(MathErrorKind::Parse, format!("数学表达式解析失败: {error}"))
}

#[derive(Clone, Copy)]
enum RecursionToken {
    // Paired pipes choose their direction from operand position.
    Delimiter(Option<bool>),
    Descend,
    Sign,
    Boundary,
    Value,
}

fn ensure_parser_depth(
    input: &str,
    format: MathInputFormat,
    depth: usize,
) -> Result<(), MathError> {
    match format {
        MathInputFormat::Plain => {
            let tokens = mathlex::parser::tokenize(input).map_err(parser_error)?;
            check_parser_depth(
                tokens.iter().map(|token| match token.value {
                    Token::LParen | Token::LBracket | Token::LBrace => {
                        RecursionToken::Delimiter(Some(true))
                    }
                    Token::RParen | Token::RBracket | Token::RBrace => {
                        RecursionToken::Delimiter(Some(false))
                    }
                    Token::Caret
                    | Token::DoubleStar
                    | Token::Sqrt
                    | Token::Not
                    | Token::Grad
                    | Token::Div
                    | Token::Curl
                    | Token::Laplacian => RecursionToken::Descend,
                    Token::Plus | Token::Minus => RecursionToken::Sign,
                    Token::Star
                    | Token::Slash
                    | Token::Percent
                    | Token::Comma
                    | Token::Semicolon => RecursionToken::Boundary,
                    _ => RecursionToken::Value,
                }),
                depth,
            )
        }
        MathInputFormat::Latex => {
            let tokens = mathlex::parser::tokenize_latex(input).map_err(parser_error)?;
            check_parser_depth(
                tokens.iter().map(|(token, _)| match token {
                    LatexToken::LParen
                    | LatexToken::LBracket
                    | LatexToken::LBrace
                    | LatexToken::BeginEnv(_) => RecursionToken::Delimiter(Some(true)),
                    LatexToken::RParen
                    | LatexToken::RBracket
                    | LatexToken::RBrace
                    | LatexToken::EndEnv(_) => RecursionToken::Delimiter(Some(false)),
                    LatexToken::Pipe => RecursionToken::Delimiter(None),
                    LatexToken::Caret | LatexToken::Underscore | LatexToken::Lnot => {
                        RecursionToken::Descend
                    }
                    LatexToken::Command(command)
                        if matches!(command.as_str(), "cdot" | "times" | "div" | "pm" | "mp") =>
                    {
                        RecursionToken::Boundary
                    }
                    // Commands may take unbraced arguments and recurse without delimiters.
                    LatexToken::Command(_) => RecursionToken::Descend,
                    LatexToken::Plus | LatexToken::Minus => RecursionToken::Sign,
                    LatexToken::Star
                    | LatexToken::Slash
                    | LatexToken::Comma
                    | LatexToken::Cdot
                    | LatexToken::Cross => RecursionToken::Boundary,
                    _ => RecursionToken::Value,
                }),
                depth,
            )
        }
    }
}

fn check_parser_depth(
    tokens: impl IntoIterator<Item = RecursionToken>,
    initial_depth: usize,
) -> Result<(), MathError> {
    let mut groups = Vec::new();
    let mut depth = initial_depth;
    let mut expects_operand = true;
    for token in tokens {
        match token {
            RecursionToken::Delimiter(direction) => {
                let opening = direction.unwrap_or(expects_operand);
                if opening {
                    groups.push(depth);
                    // This invocation's group stack is separate from outer call
                    // conversion depth, including groups around temporary atoms.
                    ensure_depth(1 + groups.len())?;
                    depth += 1;
                } else {
                    depth = groups.pop().unwrap_or(initial_depth);
                }
                expects_operand = opening;
            }
            RecursionToken::Descend => {
                depth += 1;
                expects_operand = true;
            }
            RecursionToken::Sign if expects_operand => depth += 1,
            RecursionToken::Sign | RecursionToken::Boundary => {
                depth = groups.last().map_or(initial_depth, |depth| depth + 1);
                expects_operand = true;
            }
            RecursionToken::Value => expects_operand = false,
        }
        if depth > MAX_PARSER_DEPTH {
            return Err(MathError::new(
                MathErrorKind::DepthLimit,
                format!("数学输入递归嵌套不能超过 {MAX_PARSER_DEPTH}"),
            ));
        }
    }
    Ok(())
}

fn convert(
    expression: &Expression,
    options: ParseOptions<'_>,
    depth: usize,
    budget: &mut ParseBudget,
    protected_names: &HashMap<String, ProtectedLatexAtom<'_>>,
) -> Result<MathExpr, MathError> {
    ensure_depth(depth)?;
    if let ExprKind::Variable(name) = &expression.kind
        && let Some(ProtectedLatexAtom::Call(input)) = protected_names.get(name)
    {
        return parse_latex_operator_call(input, options, depth, budget);
    }
    budget.add_node()?;

    match &expression.kind {
        ExprKind::Integer(value) => number(*value as f64),
        ExprKind::Float(value) => number(value.value()),
        ExprKind::Variable(name) => {
            if let Some(ProtectedLatexAtom::Symbol(original)) = protected_names.get(name) {
                Ok(MathExpr::Symbol((*original).to_owned()))
            } else {
                resolve_symbol(name, options, depth, budget)
            }
        }
        ExprKind::Unary {
            op: LexUnaryOp::Neg,
            operand,
        } => Ok(MathExpr::Unary {
            op: UnaryOp::Neg,
            operand: Box::new(convert(
                operand,
                options,
                depth + 1,
                budget,
                protected_names,
            )?),
        }),
        ExprKind::Unary {
            op: LexUnaryOp::Pos,
            operand,
        } => convert(operand, options, depth + 1, budget, protected_names),
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
                left: Box::new(convert(left, options, depth + 1, budget, protected_names)?),
                right: Box::new(convert(right, options, depth + 1, budget, protected_names)?),
            })
        }
        ExprKind::Function { name, args } => {
            if !is_allowed_call(name) {
                return Err(MathError::new(
                    MathErrorKind::UnknownFunction,
                    format!("不支持函数 {name}()"),
                ));
            }
            Ok(MathExpr::Call {
                name: name.to_owned(),
                args: args
                    .iter()
                    .map(|arg| convert(arg, options, depth + 1, budget, protected_names))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }
        _ => unsupported("表达式超出项目支持的数学子集"),
    }
}

fn parse_latex_operator_call(
    input: &str,
    options: ParseOptions<'_>,
    depth: usize,
    budget: &mut ParseBudget,
) -> Result<MathExpr, MathError> {
    let rest = input
        .strip_prefix("\\operatorname{")
        .expect("captured operator call");
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
    let Some(arguments) = call.strip_prefix('(') else {
        return Err(MathError::new(
            MathErrorKind::Parse,
            "函数或分布名称后需要参数列表",
        ));
    };
    let arguments = arguments
        .strip_suffix(')')
        .ok_or_else(|| MathError::new(MathErrorKind::Parse, "函数参数括号不匹配"))?;
    let args = if arguments.trim().is_empty() {
        Vec::new()
    } else {
        split_top_level(arguments, ',')?
            .into_iter()
            .map(|argument| parse_expression_with_budget(argument, options, depth + 1, budget))
            .collect::<Result<Vec<_>, _>>()?
    };
    budget.add_node()?;
    Ok(MathExpr::Call {
        name: name.to_string(),
        args,
    })
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

enum ProtectedLatexAtom<'a> {
    Symbol(&'a str),
    Call(&'a str),
}

struct ProtectedLatexInput<'a> {
    text: String,
    names: HashMap<String, ProtectedLatexAtom<'a>>,
}

fn prepare_latex_input<'a>(
    input: &'a str,
    known: &'a [String],
) -> Result<ProtectedLatexInput<'a>, MathError> {
    let mut candidates = known
        .iter()
        .filter(|name| name.chars().count() > 1)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|name| std::cmp::Reverse(name.len()));
    let mut text = String::with_capacity(input.len());
    let mut names = HashMap::new();
    let input_digits = input
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    let mut next_placeholder = 900_000;
    let mut protect = |atom| loop {
        let suffix = next_placeholder.to_string();
        next_placeholder += 1;
        // A fresh numeric subscript cannot alias a user spelling, including braces.
        if input_digits.contains(&suffix) {
            continue;
        }
        let placeholder = format!("q_{suffix}");
        names.insert(placeholder.clone(), atom);
        // Group a subscripted placeholder so subsequent powers apply to the atom.
        return format!("{{{placeholder}}}");
    };
    let mut index = 0;
    while index < input.len() {
        let rest = &input[index..];
        if rest.starts_with("\\operatorname{") {
            let end = latex_operator_call_end(rest)?;
            text.push_str(&protect(ProtectedLatexAtom::Call(&rest[..end])));
            index += end;
            continue;
        }
        if rest.starts_with("\\mathrm{") {
            let end = rest
                .find('}')
                .ok_or_else(|| MathError::new(MathErrorKind::Parse, "\\mathrm 的花括号不匹配"))?
                + 1;
            let identifier = parse_mathrm_identifier(&rest[..end])?;
            text.push_str(&protect(ProtectedLatexAtom::Symbol(identifier)));
            index += end;
            continue;
        }
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
            text.push_str(&protect(ProtectedLatexAtom::Symbol(name)));
            index += name.len();
            continue;
        }
        let ch = rest.chars().next().expect("valid character boundary");
        if ch.is_ascii_alphabetic()
            && !input[..index].ends_with('\\')
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
                text.push_str(&protect(ProtectedLatexAtom::Symbol(run)));
                index += run_len;
                continue;
            }
            if run.chars().count() > 1 {
                match segment_symbol(run, known) {
                    SymbolSegmentation::Unique(parts) if parts.len() > 1 => {
                        text.push_str(&parts.join("\\cdot "));
                        index += run_len;
                        continue;
                    }
                    SymbolSegmentation::Ambiguous => {
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
    Ok(ProtectedLatexInput { text, names })
}

fn latex_operator_call_end(input: &str) -> Result<usize, MathError> {
    let name_end = input
        .find('}')
        .ok_or_else(|| MathError::new(MathErrorKind::Parse, "\\operatorname 的花括号不匹配"))?;
    let arguments = input[name_end + 1..].trim_start();
    if !arguments.starts_with('(') {
        return Err(MathError::new(
            MathErrorKind::Parse,
            "函数或分布名称后需要参数列表",
        ));
    }
    let start = input.len() - arguments.len();
    let mut nesting = 0;
    for (index, character) in arguments.char_indices() {
        match character {
            '(' => nesting += 1,
            ')' => {
                nesting -= 1;
                if nesting == 0 {
                    return Ok(start + index + 1);
                }
            }
            _ => {}
        }
    }
    Err(MathError::new(MathErrorKind::Parse, "函数参数括号不匹配"))
}

fn parse_mathrm_identifier(input: &str) -> Result<&str, MathError> {
    let content = input
        .strip_prefix("\\mathrm{")
        .and_then(|value| value.strip_suffix('}'))
        .expect("captured mathrm identifier");
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
    Ok(content)
}

fn normalize_latex(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(index) = rest.find('\\') {
        output.push_str(&rest[..index]);
        let command = &rest[index..];
        if let Some(after_sizing) = strip_delimiter_sizing(command) {
            rest = after_sizing;
            continue;
        }
        output.push('\\');
        rest = &command[1..];
    }
    output.push_str(rest);
    output
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
    match segment_symbol(name, options.known_symbols) {
        SymbolSegmentation::Unique(parts) if parts.len() > 1 => {
            ensure_depth(depth + parts.len() - 1)?;
            let mut expressions = parts
                .iter()
                .map(|part| MathExpr::Symbol((*part).to_string()));
            let first = expressions.next().expect("segmentation is non-empty");
            expressions.try_fold(first, |left, right| {
                // The original variable was charged before expansion; each extra
                // symbol contributes a leaf and a multiplication node.
                budget.add_node()?;
                budget.add_node()?;
                Ok(MathExpr::Binary {
                    op: BinaryOp::Mul,
                    left: Box::new(left),
                    right: Box::new(right),
                })
            })
        }
        SymbolSegmentation::Ambiguous => Err(MathError::new(
            MathErrorKind::AmbiguousSymbol,
            format!("标识符 '{name}' 可按已知符号进行多种分词"),
        )),
        SymbolSegmentation::Unknown | SymbolSegmentation::Unique(_) => {
            Ok(MathExpr::Symbol(name.to_string()))
        }
    }
}

enum SymbolSegmentation<'a> {
    Unknown,
    Unique(Vec<&'a str>),
    Ambiguous,
}

fn segment_symbol<'a>(name: &str, known: &'a [String]) -> SymbolSegmentation<'a> {
    let mut counts = vec![0_u8; name.len() + 1];
    counts[name.len()] = 1;
    // Only zero, one, or multiple interpretations matter. Calculate every suffix
    // once, including dead ends, instead of recursively enumerating split paths.
    for (index, _) in name.char_indices().rev() {
        for symbol in known {
            if !symbol.is_empty() && name[index..].starts_with(symbol) {
                counts[index] = (counts[index] + counts[index + symbol.len()]).min(2);
                if counts[index] == 2 {
                    break;
                }
            }
        }
    }
    match counts[0] {
        0 => SymbolSegmentation::Unknown,
        1 => {
            let mut parts = Vec::new();
            let mut index = 0;
            while index < name.len() {
                let symbol = known
                    .iter()
                    .find(|symbol| {
                        !symbol.is_empty()
                            && name[index..].starts_with(symbol.as_str())
                            && counts[index + symbol.len()] > 0
                    })
                    .expect("a unique segmentation has a reachable next symbol");
                parts.push(symbol.as_str());
                index += symbol.len();
            }
            SymbolSegmentation::Unique(parts)
        }
        _ => SymbolSegmentation::Ambiguous,
    }
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
        if nesting == 0
            && let Some((length, op)) = relation_operator(rest)
        {
            let expression = input[start..index].trim();
            if expression.is_empty() {
                return Err(
                    MathError::new(MathErrorKind::Parse, "比较运算符两侧都需要表达式").at(index),
                );
            }
            expressions.push(expression);
            operators.push(op);
            index += length;
            start = index;
            continue;
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
    fn symbol_expansion_charges_each_generated_node_to_the_shared_budget() {
        let known = symbols(&["a", "l", "p", "h"]);
        let input = std::iter::repeat_n(r"\alpha = 0", 26)
            .collect::<Vec<_>>()
            .join(", ");
        let error = parse_relations(&input, ParseOptions::latex(&known)).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::NodeLimit);
    }

    #[test]
    fn rejects_unsegmentable_latex_runs_without_enumerating_prefix_splits() {
        let input = format!("{}b", "a".repeat(40));
        let known = symbols(&["a", "aa"]);
        let error = parse_expression(&input, ParseOptions::latex(&known)).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::Parse);
    }

    #[test]
    fn power_binds_before_unary_negation() {
        let known = symbols(&["x"]);
        assert!(
            matches!(plain("-x^2", &known), MathExpr::Unary { op: UnaryOp::Neg, operand } if matches!(*operand, MathExpr::Binary { op: BinaryOp::Pow, .. }))
        );
    }

    #[test]
    fn rejects_unknown_calls_without_a_placeholder_prefix_escape() {
        let error =
            parse_expression("q_900000(x)", ParseOptions::plain(&symbols(&["x"]))).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::UnknownFunction);
    }

    #[test]
    fn protected_latex_symbols_do_not_replace_explicit_user_subscripts() {
        let expression =
            parse_expression(r"age + q_{900000}", ParseOptions::latex(&symbols(&["age"]))).unwrap();
        assert_eq!(
            expression,
            MathExpr::Binary {
                op: BinaryOp::Add,
                left: Box::new(MathExpr::Symbol("age".into())),
                right: Box::new(MathExpr::Symbol("q_900000".into())),
            }
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
    fn parses_explicit_latex_calls_in_binary_expressions() {
        let known = symbols(&["x"]);
        let call = MathExpr::Call {
            name: "exp".into(),
            args: vec![MathExpr::Symbol("x".into())],
        };
        for (input, left, right) in [
            (
                r"\operatorname{exp}(x)+1",
                call.clone(),
                MathExpr::Number(1.0),
            ),
            (
                r"x+\operatorname{exp}(x)",
                MathExpr::Symbol("x".into()),
                call.clone(),
            ),
        ] {
            assert_eq!(
                parse_expression(input, ParseOptions::latex(&known)).unwrap(),
                MathExpr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(left),
                    right: Box::new(right),
                }
            );
        }
        assert_eq!(
            parse_expression(r"\operatorname{exp}(x)^2", ParseOptions::latex(&known)).unwrap(),
            MathExpr::Binary {
                op: BinaryOp::Pow,
                left: Box::new(call),
                right: Box::new(MathExpr::Number(2.0))
            }
        );
        assert_eq!(
            parse_expression(
                r"2+\operatorname{min}(x,\operatorname{exp}(1))",
                ParseOptions::latex(&known)
            )
            .unwrap(),
            MathExpr::Binary {
                op: BinaryOp::Add,
                left: Box::new(MathExpr::Number(2.0)),
                right: Box::new(MathExpr::Call {
                    name: "min".into(),
                    args: vec![
                        MathExpr::Symbol("x".into()),
                        MathExpr::Call {
                            name: "exp".into(),
                            args: vec![MathExpr::Number(1.0)]
                        }
                    ],
                })
            }
        );
        let error =
            parse_expression(r"x+\operatorname{frob}(x)", ParseOptions::latex(&known)).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::UnknownFunction);
    }

    #[test]
    fn explicit_latex_calls_charge_actual_nodes_to_the_shared_budget() {
        let relations = |count| {
            std::iter::repeat_n(r"0 = \operatorname{exp}(1)+\operatorname{exp}(1)", count)
                .collect::<Vec<_>>()
                .join(", ")
        };
        assert_eq!(
            parse_relations(&relations(42), ParseOptions::latex(&[]))
                .unwrap()
                .len(),
            42
        );
        let error = parse_relations(&relations(43), ParseOptions::latex(&[])).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::NodeLimit);
    }

    #[test]
    fn protected_latex_atoms_preserve_the_expression_depth_boundary() {
        let known = symbols(&["age"]);
        let nested = |count| {
            format!(
                "{}{}{}",
                r"\operatorname{exp}(".repeat(count),
                r"\mathrm{age}",
                ")".repeat(count)
            )
        };
        assert!(parse_expression(&nested(MAX_DEPTH - 1), ParseOptions::latex(&known)).is_ok());
        let error = parse_expression(&nested(MAX_DEPTH), ParseOptions::latex(&known)).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::DepthLimit);
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
    fn rejects_excessive_unary_prefixes_before_recursive_parsing() {
        let input = format!("{}1", "-".repeat(MAX_INPUT_BYTES - 1));
        let error = parse_expression(&input, ParseOptions::plain(&[])).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::DepthLimit);
        let at_limit = format!("{}1", "-".repeat(MAX_DEPTH - 1));
        assert!(parse_expression(&at_limit, ParseOptions::plain(&[])).is_ok());
    }

    #[test]
    fn bounds_group_nesting_before_recursive_parsing() {
        for format in [MathInputFormat::Plain, MathInputFormat::Latex] {
            let options = ParseOptions {
                format,
                known_symbols: &[],
            };
            let nested = |groups| format!("{}1{}", "(".repeat(groups), ")".repeat(groups));
            let error = parse_expression(&nested(1000), options).unwrap_err();
            assert_eq!(error.kind, MathErrorKind::DepthLimit);
            assert_eq!(
                parse_expression(&nested(MAX_DEPTH - 1), options).unwrap(),
                MathExpr::Number(1.0)
            );
        }
    }

    #[test]
    fn bounds_absolute_value_nesting_before_recursive_parsing() {
        let nested = |groups| format!("{}x{}", "|".repeat(groups), "|".repeat(groups));
        let error = parse_expression(&nested(1000), ParseOptions::latex(&[])).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::DepthLimit);
        assert!(parse_expression(&nested(MAX_DEPTH - 1), ParseOptions::latex(&[])).is_ok());
        assert!(parse_expression("|x| + |x+1|", ParseOptions::latex(&[])).is_ok());
    }

    #[test]
    fn bounds_unsupported_gradient_prefixes_before_recursive_parsing() {
        let input = format!("{}x", "grad ".repeat(1000));
        let error = parse_expression(&input, ParseOptions::plain(&[])).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::DepthLimit);
        let error = parse_expression("grad x", ParseOptions::plain(&[])).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::Unsupported);
    }

    #[test]
    fn bounds_power_recursion_without_counting_independent_branches() {
        for operator in ["^", "**"] {
            let input = std::iter::repeat_n("1", 1000)
                .collect::<Vec<_>>()
                .join(operator);
            let error = parse_expression(&input, ParseOptions::plain(&[])).unwrap_err();
            assert_eq!(error.kind, MathErrorKind::DepthLimit);
        }
        let braced = |powers| format!("{}1{}", "1^{".repeat(powers), "}".repeat(powers));
        let error = parse_expression(&braced(1000), ParseOptions::latex(&[])).unwrap_err();
        assert_eq!(error.kind, MathErrorKind::DepthLimit);
        assert!(parse_expression(&braced(MAX_DEPTH - 1), ParseOptions::latex(&[])).is_ok());

        fn balanced_powers(terms: usize) -> String {
            if terms == 1 {
                return "1e-3^2".into();
            }
            format!(
                "({}+{})",
                balanced_powers(terms / 2),
                balanced_powers(terms - terms / 2)
            )
        }
        assert!(parse_expression(&balanced_powers(64), ParseOptions::plain(&[])).is_ok());
    }

    #[test]
    fn latex_operator_calls_share_the_expression_depth_limit() {
        let known = symbols(&["y", "x"]);
        for (leaf, leaf_depth) in [("x", 1), ("exp(x)", 2)] {
            let nested = |calls: usize| {
                format!(
                    "y = {}{leaf}{}",
                    r"\operatorname{exp}(".repeat(calls),
                    ")".repeat(calls)
                )
            };
            let at_limit = nested(MAX_DEPTH - leaf_depth);
            assert_eq!(
                parse_relations(&at_limit, ParseOptions::latex(&known))
                    .unwrap()
                    .len(),
                1
            );
            let too_deep = nested(MAX_DEPTH + 1 - leaf_depth);
            let error = parse_relations(&too_deep, ParseOptions::latex(&known)).unwrap_err();
            assert_eq!(error.kind, MathErrorKind::DepthLimit);
        }
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
