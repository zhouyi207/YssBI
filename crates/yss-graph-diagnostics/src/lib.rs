//! Authoritative graph diagnostic codes, localization templates, and definition validation.
//!
//! Runtime diagnostic values live in `yss-graph-analysis-contract`; this crate owns only the
//! stable graph vocabulary and the templates generated for frontend Graph diagnostics.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use yss_graph_analysis_contract::DiagnosticSeverity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticTemplate {
    pub locale: &'static str,
    pub text: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphDiagnosticDefinition {
    pub code: &'static str,
    pub message_key: &'static str,
    pub default_severity: DiagnosticSeverity,
    pub blocking: bool,
    pub argument_names: &'static [&'static str],
    pub templates: &'static [DiagnosticTemplate],
}

macro_rules! define_graph_diagnostics {
    (
        $(
            $name:ident { $($argument:ident),* $(,)? } => {
                code: $code:literal,
                message_key: $message_key:literal,
                severity: $severity:ident,
                blocking: $blocking:literal,
                en: $en:literal,
                zh: $zh:literal $(,)?
            }
        ),* $(,)?
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum GraphDiagnosticKind {
            $($name,)*
        }

        impl GraphDiagnosticKind {
            pub const fn code(self) -> &'static str {
                self.definition().code
            }

            pub const fn definition(self) -> &'static GraphDiagnosticDefinition {
                &GRAPH_DIAGNOSTIC_DEFINITIONS[self as usize]
            }

            pub fn default_severity(self) -> DiagnosticSeverity {
                self.definition().default_severity
            }
        }

        #[cfg(test)]
        const GRAPH_DIAGNOSTIC_KINDS: &[GraphDiagnosticKind] = &[$(GraphDiagnosticKind::$name,)*];

        pub const GRAPH_DIAGNOSTIC_DEFINITIONS: &[GraphDiagnosticDefinition] = &[
            $(
                GraphDiagnosticDefinition {
                    code: $code,
                    message_key: $message_key,
                    default_severity: DiagnosticSeverity::$severity,
                    blocking: $blocking,
                    argument_names: &[$(stringify!($argument)),*],
                    templates: &[
                        DiagnosticTemplate { locale: "en-US", text: $en },
                        DiagnosticTemplate { locale: "zh-CN", text: $zh },
                    ],
                },
            )*
        ];
    };
}

