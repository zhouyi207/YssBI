//! Authoritative graph diagnostic codes and definition validation. Message templates live in
//! `locales/<locale>.json`, keyed by each definition's message key.
//!
//! Runtime diagnostic values remain in `yss-graph-analysis-contract`, independent of
//! display-language selection and sanitized message rendering.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use yss_graph_analysis_contract::{DiagnosticArguments, DiagnosticSeverity};

/// Compiled from `locales/<locale>.json` by the build script.
static MESSAGES: yss_i18n::StaticCatalog = include!(concat!(env!("OUT_DIR"), "/messages.rs"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphDiagnosticDefinition {
    pub code: &'static str,
    pub message_key: &'static str,
    pub default_severity: DiagnosticSeverity,
    pub blocking: bool,
    pub argument_names: &'static [&'static str],
}

macro_rules! define_graph_diagnostics {
    (
        $(
            $name:ident { $($argument:ident),* $(,)? } => {
                code: $code:literal,
                message_key: $message_key:literal,
                severity: $severity:ident,
                blocking: $blocking:literal $(,)?
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

        pub const GRAPH_DIAGNOSTIC_DEFINITIONS: &[GraphDiagnosticDefinition] = &[
            $(
                GraphDiagnosticDefinition {
                    code: $code,
                    message_key: $message_key,
                    default_severity: DiagnosticSeverity::$severity,
                    blocking: $blocking,
                    argument_names: &[$(stringify!($argument)),*],
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
    },
    ConnectionLimit { port } => {
        code: "graph.connection.limit",
        message_key: "diagnostics.graph.connection.limit",
        severity: Error,
        blocking: true,
    },
    ConnectionOrderForbidden { port } => {
        code: "graph.connection.order_forbidden",
        message_key: "diagnostics.graph.connection.order_forbidden",
        severity: Error,
        blocking: true,
    },
    ConnectionOrderRequired { port } => {
        code: "graph.connection.order_required",
        message_key: "diagnostics.graph.connection.order_required",
        severity: Error,
        blocking: true,
    },
    ConnectionOutputDirection { port } => {
        code: "graph.connection.output_direction",
        message_key: "diagnostics.graph.connection.output_direction",
        severity: Error,
        blocking: true,
    },
    DependencyValueCycle {} => {
        code: "graph.dependency.value_cycle",
        message_key: "diagnostics.graph.dependency.value_cycle",
        severity: Error,
        blocking: true,
    },

    ResourceResolutionFailed { resource_key } => {
        code: "graph.resource.resolution_failed",
        message_key: "diagnostics.graph.resource.resolution_failed",
        severity: Error,
        blocking: true,
    },
    InputConflictingBindings { port } => {
        code: "graph.input.conflicting_bindings",
        message_key: "diagnostics.graph.input.conflicting_bindings",
        severity: Error,
        blocking: true,
    },
    InputLiteralForbidden { port } => {
        code: "graph.input.literal_forbidden",
        message_key: "diagnostics.graph.input.literal_forbidden",
        severity: Error,
        blocking: true,
    },
    InputLiteralInvalid { port } => {
        code: "graph.input.literal_invalid",
        message_key: "diagnostics.graph.input.literal_invalid",
        severity: Error,
        blocking: true,
    },
    InputNotInput { port } => {
        code: "graph.input.not_input",
        message_key: "diagnostics.graph.input.not_input",
        severity: Error,
        blocking: true,
    },
    InputUnbound { port } => {
        code: "graph.input.unbound",
        message_key: "diagnostics.graph.input.unbound",
        severity: Warning,
        blocking: true,
    },
    InputUnknownPort { port } => {
        code: "graph.input.unknown_port",
        message_key: "diagnostics.graph.input.unknown_port",
        severity: Error,
        blocking: true,
    },
    InterfaceSchemaDependencyUnresolved {} => {
        code: "graph.interface.schema_dependency_unresolved",
        message_key: "diagnostics.graph.interface.schema_dependency_unresolved",
        severity: Error,
        blocking: true,
    },

    NodeUnknown { node_type } => {
        code: "graph.node.unknown",
        message_key: "diagnostics.graph.node.unknown",
        severity: Error,
        blocking: true,
    },
    NodeKernelUnavailable { node_type } => {
        code: "graph.node.kernel_unavailable",
        message_key: "diagnostics.graph.node.kernel_unavailable",
        severity: Error,
        blocking: true,
    },
    ParameterInvalid { parameter_key } => {
        code: "graph.parameter.invalid",
        message_key: "diagnostics.graph.parameter.invalid",
        severity: Error,
        blocking: true,
    },
    ParameterRequired { parameter_key } => {
        code: "graph.parameter.required",
        message_key: "diagnostics.graph.parameter.required",
        severity: Error,
        blocking: true,
    },
    ParameterUnknown { parameter_key } => {
        code: "graph.parameter.unknown",
        message_key: "diagnostics.graph.parameter.unknown",
        severity: Error,
        blocking: true,
    },
    PortBindingKindMismatch { expected_kind, actual_kind } => {
        code: "graph.port.binding_kind_mismatch",
        message_key: "diagnostics.graph.port.binding_kind_mismatch",
        severity: Error,
        blocking: true,
    },
    PortOrphan { port } => {
        code: "graph.port.orphan",
        message_key: "diagnostics.graph.port.orphan",
        severity: Error,
        blocking: true,
    },
    PortUnknown { port } => {
        code: "graph.port.unknown",
        message_key: "diagnostics.graph.port.unknown",
        severity: Error,
        blocking: true,
    },
    SchemaParameterInvalid { parameter_key } => {
        code: "graph.schema.parameter_invalid",
        message_key: "diagnostics.graph.schema.parameter_invalid",
        severity: Error,
        blocking: true,
    },
    SemanticInvalid {} => {
        code: "graph.semantic.invalid",
        message_key: "diagnostics.graph.semantic.invalid",
        severity: Error,
        blocking: true,
    },
    TypeConnectionMismatch { output, input } => {
        code: "graph.type.connection_mismatch",
        message_key: "diagnostics.graph.type.connection_mismatch",
        severity: Error,
        blocking: true,
    },
    TypeGenericConflict { type_parameter } => {
        code: "graph.type.generic_conflict",
        message_key: "diagnostics.graph.type.generic_conflict",
        severity: Error,
        blocking: true,
    },
    TypeInputNotAccepted { port } => {
        code: "graph.type.input_not_accepted",
        message_key: "diagnostics.graph.type.input_not_accepted",
        severity: Error,
        blocking: true,
    },
    TypeResolutionIncomplete { port } => {
        code: "graph.type.resolution_incomplete",
        message_key: "diagnostics.graph.type.resolution_incomplete",
        severity: Error,
        blocking: true,
    },
    FunctionBodyUnavailable { function } => {
        code: "graph.function.body_unavailable",
        message_key: "diagnostics.graph.function.body_unavailable",
        severity: Error,
        blocking: true,
    },
    FunctionAbiMismatch { function } => {
        code: "graph.function.abi_mismatch",
        message_key: "diagnostics.graph.function.abi_mismatch",
        severity: Error,
        blocking: true,
    },
    FunctionDependencyCycle { function } => {
        code: "graph.function.dependency_cycle",
        message_key: "diagnostics.graph.function.dependency_cycle",
        severity: Error,
        blocking: true,
    },
    FunctionBlocked { function } => {
        code: "graph.function.blocked",
        message_key: "diagnostics.graph.function.blocked",
        severity: Error,
        blocking: true,
    },
}

pub fn render_diagnostic(
    locale: &str,
    code: &str,
    message_key: &str,
    arguments: &DiagnosticArguments,
) -> Option<String> {
    let definition = GRAPH_DIAGNOSTIC_DEFINITIONS
        .iter()
        .find(|definition| definition.code == code && definition.message_key == message_key)?;
    let values = definition
        .argument_names
        .iter()
        .map(|name| {
            arguments.get(*name).map(|argument| {
                let value = argument
                    .chars()
                    .take(512)
                    .map(|character| {
                        if character.is_control() {
                            ' '
                        } else {
                            character
                        }
                    })
                    .collect::<String>();
                (*name, value)
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let template = yss_i18n::translate(MESSAGES, locale, message_key);
    Some(yss_i18n::format(template, &values))
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
                "diagnostic {code} ({message_key}) has no {} template",
                yss_i18n::FALLBACK_LOCALE
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
    validate_definitions(definitions, MESSAGES)
}

fn validate_definitions(
    definitions: &[GraphDiagnosticDefinition],
    catalog: &[(&str, &[(&str, yss_i18n::Message)])],
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
        let templates = catalog
            .iter()
            .filter_map(|(locale, messages)| {
                let index = messages
                    .binary_search_by(|(key, _)| (*key).cmp(definition.message_key))
                    .ok()?;
                match messages.get(index)?.1 {
                    yss_i18n::Message::Text(text) => Some((*locale, text)),
                    yss_i18n::Message::List(_) | yss_i18n::Message::Plural(_) => None,
                }
            })
            .collect::<Vec<_>>();
        if !templates
            .iter()
            .any(|(locale, _)| *locale == yss_i18n::FALLBACK_LOCALE)
        {
            return Err(GraphDiagnosticDefinitionError::MissingDefaultTemplate {
                code: definition.code.into(),
                message_key: definition.message_key.into(),
            });
        }

        let declared = canonical_names(definition.argument_names.iter().copied());
        for (locale, text) in templates {
            let referenced = extract_placeholders(text).map_err(|error| match error {
                TemplatePlaceholderError::UnmatchedBrace { brace, offset } => {
                    GraphDiagnosticDefinitionError::UnmatchedTemplateBrace {
                        code: definition.code.into(),
                        locale: locale.into(),
                        brace,
                        offset,
                    }
                }
                TemplatePlaceholderError::InvalidName { name } => {
                    GraphDiagnosticDefinitionError::InvalidTemplatePlaceholder {
                        code: definition.code.into(),
                        locale: locale.into(),
                        name,
                    }
                }
            })?;
            if declared != referenced {
                return Err(GraphDiagnosticDefinitionError::ArgumentTemplateMismatch {
                    code: definition.code.into(),
                    locale: locale.into(),
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
            b'%' if bytes.get(cursor + 1) == Some(&b'{') => {
                let name_start = cursor + 2;
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
            brace @ (b'{' | b'}') => {
                return Err(TemplatePlaceholderError::UnmatchedBrace {
                    brace: char::from(brace),
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

    #[test]
    fn rendering_rejects_unknown_codes_mismatched_keys_and_missing_arguments() {
        let definition = GraphDiagnosticKind::InputUnbound.definition();
        let arguments = DiagnosticArguments::from([("port".into(), "sales".into())]);
        assert!(
            render_diagnostic("en-US", "unknown", definition.message_key, &arguments).is_none()
        );
        assert!(render_diagnostic("en-US", definition.code, "unknown", &arguments).is_none());
        assert!(
            render_diagnostic(
                "en-US",
                definition.code,
                definition.message_key,
                &DiagnosticArguments::new(),
            )
            .is_none()
        );
    }

    #[test]
    fn rendering_keeps_injected_placeholders_literal_and_bounds_unicode_arguments() {
        let definition = GraphDiagnosticKind::TypeConnectionMismatch.definition();
        let arguments = DiagnosticArguments::from([
            ("output".into(), "%{input}\n世界".into()),
            ("input".into(), "界".repeat(513).into()),
        ]);
        let rendered =
            render_diagnostic("en-US", definition.code, definition.message_key, &arguments)
                .expect("known diagnostic with all required arguments");
        assert!(rendered.contains("%{input} 世界"));
        assert!(!rendered.contains('\n'));
        assert!(rendered.contains(&"界".repeat(512)));
        assert!(!rendered.contains(&"界".repeat(513)));
        let chinese =
            render_diagnostic("ZH_cn", definition.code, definition.message_key, &arguments)
                .expect("Chinese diagnostic");
        assert_ne!(rendered, chinese);
        assert_eq!(
            render_diagnostic(
                "unsupported",
                definition.code,
                definition.message_key,
                &arguments
            ),
            Some(rendered),
        );
    }

    #[test]
    fn definition_validation_rejects_duplicate_codes_and_missing_default_templates() {
        let definition = *GraphDiagnosticKind::InputUnbound.definition();
        assert!(matches!(
            validate_graph_diagnostic_definitions(&[definition, definition]),
            Err(GraphDiagnosticDefinitionError::DuplicateCode { .. }),
        ));
        assert!(matches!(
            validate_definitions(&[definition], &[(yss_i18n::FALLBACK_LOCALE, &[])]),
            Err(GraphDiagnosticDefinitionError::MissingDefaultTemplate { .. }),
        ));
    }

    #[test]
    fn every_template_belongs_to_a_definition() {
        for (locale, messages) in MESSAGES {
            for (key, _) in *messages {
                assert!(
                    GRAPH_DIAGNOSTIC_DEFINITIONS
                        .iter()
                        .any(|definition| definition.message_key == *key),
                    "{locale} template {key} has no diagnostic definition"
                );
            }
        }
    }

    const TEST_DEFINITION: GraphDiagnosticDefinition = GraphDiagnosticDefinition {
        code: "graph.test.template",
        message_key: "diagnostics.graph.test.template",
        default_severity: DiagnosticSeverity::Error,
        blocking: true,
        argument_names: &["value"],
    };

    fn validate_template(text: &'static str) -> Result<(), GraphDiagnosticDefinitionError> {
        let messages = [(TEST_DEFINITION.message_key, yss_i18n::Message::Text(text))];
        validate_definitions(
            &[TEST_DEFINITION],
            &[(yss_i18n::FALLBACK_LOCALE, &messages)],
        )
    }

    #[test]
    fn malformed_template_placeholders_are_typed_definition_errors() {
        assert!(matches!(
            validate_template("Broken %{value"),
            Err(GraphDiagnosticDefinitionError::UnmatchedTemplateBrace { brace: '{', .. })
        ));
        assert!(matches!(
            validate_template("Broken value}"),
            Err(GraphDiagnosticDefinitionError::UnmatchedTemplateBrace { brace: '}', .. })
        ));
        assert!(matches!(
            validate_template("Broken %{Value}"),
            Err(GraphDiagnosticDefinitionError::InvalidTemplatePlaceholder {
                name,
                ..
            }) if name.as_ref() == "Value"
        ));
    }
}
