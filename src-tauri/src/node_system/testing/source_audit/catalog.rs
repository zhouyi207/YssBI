use std::collections::{HashMap, HashSet};
use std::path::Path;

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{Expr, Item, Macro, Member, Pat, ReturnType, Type, UseTree};

use super::shared::{
    is_test_only, line_for, pattern_ident, record, rust_sources, static_string_expression,
};

fn collect_use_bindings(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    bindings: &mut Vec<(String, Vec<String>)>,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_bindings(&path.tree, prefix, bindings);
            prefix.pop();
        }
        UseTree::Name(name) if name.ident == "self" => {
            if let Some(alias) = prefix.last() {
                bindings.push((alias.clone(), prefix.clone()));
            }
        }
        UseTree::Name(name) => {
            let mut target = prefix.clone();
            target.push(name.ident.to_string());
            bindings.push((name.ident.to_string(), target));
        }
        UseTree::Rename(rename) => {
            let mut target = prefix.clone();
            target.push(rename.ident.to_string());
            bindings.push((rename.rename.to_string(), target));
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_bindings(item, prefix, bindings);
            }
        }
        UseTree::Glob(_) => {}
    }
}

#[derive(Default)]
pub(super) struct CompilerDiagnosticAudit {
    pub(super) violations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CallableContext {
    Module(String),
    Impl(String),
    Trait(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CallableKey {
    file: String,
    module: String,
    context: CallableContext,
    name: String,
}

#[derive(Clone)]
struct DiagnosticConstructorCandidate {
    key: CallableKey,
    static_parameters: HashMap<String, usize>,
    output: ReturnType,
    body: Option<syn::Block>,
}

#[derive(Default)]
struct CompilerDiagnosticSyntaxIndex {
    diagnostic_types: HashSet<String>,

    constructors: HashMap<CallableKey, usize>,
}

struct DiagnosticDefinitionCollector {
    file: String,
    modules: Vec<String>,
    owner: Option<CallableContext>,
    diagnostic_types: HashSet<String>,
    aliases: Vec<(String, Type)>,
    import_aliases: Vec<(String, String)>,
    constructors: Vec<DiagnosticConstructorCandidate>,
}

fn static_string_parameters(signature: &syn::Signature) -> HashMap<String, usize> {
    signature
        .inputs
        .iter()
        .filter_map(|argument| match argument {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(argument) => Some(argument),
        })
        .enumerate()
        .filter_map(|(position, argument)| {
            type_is_static_str_reference(&argument.ty)
                .then(|| pattern_ident(&argument.pat).map(|name| (name, position)))
                .flatten()
        })
        .collect()
}

fn module_context(modules: &[String]) -> CallableContext {
    CallableContext::Module(modules.join("::"))
}

fn callable_key(
    file: &str,
    modules: &[String],
    owner: Option<&CallableContext>,
    name: &syn::Ident,
) -> CallableKey {
    CallableKey {
        file: file.to_owned(),
        module: modules.join("::"),
        context: owner.cloned().unwrap_or_else(|| module_context(modules)),
        name: name.to_string(),
    }
}

fn impl_context(self_type: &Type) -> Option<CallableContext> {
    named_type(self_type).map(CallableContext::Impl)
}

fn named_type(value_type: &Type) -> Option<String> {
    match value_type {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Type::Reference(reference) => named_type(&reference.elem),
        Type::Paren(parenthesized) => named_type(&parenthesized.elem),
        Type::Group(group) => named_type(&group.elem),
        _ => None,
    }
}

impl<'ast> Visit<'ast> for DiagnosticDefinitionCollector {
    fn visit_item(&mut self, node: &'ast Item) {
        if is_test_only(item_attributes(node)) {
            return;
        }
        visit::visit_item(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        let code = node.fields.iter().any(|field| {
            field.ident.as_ref().is_some_and(|name| name == "code")
                && type_is_static_str_reference(&field.ty)
        });
        let detail = node
            .fields
            .iter()
            .any(|field| field.ident.as_ref().is_some_and(|name| name == "detail"));
        if code && detail {
            self.diagnostic_types.insert(node.ident.to_string());
        }
        visit::visit_item_struct(self, node);
    }

    fn visit_item_type(&mut self, node: &'ast syn::ItemType) {
        self.aliases
            .push((node.ident.to_string(), (*node.ty).clone()));
        visit::visit_item_type(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        let mut bindings = Vec::new();
        collect_use_bindings(&node.tree, &mut Vec::new(), &mut bindings);
        self.import_aliases.extend(
            bindings
                .into_iter()
                .filter_map(|(alias, target)| target.last().cloned().map(|target| (alias, target))),
        );
        visit::visit_item_use(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        self.modules.push(node.ident.to_string());
        visit::visit_item_mod(self, node);
        self.modules.pop();
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let previous = self.owner.clone();
        self.owner = impl_context(&node.self_ty);
        visit::visit_item_impl(self, node);
        self.owner = previous;
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        let previous = self.owner.clone();
        self.owner = Some(CallableContext::Trait(node.ident.to_string()));
        visit::visit_item_trait(self, node);
        self.owner = previous;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.constructors.push(DiagnosticConstructorCandidate {
            key: callable_key(
                &self.file,
                &self.modules,
                self.owner.as_ref(),
                &node.sig.ident,
            ),
            static_parameters: static_string_parameters(&node.sig),
            output: node.sig.output.clone(),
            body: Some((*node.block).clone()),
        });
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if is_test_only(&node.attrs) {
            return;
        }
        self.constructors.push(DiagnosticConstructorCandidate {
            key: callable_key(
                &self.file,
                &self.modules,
                self.owner.as_ref(),
                &node.sig.ident,
            ),
            static_parameters: static_string_parameters(&node.sig),
            output: node.sig.output.clone(),
            body: Some(node.block.clone()),
        });
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        if is_test_only(&node.attrs) {
            return;
        }
        self.constructors.push(DiagnosticConstructorCandidate {
            key: callable_key(
                &self.file,
                &self.modules,
                self.owner.as_ref(),
                &node.sig.ident,
            ),
            static_parameters: static_string_parameters(&node.sig),
            output: node.sig.output.clone(),
            body: node.default.clone(),
        });
        visit::visit_trait_item_fn(self, node);
    }
}

fn resolve_path_callable(
    current: &CallableKey,
    path: &syn::Path,
    definitions: &HashSet<CallableKey>,
) -> Option<CallableKey> {
    let segments = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let name = segments.last()?.clone();
    let mut candidates = Vec::new();
    if segments.len() == 1 {
        candidates.push(CallableKey {
            file: current.file.clone(),
            module: current.module.clone(),
            context: CallableContext::Module(current.module.clone()),
            name,
        });
    } else {
        let qualifier = &segments[segments.len() - 2];
        candidates.push(CallableKey {
            file: current.file.clone(),
            module: current.module.clone(),
            context: CallableContext::Impl(qualifier.clone()),
            name: name.clone(),
        });
        candidates.push(CallableKey {
            file: current.file.clone(),
            module: current.module.clone(),
            context: CallableContext::Trait(qualifier.clone()),
            name: name.clone(),
        });
        let mut modules = current
            .module
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        modules.extend(segments[..segments.len() - 1].iter().cloned());
        let module = modules.join("::");
        candidates.push(CallableKey {
            file: current.file.clone(),
            module: module.clone(),
            context: CallableContext::Module(module),
            name,
        });
    }
    let mut matches = candidates
        .into_iter()
        .filter(|candidate| definitions.contains(candidate));
    let resolved = matches.next()?;
    matches.next().is_none().then_some(resolved)
}

fn resolve_method_callable(
    current: &CallableKey,
    receiver: &Expr,
    receiver_type: Option<&str>,
    method: &syn::Ident,
    definitions: &HashSet<CallableKey>,
) -> Option<CallableKey> {
    if matches!(receiver, Expr::Path(path) if path.path.is_ident("self")) {
        let candidate = CallableKey {
            file: current.file.clone(),
            module: current.module.clone(),
            context: current.context.clone(),
            name: method.to_string(),
        };
        return definitions.contains(&candidate).then_some(candidate);
    }
    let receiver_type = receiver_type?;
    let candidate = CallableKey {
        file: current.file.clone(),
        module: current.module.clone(),
        context: CallableContext::Impl(receiver_type.to_owned()),
        name: method.to_string(),
    };
    definitions.contains(&candidate).then_some(candidate)
}

fn declared_return_type(candidate: &DiagnosticConstructorCandidate) -> Option<String> {
    let ReturnType::Type(_, value_type) = &candidate.output else {
        return None;
    };
    let name = named_type(value_type)?;
    if name != "Self" {
        return Some(name);
    }
    match &candidate.key.context {
        CallableContext::Impl(owner) => Some(owner.clone()),
        CallableContext::Module(_) | CallableContext::Trait(_) => None,
    }
}

fn inferred_expression_type(
    expression: &Expr,
    current: Option<&CallableKey>,
    definitions: &HashSet<CallableKey>,
    return_types: &HashMap<CallableKey, String>,
) -> Option<String> {
    match expression {
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .filter(|name| name.chars().next().is_some_and(char::is_uppercase)),
        Expr::Call(call) => {
            let current = current?;
            let Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            let callee = resolve_path_callable(current, &path.path, definitions)?;
            return_types.get(&callee).cloned()
        }
        Expr::Struct(value) => value
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Expr::Paren(value) => {
            inferred_expression_type(&value.expr, current, definitions, return_types)
        }
        Expr::Group(value) => {
            inferred_expression_type(&value.expr, current, definitions, return_types)
        }
        _ => None,
    }
}

fn local_receiver_type(
    pattern: &Pat,
    expression: &Expr,
    current: Option<&CallableKey>,
    definitions: &HashSet<CallableKey>,
    return_types: &HashMap<CallableKey, String>,
) -> Option<String> {
    match pattern {
        Pat::Type(typed) => named_type(&typed.ty),
        _ => inferred_expression_type(expression, current, definitions, return_types),
    }
}

fn unwrap_diagnostic_code_expression(expression: &Expr) -> &Expr {
    match expression {
        Expr::Call(call)
            if matches!(
                call.func.as_ref(),
                Expr::Path(path)
                    if path.path.segments.last().is_some_and(|segment| segment.ident == "new")
                        && path.path.segments.iter().any(|segment| segment.ident == "DiagnosticCode")
            ) =>
        {
            call.args
                .first()
                .map_or(expression, unwrap_diagnostic_code_expression)
        }
        Expr::Paren(parenthesized) => unwrap_diagnostic_code_expression(&parenthesized.expr),
        Expr::Group(group) => unwrap_diagnostic_code_expression(&group.expr),
        _ => expression,
    }
}

fn parameter_position_in_expression(
    expression: &Expr,
    parameters: &HashMap<String, usize>,
    bindings: &HashMap<String, Expr>,
    visiting: &mut HashSet<String>,
) -> Option<usize> {
    let expression = unwrap_diagnostic_code_expression(expression);
    match expression {
        Expr::Path(path) if path.path.segments.len() == 1 => {
            let name = path.path.segments[0].ident.to_string();
            if let Some(position) = parameters.get(&name) {
                return Some(*position);
            }
            if !visiting.insert(name.clone()) {
                return None;
            }
            let resolved = bindings.get(&name).and_then(|bound| {
                parameter_position_in_expression(bound, parameters, bindings, visiting)
            });
            visiting.remove(&name);
            resolved
        }
        Expr::Reference(reference) => {
            parameter_position_in_expression(&reference.expr, parameters, bindings, visiting)
        }
        Expr::Paren(parenthesized) => {
            parameter_position_in_expression(&parenthesized.expr, parameters, bindings, visiting)
        }
        Expr::Group(group) => {
            parameter_position_in_expression(&group.expr, parameters, bindings, visiting)
        }
        _ => None,
    }
}

struct ConstructorFlowAnalyzer<'a> {
    current: &'a CallableKey,
    parameters: &'a HashMap<String, usize>,
    diagnostic_types: &'a HashSet<String>,
    definitions: &'a HashSet<CallableKey>,
    constructors: &'a HashMap<CallableKey, usize>,
    return_types: &'a HashMap<CallableKey, String>,
    bindings: HashMap<String, Expr>,
    receiver_types: HashMap<String, String>,
    positions: HashSet<usize>,
}

impl ConstructorFlowAnalyzer<'_> {
    fn record_source(&mut self, expression: &Expr) {
        if let Some(position) = parameter_position_in_expression(
            expression,
            self.parameters,
            &self.bindings,
            &mut HashSet::new(),
        ) {
            self.positions.insert(position);
        }
    }
}

impl<'ast> Visit<'ast> for ConstructorFlowAnalyzer<'_> {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        visit::visit_local(self, node);
        if let (Some(name), Some(init)) = (pattern_ident(&node.pat), node.init.as_ref()) {
            if let Some(receiver_type) = local_receiver_type(
                &node.pat,
                &init.expr,
                Some(self.current),
                self.definitions,
                self.return_types,
            ) {
                self.receiver_types.insert(name.clone(), receiver_type);
            }
            self.bindings.insert(name, (*init.expr).clone());
        }
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        let diagnostic = node
            .path
            .segments
            .last()
            .is_some_and(|segment| self.diagnostic_types.contains(&segment.ident.to_string()));
        if diagnostic {
            for field in &node.fields {
                if matches!(&field.member, Member::Named(name) if name == "code") {
                    self.record_source(&field.expr);
                }
            }
        }
        visit::visit_expr_struct(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(path) = node.func.as_ref() {
            if let Some(callee) = resolve_path_callable(self.current, &path.path, self.definitions)
            {
                if let Some(position) = self.constructors.get(&callee) {
                    if let Some(argument) = node.args.iter().nth(*position) {
                        self.record_source(argument);
                    }
                }
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let receiver_type = match node.receiver.as_ref() {
            Expr::Path(path) if path.path.segments.len() == 1 => self
                .receiver_types
                .get(&path.path.segments[0].ident.to_string())
                .map(String::as_str),
            _ => None,
        };
        if let Some(callee) = resolve_method_callable(
            self.current,
            &node.receiver,
            receiver_type,
            &node.method,
            self.definitions,
        ) {
            if let Some(position) = self.constructors.get(&callee) {
                if let Some(argument) = node.args.iter().nth(*position) {
                    self.record_source(argument);
                }
            }
        }
        visit::visit_expr_method_call(self, node);
    }
}

fn build_compiler_diagnostic_index(files: &[(String, syn::File)]) -> CompilerDiagnosticSyntaxIndex {
    let mut collector = DiagnosticDefinitionCollector {
        file: String::new(),
        modules: Vec::new(),
        owner: None,
        diagnostic_types: HashSet::from(["NodeDiagnostic".to_owned()]),
        aliases: Vec::new(),
        import_aliases: Vec::new(),
        constructors: Vec::new(),
    };
    for (relative, file) in files {
        collector.file.clone_from(relative);
        collector.modules.clear();
        collector.owner = None;
        collector.visit_file(file);
    }

    loop {
        let mut changed = false;
        for (alias, target) in &collector.aliases {
            if named_type(target).is_some_and(|name| collector.diagnostic_types.contains(&name)) {
                changed |= collector.diagnostic_types.insert(alias.clone());
            }
        }
        for (alias, target) in &collector.import_aliases {
            if collector.diagnostic_types.contains(target) {
                changed |= collector.diagnostic_types.insert(alias.clone());
            }
        }
        if !changed {
            break;
        }
    }

    let definitions = collector
        .constructors
        .iter()
        .map(|candidate| candidate.key.clone())
        .collect::<HashSet<_>>();
    let return_types = collector
        .constructors
        .iter()
        .filter_map(|candidate| {
            declared_return_type(candidate).map(|output| (candidate.key.clone(), output))
        })
        .collect::<HashMap<_, _>>();
    let mut constructors = HashMap::<CallableKey, usize>::new();
    loop {
        let mut changed = false;
        for candidate in &collector.constructors {
            let Some(body) = &candidate.body else {
                continue;
            };
            if candidate.static_parameters.is_empty() {
                continue;
            }
            let mut analyzer = ConstructorFlowAnalyzer {
                current: &candidate.key,
                parameters: &candidate.static_parameters,
                diagnostic_types: &collector.diagnostic_types,
                definitions: &definitions,
                constructors: &constructors,
                return_types: &return_types,
                bindings: HashMap::new(),
                receiver_types: HashMap::new(),
                positions: HashSet::new(),
            };
            analyzer.visit_block(body);
            if analyzer.positions.len() == 1 {
                let position = *analyzer.positions.iter().next().unwrap();
                changed |= constructors
                    .insert(candidate.key.clone(), position)
                    .is_none();
            }
        }
        if !changed {
            break;
        }
    }

    CompilerDiagnosticSyntaxIndex {
        diagnostic_types: collector.diagnostic_types,
        constructors,
    }
}

struct CompilerDiagnosticVisitor<'a> {
    relative: &'a str,
    source: &'a str,
    audit: &'a mut CompilerDiagnosticAudit,
    index: &'a CompilerDiagnosticSyntaxIndex,
    argument_maps: Vec<HashSet<String>>,
    modules: Vec<String>,
    owner: Option<CallableContext>,
}

impl CompilerDiagnosticVisitor<'_> {
    fn report(&mut self, label: &str, token: &str) {
        record(
            &mut self.audit.violations,
            self.relative,
            line_for(self.source, token),
            label,
            token,
        );
    }

    fn inspect_string(&mut self, value: &str) {
        if value.starts_with("compiler.") {
            self.report("untyped compiler diagnostic code", value);
        }
    }

    fn receiver_is_argument_map(&self, expression: &Expr) -> bool {
        match expression {
            Expr::Path(path) if path.path.segments.len() == 1 => {
                let name = path.path.segments[0].ident.to_string();
                self.argument_maps
                    .iter()
                    .rev()
                    .any(|scope| scope.contains(&name))
            }
            Expr::Field(field) => matches!(
                &field.member,
                Member::Named(name)
                    if matches!(name.to_string().as_str(), "arguments" | "diagnostic_arguments")
            ),
            Expr::Paren(parenthesized) => self.receiver_is_argument_map(&parenthesized.expr),
            Expr::Reference(reference) => self.receiver_is_argument_map(&reference.expr),
            _ => false,
        }
    }

    fn inspect_macro_tokens(&mut self, tokens: TokenStream) {
        for token in tokens {
            match token {
                TokenTree::Group(group) => self.inspect_macro_tokens(group.stream()),
                TokenTree::Literal(literal) => {
                    if let Ok(value) = syn::parse_str::<syn::LitStr>(&literal.to_string()) {
                        self.inspect_string(&value.value());
                    }
                }
                TokenTree::Ident(_) | TokenTree::Punct(_) => {}
            }
        }
    }
}

fn item_attributes(item: &Item) -> &[syn::Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        Item::Verbatim(_) => &[],
        _ => &[],
    }
}

fn type_is_static_str_reference(value_type: &Type) -> bool {
    let Type::Reference(reference) = value_type else {
        return false;
    };
    reference
        .lifetime
        .as_ref()
        .is_some_and(|lifetime| lifetime.ident == "static")
        && matches!(
            reference.elem.as_ref(),
            Type::Path(path)
                if path.path.segments.last().is_some_and(|segment| segment.ident == "str")
        )
}

fn path_is_argument_map_constructor(path: &syn::Path) -> bool {
    path.segments.iter().any(|segment| {
        matches!(
            segment.ident.to_string().as_str(),
            "DiagnosticArguments" | "BTreeMap"
        )
    })
}

fn type_is_argument_map(value_type: &Type) -> bool {
    matches!(
        value_type,
        Type::Path(path) if path_is_argument_map_constructor(&path.path)
    )
}

fn expression_constructs_argument_map(expression: &Expr) -> bool {
    match expression {
        Expr::Call(call) => matches!(
            call.func.as_ref(),
            Expr::Path(path)
                if path_is_argument_map_constructor(&path.path)
                    && path.path.segments.last().is_some_and(|segment| {
                        matches!(segment.ident.to_string().as_str(), "new" | "default" | "from")
                    })
        ),
        Expr::Macro(expression) => expression.mac.path.segments.last().is_some_and(|segment| {
            matches!(
                segment.ident.to_string().as_str(),
                "btreemap" | "diagnostic_arguments"
            )
        }),
        Expr::Paren(parenthesized) => expression_constructs_argument_map(&parenthesized.expr),
        Expr::Group(group) => expression_constructs_argument_map(&group.expr),
        _ => false,
    }
}

fn expression_is_detail_literal(expression: &Expr) -> bool {
    if static_string_expression(expression).as_deref() == Some("detail") {
        return true;
    }
    match expression {
        Expr::Call(call)
            if matches!(
                call.func.as_ref(),
                Expr::Path(path)
                    if path.path.segments.last().is_some_and(|segment| {
                        matches!(segment.ident.to_string().as_str(), "from" | "new")
                    })
            ) =>
        {
            call.args.first().is_some_and(expression_is_detail_literal)
        }
        Expr::MethodCall(call)
            if matches!(
                call.method.to_string().as_str(),
                "into" | "to_owned" | "to_string"
            ) =>
        {
            expression_is_detail_literal(&call.receiver)
        }
        Expr::Reference(reference) => expression_is_detail_literal(&reference.expr),
        Expr::Paren(parenthesized) => expression_is_detail_literal(&parenthesized.expr),
        Expr::Group(group) => expression_is_detail_literal(&group.expr),
        _ => false,
    }
}

fn expression_contains_detail_entry(expression: &Expr) -> bool {
    match expression {
        Expr::Tuple(tuple) => tuple
            .elems
            .first()
            .is_some_and(expression_is_detail_literal),
        Expr::Array(array) => array.elems.iter().any(expression_contains_detail_entry),
        Expr::Reference(reference) => expression_contains_detail_entry(&reference.expr),
        Expr::Paren(parenthesized) => expression_contains_detail_entry(&parenthesized.expr),
        Expr::Group(group) => expression_contains_detail_entry(&group.expr),
        _ => false,
    }
}

fn pattern_argument_map_name(pattern: &Pat) -> Option<String> {
    match pattern {
        Pat::Type(typed) if type_is_argument_map(&typed.ty) => pattern_ident(&typed.pat),
        _ => None,
    }
}

impl<'ast> Visit<'ast> for CompilerDiagnosticVisitor<'_> {
    fn visit_item(&mut self, node: &'ast Item) {
        if is_test_only(item_attributes(node)) {
            return;
        }
        visit::visit_item(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        self.modules.push(node.ident.to_string());
        visit::visit_item_mod(self, node);
        self.modules.pop();
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let previous = self.owner.clone();
        self.owner = impl_context(&node.self_ty);
        visit::visit_item_impl(self, node);
        self.owner = previous;
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        let previous = self.owner.clone();
        self.owner = Some(CallableContext::Trait(node.ident.to_string()));
        visit::visit_item_trait(self, node);
        self.owner = previous;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let key = callable_key(
            self.relative,
            &self.modules,
            self.owner.as_ref(),
            &node.sig.ident,
        );
        if self.index.constructors.contains_key(&key) {
            self.report(
                "generic compiler diagnostic constructor",
                &format!("fn {}", node.sig.ident),
            );
        }
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if is_test_only(&node.attrs) {
            return;
        }
        let key = callable_key(
            self.relative,
            &self.modules,
            self.owner.as_ref(),
            &node.sig.ident,
        );
        if self.index.constructors.contains_key(&key) {
            self.report(
                "generic compiler diagnostic constructor",
                &format!("fn {}", node.sig.ident),
            );
        }
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        if is_test_only(&node.attrs) {
            return;
        }
        let key = callable_key(
            self.relative,
            &self.modules,
            self.owner.as_ref(),
            &node.sig.ident,
        );
        if self.index.constructors.contains_key(&key) {
            self.report(
                "generic compiler diagnostic constructor",
                &format!("fn {}", node.sig.ident),
            );
        }
        visit::visit_trait_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if self
            .index
            .diagnostic_types
            .contains(&node.ident.to_string())
        {
            for field in &node.fields {
                let Some(name) = &field.ident else {
                    continue;
                };
                if matches!(name.to_string().as_str(), "code" | "detail") {
                    self.report("untyped compiler issue field", &name.to_string());
                }
            }
        }
        visit::visit_item_struct(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        let type_name = node
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        if type_name
            .as_ref()
            .is_some_and(|name| self.index.diagnostic_types.contains(name))
        {
            let name = type_name.as_deref().unwrap_or("NodeDiagnostic");
            self.report(
                "direct compiler NodeDiagnostic construction",
                &format!("{name} {{"),
            );
        }

        visit::visit_expr_struct(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(path) = node.func.as_ref() {
            if path_is_argument_map_constructor(&path.path)
                && path
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "from")
                && node.args.iter().any(expression_contains_detail_entry)
            {
                self.report("generic compiler diagnostic argument", "\"detail\"");
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "insert"
            && self.receiver_is_argument_map(&node.receiver)
            && node.args.first().is_some_and(expression_is_detail_literal)
        {
            self.report("generic compiler diagnostic argument", "\"detail\"");
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        visit::visit_local(self, node);
        let name = pattern_argument_map_name(&node.pat).or_else(|| {
            let init = node.init.as_ref()?;
            expression_constructs_argument_map(&init.expr)
                .then(|| pattern_ident(&node.pat))
                .flatten()
        });
        if let (Some(name), Some(scope)) = (name, self.argument_maps.last_mut()) {
            scope.insert(name);
        }
    }

    fn visit_block(&mut self, node: &'ast syn::Block) {
        self.argument_maps.push(HashSet::new());
        visit::visit_block(self, node);
        self.argument_maps.pop();
    }

    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        self.inspect_string(&node.value());
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        let name = node
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        if matches!(
            name.as_deref(),
            Some(
                "assert"
                    | "assert_eq"
                    | "assert_ne"
                    | "debug_assert"
                    | "debug_assert_eq"
                    | "debug_assert_ne"
            )
        ) {
            return;
        }
        self.inspect_macro_tokens(node.tokens.clone());
    }
}

fn is_compiler_test_source(relative: &str) -> bool {
    let file_name = relative.rsplit('/').next().unwrap_or(relative);
    file_name == "tests.rs" || file_name.starts_with("tests_") || file_name.ends_with("_tests.rs")
}

pub(super) fn inspect_compiler_diagnostic_source(
    relative: &str,
    source: &str,
    audit: &mut CompilerDiagnosticAudit,
) {
    match syn::parse_file(source) {
        Ok(module) => {
            let index = build_compiler_diagnostic_index(&[(relative.to_owned(), module.clone())]);
            CompilerDiagnosticVisitor {
                relative,
                source,
                audit,
                index: &index,
                argument_maps: Vec::new(),
                modules: Vec::new(),
                owner: None,
            }
            .visit_file(&module);
        }
        Err(error) => record(
            &mut audit.violations,
            relative,
            1,
            "Rust source parse failure",
            &error.to_string(),
        ),
    }
}

pub(super) fn audit_compiler_diagnostic_tree(
    compiler_root: &Path,
    exclude_definition_authority: bool,
) -> CompilerDiagnosticAudit {
    let mut paths = Vec::new();
    rust_sources(compiler_root, &mut paths);
    paths.sort();

    let mut audit = CompilerDiagnosticAudit::default();
    let mut sources = Vec::new();
    for path in paths {
        let relative = path
            .strip_prefix(compiler_root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if is_compiler_test_source(&relative)
            || exclude_definition_authority && relative == "diagnostics.rs"
        {
            continue;
        }
        let source = std::fs::read_to_string(path).unwrap();
        match syn::parse_file(&source) {
            Ok(module) => sources.push((relative, source, module)),
            Err(error) => record(
                &mut audit.violations,
                &relative,
                1,
                "Rust source parse failure",
                &error.to_string(),
            ),
        }
    }

    let indexed_sources = sources
        .iter()
        .map(|(relative, _, module)| (relative.clone(), module.clone()))
        .collect::<Vec<_>>();
    let index = build_compiler_diagnostic_index(&indexed_sources);
    for (relative, source, module) in &sources {
        CompilerDiagnosticVisitor {
            relative,
            source,
            audit: &mut audit,
            index: &index,
            argument_maps: Vec::new(),
            modules: Vec::new(),
            owner: None,
        }
        .visit_file(module);
    }
    audit.violations.sort();
    audit.violations.dedup();
    audit
}