define_graph_diagnostics! {
    ConnectionInputDirection { port } => {
        code: "graph.connection.input_direction",
        message_key: "diagnostics.graph.connection.input_direction",
        severity: Error,
        blocking: true,
        en: "Connection target {port} must be an input port.",
        zh: "连接目标 {port} 必须是输入端口。",
    },
    ConnectionLimit { port } => {
        code: "graph.connection.limit",
        message_key: "diagnostics.graph.connection.limit",
        severity: Error,
        blocking: true,
        en: "Connection limit exceeded for {port}.",
        zh: "端口 {port} 超出连接数量限制。",
    },
    ConnectionOrderForbidden { port } => {
        code: "graph.connection.order_forbidden",
        message_key: "diagnostics.graph.connection.order_forbidden",
        severity: Error,
        blocking: true,
        en: "Connection order is forbidden for {port}.",
        zh: "端口 {port} 不允许连接顺序。",
    },
    ConnectionOrderRequired { port } => {
        code: "graph.connection.order_required",
        message_key: "diagnostics.graph.connection.order_required",
        severity: Error,
        blocking: true,
        en: "Connection order is required for {port}.",
        zh: "端口 {port} 需要连接顺序。",
    },
    ConnectionOutputDirection { port } => {
        code: "graph.connection.output_direction",
        message_key: "diagnostics.graph.connection.output_direction",
        severity: Error,
        blocking: true,
        en: "Connection source {port} must be an output port.",
        zh: "连接源 {port} 必须是输出端口。",
    },
    DependencyValueCycle {} => {
        code: "graph.dependency.value_cycle",
        message_key: "diagnostics.graph.dependency.value_cycle",
        severity: Error,
        blocking: true,
        en: "Value dependencies contain a cycle.",
        zh: "值依赖包含循环。",
    },

    ResourceResolutionFailed { resource_key } => {
        code: "graph.resource.resolution_failed",
        message_key: "diagnostics.graph.resource.resolution_failed",
        severity: Error,
        blocking: true,
        en: "Resource {resource_key} could not be resolved.",
        zh: "无法解析资源 {resource_key}。",
    },
    InputConflictingBindings { port } => {
        code: "graph.input.conflicting_bindings",
        message_key: "diagnostics.graph.input.conflicting_bindings",
        severity: Error,
        blocking: true,
        en: "Input {port} has conflicting bindings.",
        zh: "输入 {port} 存在冲突绑定。",
    },
    InputLiteralForbidden { port } => {
        code: "graph.input.literal_forbidden",
        message_key: "diagnostics.graph.input.literal_forbidden",
        severity: Error,
        blocking: true,
        en: "Input {port} does not allow a literal binding.",
        zh: "输入 {port} 不允许字面量绑定。",
    },
    InputLiteralInvalid { port } => {
        code: "graph.input.literal_invalid",
        message_key: "diagnostics.graph.input.literal_invalid",
        severity: Error,
        blocking: true,
        en: "Input {port} has an invalid persisted literal.",
        zh: "输入 {port} 的持久化字面量无效。",
    },
    InputNotInput { port } => {
        code: "graph.input.not_input",
        message_key: "diagnostics.graph.input.not_input",
        severity: Error,
        blocking: true,
        en: "Port {port} is not an input.",
        zh: "端口 {port} 不是输入端口。",
    },
    InputUnbound { port } => {
        code: "graph.input.unbound",
        message_key: "diagnostics.graph.input.unbound",
        severity: Warning,
        blocking: true,
        en: "Required input {port} is unbound.",
        zh: "必需输入 {port} 尚未绑定。",
    },
    InputUnknownPort { port } => {
        code: "graph.input.unknown_port",
        message_key: "diagnostics.graph.input.unknown_port",
        severity: Error,
        blocking: true,
        en: "Input port {port} is unknown.",
        zh: "输入端口 {port} 未知。",
    },
    InterfaceSchemaDependencyUnresolved {} => {
        code: "graph.interface.schema_dependency_unresolved",
        message_key: "diagnostics.graph.interface.schema_dependency_unresolved",
        severity: Error,
        blocking: true,
        en: "Schema-dependent interface requirements could not be resolved.",
        zh: "无法解析依赖架构的接口要求。",
    },

    NodeUnknown { node_type } => {
        code: "graph.node.unknown",
        message_key: "diagnostics.graph.node.unknown",
        severity: Error,
        blocking: true,
        en: "Node type {node_type} is unknown.",
        zh: "节点类型 {node_type} 未知。",
    },
    NodeKernelUnavailable { node_type } => {
        code: "graph.node.kernel_unavailable",
        message_key: "diagnostics.graph.node.kernel_unavailable",
        severity: Error,
        blocking: true,
        en: "Node type {node_type} has no execution kernel in this build.",
        zh: "当前版本尚未实现节点类型 {node_type} 的执行内核。",
    },
    ParameterInvalid { parameter_key } => {
        code: "graph.parameter.invalid",
        message_key: "diagnostics.graph.parameter.invalid",
        severity: Error,
        blocking: true,
        en: "Parameter {parameter_key} is invalid.",
        zh: "参数 {parameter_key} 无效。",
    },
    ParameterRequired { parameter_key } => {
        code: "graph.parameter.required",
        message_key: "diagnostics.graph.parameter.required",
        severity: Error,
        blocking: true,
        en: "Parameter {parameter_key} is required.",
        zh: "参数 {parameter_key} 是必需的。",
    },
    ParameterUnknown { parameter_key } => {
        code: "graph.parameter.unknown",
        message_key: "diagnostics.graph.parameter.unknown",
        severity: Error,
        blocking: true,
        en: "Parameter {parameter_key} is unknown.",
        zh: "参数 {parameter_key} 未知。",
    },
    PortBindingKindMismatch { expected_kind, actual_kind } => {
        code: "graph.port.binding_kind_mismatch",
        message_key: "diagnostics.graph.port.binding_kind_mismatch",
        severity: Error,
        blocking: true,
        en: "Port binding kind {actual_kind} does not match {expected_kind}.",
        zh: "端口绑定类型 {actual_kind} 与 {expected_kind} 不匹配。",
    },
    PortOrphan { port } => {
        code: "graph.port.orphan",
        message_key: "diagnostics.graph.port.orphan",
        severity: Error,
        blocking: true,
        en: "Port {port} is orphaned.",
        zh: "端口 {port} 已孤立。",
    },
    PortUnknown { port } => {
        code: "graph.port.unknown",
        message_key: "diagnostics.graph.port.unknown",
        severity: Error,
        blocking: true,
        en: "Port {port} is unknown.",
        zh: "端口 {port} 未知。",
    },
    SchemaParameterInvalid { parameter_key } => {
        code: "graph.schema.parameter_invalid",
        message_key: "diagnostics.graph.schema.parameter_invalid",
        severity: Error,
        blocking: true,
        en: "Schema parameter {parameter_key} is invalid.",
        zh: "架构参数 {parameter_key} 无效。",
    },
    SemanticInvalid {} => {
        code: "graph.semantic.invalid",
        message_key: "diagnostics.graph.semantic.invalid",
        severity: Error,
        blocking: true,
        en: "Semantic graph is invalid.",
        zh: "语义图无效。",
    },
    TypeConnectionMismatch { output, input } => {
        code: "graph.type.connection_mismatch",
        message_key: "diagnostics.graph.type.connection_mismatch",
        severity: Error,
        blocking: true,
        en: "Resolved output type at {output} is not accepted by {input}.",
        zh: "输出端口 {output} 的已解析类型不被输入端口 {input} 接受。",
    },
    TypeGenericConflict { type_parameter } => {
        code: "graph.type.generic_conflict",
        message_key: "diagnostics.graph.type.generic_conflict",
        severity: Error,
        blocking: true,
        en: "Type parameter {type_parameter} received incompatible input types.",
        zh: "类型参数 {type_parameter} 收到了不兼容的输入类型。",
    },
    TypeInputNotAccepted { port } => {
        code: "graph.type.input_not_accepted",
        message_key: "diagnostics.graph.type.input_not_accepted",
        severity: Error,
        blocking: true,
        en: "Input value type is not accepted by port {port}.",
        zh: "输入值类型不被端口 {port} 接受。",
    },
    TypeResolutionIncomplete { port } => {
        code: "graph.type.resolution_incomplete",
        message_key: "diagnostics.graph.type.resolution_incomplete",
        severity: Error,
        blocking: true,
        en: "Port {port} does not have one exact resolved type.",
        zh: "端口 {port} 尚未求解为唯一确定类型。",
    },
    FunctionBodyUnavailable { function } => {
        code: "graph.function.body_unavailable",
        message_key: "diagnostics.graph.function.body_unavailable",
        severity: Error,
        blocking: true,
        en: "Function {function} has no available body.",
        zh: "函数 {function} 的正文不可用。",
    },
    FunctionAbiMismatch { function } => {
        code: "graph.function.abi_mismatch",
        message_key: "diagnostics.graph.function.abi_mismatch",
        severity: Error,
        blocking: true,
        en: "Function {function} entry or return does not match its signature.",
        zh: "函数 {function} 的入口或返回与签名不一致。",
    },
    FunctionDependencyCycle { function } => {
        code: "graph.function.dependency_cycle",
        message_key: "diagnostics.graph.function.dependency_cycle",
        severity: Error,
        blocking: true,
        en: "Function {function} participates in a recursive call cycle.",
        zh: "函数 {function} 参与了递归调用循环。",
    },
    FunctionBlocked { function } => {
        code: "graph.function.blocked",
        message_key: "diagnostics.graph.function.blocked",
        severity: Error,
        blocking: true,
        en: "Function {function} contains blocking graph problems.",
        zh: "函数 {function} 中存在阻止运行的图问题。",
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphDiagnosticDefinitionError {
    DuplicateCode {
        code: Box<str>,
    },
    DuplicateMessageKey {
        message_key: Box<str>,
    },
    MissingDefaultTemplate {
        code: Box<str>,
        message_key: Box<str>,
    },
    ArgumentTemplateMismatch {
        code: Box<str>,
        locale: Box<str>,
        declared: Vec<Box<str>>,
        referenced: Vec<Box<str>>,
    },
    UnmatchedTemplateBrace {
        code: Box<str>,
        locale: Box<str>,
        brace: char,
        offset: usize,
    },
    InvalidTemplatePlaceholder {
        code: Box<str>,
        locale: Box<str>,
        name: Box<str>,
    },
}

impl fmt::Display for GraphDiagnosticDefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCode { code } => write!(formatter, "duplicate diagnostic code: {code}"),
            Self::DuplicateMessageKey { message_key } => {
                write!(formatter, "duplicate diagnostic message key: {message_key}")
            }
            Self::MissingDefaultTemplate { code, message_key } => write!(
                formatter,
                "diagnostic {code} ({message_key}) has no en-US template"
            ),
            Self::ArgumentTemplateMismatch {
                code,
                locale,
                declared,
                referenced,
            } => write!(
                formatter,
                "diagnostic {code} template {locale} references {referenced:?}, but declares {declared:?}"
            ),
            Self::UnmatchedTemplateBrace {
                code,
                locale,
                brace,
                offset,
            } => write!(
                formatter,
                "diagnostic {code} template {locale} has unmatched '{brace}' at byte {offset}"
            ),
            Self::InvalidTemplatePlaceholder { code, locale, name } => write!(
                formatter,
                "diagnostic {code} template {locale} has invalid placeholder '{name}'"
            ),
        }
    }
}

impl Error for GraphDiagnosticDefinitionError {}

pub fn validate_graph_diagnostic_definitions(
    definitions: &[GraphDiagnosticDefinition],
) -> Result<(), GraphDiagnosticDefinitionError> {
    let mut codes = BTreeSet::new();
    let mut message_keys = BTreeSet::new();

    for definition in definitions {
        if !codes.insert(definition.code) {
            return Err(GraphDiagnosticDefinitionError::DuplicateCode {
                code: definition.code.into(),
            });
        }
        if !message_keys.insert(definition.message_key) {
            return Err(GraphDiagnosticDefinitionError::DuplicateMessageKey {
                message_key: definition.message_key.into(),
            });
        }
        if !definition
            .templates
            .iter()
            .any(|template| template.locale == "en-US")
        {
            return Err(GraphDiagnosticDefinitionError::MissingDefaultTemplate {
                code: definition.code.into(),
                message_key: definition.message_key.into(),
            });
        }

        let declared = canonical_names(definition.argument_names.iter().copied());
        for template in definition.templates {
            let referenced = extract_placeholders(template.text).map_err(|error| match error {
                TemplatePlaceholderError::UnmatchedBrace { brace, offset } => {
                    GraphDiagnosticDefinitionError::UnmatchedTemplateBrace {
                        code: definition.code.into(),
                        locale: template.locale.into(),
                        brace,
                        offset,
                    }
                }
                TemplatePlaceholderError::InvalidName { name } => {
                    GraphDiagnosticDefinitionError::InvalidTemplatePlaceholder {
                        code: definition.code.into(),
                        locale: template.locale.into(),
                        name,
                    }
                }
            })?;
            if declared != referenced {
                return Err(GraphDiagnosticDefinitionError::ArgumentTemplateMismatch {
                    code: definition.code.into(),
                    locale: template.locale.into(),
                    declared,
                    referenced,
                });
            }
        }
    }

    Ok(())
}

fn canonical_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<Box<str>> {
    names
        .into_iter()
        .map(Box::<str>::from)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TemplatePlaceholderError {
    UnmatchedBrace { brace: char, offset: usize },
    InvalidName { name: Box<str> },
}

fn extract_placeholders(template: &str) -> Result<Vec<Box<str>>, TemplatePlaceholderError> {
    let bytes = template.as_bytes();
    let mut names = BTreeSet::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        match bytes[cursor] {
            b'{' => {
                let name_start = cursor + 1;
                let Some(relative_end) = bytes[name_start..].iter().position(|byte| *byte == b'}')
                else {
                    return Err(TemplatePlaceholderError::UnmatchedBrace {
                        brace: '{',
                        offset: cursor,
                    });
                };
                let name_end = name_start + relative_end;
                let name = &template[name_start..name_end];
                if !is_placeholder_name(name) {
                    return Err(TemplatePlaceholderError::InvalidName { name: name.into() });
                }
                names.insert(Box::<str>::from(name));
                cursor = name_end + 1;
            }
            b'}' => {
                return Err(TemplatePlaceholderError::UnmatchedBrace {
                    brace: '}',
                    offset: cursor,
                });
            }
            _ => cursor += 1,
        }
    }

    Ok(names.into_iter().collect())
}

fn is_placeholder_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_lowercase())
        && characters.all(|character| {
            character == '_' || character.is_ascii_lowercase() || character.is_ascii_digit()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn graph_diagnostic_definitions_are_unique_template_safe_and_dataflow_only() {
        assert!(!GRAPH_DIAGNOSTIC_DEFINITIONS.is_empty());
        validate_graph_diagnostic_definitions(GRAPH_DIAGNOSTIC_DEFINITIONS).unwrap();

        assert!(GRAPH_DIAGNOSTIC_DEFINITIONS.iter().all(|definition| {
            !definition.code.contains(".control") && !definition.code.contains(".effect")
        }));

        let codes = GRAPH_DIAGNOSTIC_DEFINITIONS
            .iter()
            .map(|definition| definition.code)
            .collect::<BTreeSet<_>>();
        let message_keys = GRAPH_DIAGNOSTIC_DEFINITIONS
            .iter()
            .map(|definition| definition.message_key)
            .collect::<BTreeSet<_>>();
        assert_eq!(codes.len(), GRAPH_DIAGNOSTIC_DEFINITIONS.len());
        assert_eq!(message_keys.len(), GRAPH_DIAGNOSTIC_DEFINITIONS.len());
        assert!(GRAPH_DIAGNOSTIC_DEFINITIONS.iter().all(|definition| {
            definition
                .templates
                .iter()
                .any(|template| template.locale == "en-US")
        }));
        assert_eq!(
            GRAPH_DIAGNOSTIC_KINDS.len(),
            GRAPH_DIAGNOSTIC_DEFINITIONS.len()
        );
        for (kind, definition) in GRAPH_DIAGNOSTIC_KINDS
            .iter()
            .zip(GRAPH_DIAGNOSTIC_DEFINITIONS)
        {
            assert_eq!(kind.code(), definition.code);
            assert_eq!(kind.definition(), definition);
            assert_eq!(kind.default_severity(), definition.default_severity);
        }
    }

    fn test_definition(templates: &'static [DiagnosticTemplate]) -> GraphDiagnosticDefinition {
        GraphDiagnosticDefinition {
            code: "graph.test.template",
            message_key: "diagnostics.graph.test.template",
            default_severity: DiagnosticSeverity::Error,
            blocking: true,
            argument_names: &["value"],
            templates,
        }
    }

    #[test]
    fn malformed_template_placeholders_are_typed_definition_errors() {
        const UNMATCHED_OPEN: &[DiagnosticTemplate] = &[DiagnosticTemplate {
            locale: "en-US",
            text: "Broken {value",
        }];
        const UNMATCHED_CLOSE: &[DiagnosticTemplate] = &[DiagnosticTemplate {
            locale: "en-US",
            text: "Broken value}",
        }];
        const INVALID_NAME: &[DiagnosticTemplate] = &[DiagnosticTemplate {
            locale: "en-US",
            text: "Broken {Value}",
        }];

        assert!(matches!(
            validate_graph_diagnostic_definitions(&[test_definition(UNMATCHED_OPEN)]),
            Err(GraphDiagnosticDefinitionError::UnmatchedTemplateBrace { brace: '{', .. })
        ));
        assert!(matches!(
            validate_graph_diagnostic_definitions(&[test_definition(UNMATCHED_CLOSE)]),
            Err(GraphDiagnosticDefinitionError::UnmatchedTemplateBrace { brace: '}', .. })
        ));
        assert!(matches!(
            validate_graph_diagnostic_definitions(&[test_definition(INVALID_NAME)]),
            Err(GraphDiagnosticDefinitionError::InvalidTemplatePlaceholder {
                name,
                ..
            }) if name.as_ref() == "Value"
        ));
    }
}
