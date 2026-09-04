//! Authoritative compiler diagnostic codes, localization templates, and definition validation.
//!
//! Runtime diagnostic values live in `yss-graph-analysis-contract`; this crate owns only the
//! stable compiler vocabulary consumed while assembling the built-in catalog.

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
pub struct CompilerDiagnosticDefinition {
    pub code: &'static str,
    pub message_key: &'static str,
    pub default_severity: DiagnosticSeverity,
    pub argument_names: &'static [&'static str],
    pub templates: &'static [DiagnosticTemplate],
}

macro_rules! define_compiler_diagnostics {
    (
        $(
            $name:ident { $($argument:ident),* $(,)? } => {
                code: $code:literal,
                message_key: $message_key:literal,
                severity: $severity:ident,
                en: $en:literal,
                zh: $zh:literal $(,)?
            }
        ),* $(,)?
    ) => {
        pub const COMPILER_DIAGNOSTIC_DEFINITIONS: &[CompilerDiagnosticDefinition] = &[
            $(
                CompilerDiagnosticDefinition {
                    code: $code,
                    message_key: $message_key,
                    default_severity: DiagnosticSeverity::$severity,
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

define_compiler_diagnostics! {
    DataframeFieldTypeUnsupported { column, schema_type, reason } => {
        code: "compiler.dataframe.field_type_unsupported",
        message_key: "diagnostics.compiler.dataframe.field_type_unsupported",
        severity: Warning,
        en: "Column {column} uses unsupported schema type {schema_type}: {reason}.",
        zh: "列 {column} 使用了不支持的 Schema 类型 {schema_type}：{reason}。",
    },
    ConnectionInputDirection { port } => {
        code: "compiler.connection.input_direction",
        message_key: "diagnostics.compiler.connection.input_direction",
        severity: Error,
        en: "Connection target {port} must be an input port.",
        zh: "连接目标 {port} 必须是输入端口。",
    },
    ConnectionLimit { port } => {
        code: "compiler.connection.limit",
        message_key: "diagnostics.compiler.connection.limit",
        severity: Error,
        en: "Connection limit exceeded for {port}.",
        zh: "端口 {port} 超出连接数量限制。",
    },
    ConnectionOrderForbidden { port } => {
        code: "compiler.connection.order_forbidden",
        message_key: "diagnostics.compiler.connection.order_forbidden",
        severity: Error,
        en: "Connection order is forbidden for {port}.",
        zh: "端口 {port} 不允许连接顺序。",
    },
    ConnectionOrderRequired { port } => {
        code: "compiler.connection.order_required",
        message_key: "diagnostics.compiler.connection.order_required",
        severity: Error,
        en: "Connection order is required for {port}.",
        zh: "端口 {port} 需要连接顺序。",
    },
    ConnectionOutputDirection { port } => {
        code: "compiler.connection.output_direction",
        message_key: "diagnostics.compiler.connection.output_direction",
        severity: Error,
        en: "Connection source {port} must be an output port.",
        zh: "连接源 {port} 必须是输出端口。",
    },
    DependencyValueCycle {} => {
        code: "compiler.dependency.value_cycle",
        message_key: "diagnostics.compiler.dependency.value_cycle",
        severity: Error,
        en: "Value dependencies contain a cycle.",
        zh: "值依赖包含循环。",
    },
    DocumentConnectionIdMismatch { expected_id, actual_id } => {
        code: "compiler.document.connection_id_mismatch",
        message_key: "diagnostics.compiler.document.connection_id_mismatch",
        severity: Error,
        en: "Connection ID {actual_id} does not match {expected_id}.",
        zh: "连接 ID {actual_id} 与 {expected_id} 不匹配。",
    },
    DocumentNodeIdMismatch { expected_id, actual_id } => {
        code: "compiler.document.node_id_mismatch",
        message_key: "diagnostics.compiler.document.node_id_mismatch",
        severity: Error,
        en: "Node ID {actual_id} does not match {expected_id}.",
        zh: "节点 ID {actual_id} 与 {expected_id} 不匹配。",
    },
    FunctionAbiEndpointInvalid { port } => {
        code: "compiler.function.abi.endpoint_invalid",
        message_key: "diagnostics.compiler.function.abi.endpoint_invalid",
        severity: Error,
        en: "Function ABI endpoint {port} is invalid.",
        zh: "函数 ABI 端点 {port} 无效。",
    },
    FunctionAbiLocatorInvalid { port } => {
        code: "compiler.function.abi.locator_invalid",
        message_key: "diagnostics.compiler.function.abi.locator_invalid",
        severity: Error,
        en: "Function ABI locator for {port} is invalid.",
        zh: "端口 {port} 的函数 ABI 定位器无效。",
    },
    FunctionAbiLocatorTargetMismatch { function_path } => {
        code: "compiler.function.abi.locator_target_mismatch",
        message_key: "diagnostics.compiler.function.abi.locator_target_mismatch",
        severity: Error,
        en: "Function ABI locator does not target {function_path}.",
        zh: "函数 ABI 定位器未指向 {function_path}。",
    },
    FunctionAbiManagedRoleInvalid { expected_role, actual_count } => {
        code: "compiler.function.abi.managed_role_invalid",
        message_key: "diagnostics.compiler.function.abi.managed_role_invalid",
        severity: Error,
        en: "Function ABI requires one {expected_role} node but found {actual_count}.",
        zh: "函数 ABI 需要一个 {expected_role} 节点，但找到 {actual_count} 个。",
    },
    FunctionAbiMemberDuplicate { field_name } => {
        code: "compiler.function.abi.member_duplicate",
        message_key: "diagnostics.compiler.function.abi.member_duplicate",
        severity: Error,
        en: "Function ABI member {field_name} is duplicated.",
        zh: "函数 ABI 成员 {field_name} 重复。",
    },
    FunctionAbiMemberMissing { field_name } => {
        code: "compiler.function.abi.member_missing",
        message_key: "diagnostics.compiler.function.abi.member_missing",
        severity: Error,
        en: "Function ABI member {field_name} is missing.",
        zh: "缺少函数 ABI 成员 {field_name}。",
    },
    FunctionAbiMemberUnexpected { field_name } => {
        code: "compiler.function.abi.member_unexpected",
        message_key: "diagnostics.compiler.function.abi.member_unexpected",
        severity: Error,
        en: "Function ABI member {field_name} is unexpected.",
        zh: "函数 ABI 成员 {field_name} 不符合预期。",
    },

    ResourceDisplayNameUnavailable { resource_key, reason } => {
        code: "compiler.resource.display_name_unavailable",
        message_key: "diagnostics.compiler.resource.display_name_unavailable",
        severity: Warning,
        en: "Resource {resource_key} uses the default node title: {reason}.",
        zh: "资源 {resource_key} 使用默认节点标题：{reason}。",
    },
    ResourceResolutionFailed { resource_key } => {
        code: "compiler.resource.resolution_failed",
        message_key: "diagnostics.compiler.resource.resolution_failed",
        severity: Error,
        en: "Resource {resource_key} could not be resolved.",
        zh: "无法解析资源 {resource_key}。",
    },
    FunctionAbiTargetMismatch { function_path } => {
        code: "compiler.function.abi_target_mismatch",
        message_key: "diagnostics.compiler.function.abi_target_mismatch",
        severity: Error,
        en: "Function ABI target does not match {function_path}.",
        zh: "函数 ABI 目标与 {function_path} 不匹配。",
    },
    InputConflictingBindings { port } => {
        code: "compiler.input.conflicting_bindings",
        message_key: "diagnostics.compiler.input.conflicting_bindings",
        severity: Error,
        en: "Input {port} has conflicting bindings.",
        zh: "输入 {port} 存在冲突绑定。",
    },
    InputLiteralForbidden { port } => {
        code: "compiler.input.literal_forbidden",
        message_key: "diagnostics.compiler.input.literal_forbidden",
        severity: Error,
        en: "Input {port} does not allow a literal binding.",
        zh: "输入 {port} 不允许字面量绑定。",
    },
    InputLiteralInvalid { port } => {
        code: "compiler.input.literal_invalid",
        message_key: "diagnostics.compiler.input.literal_invalid",
        severity: Error,
        en: "Input {port} has an invalid persisted literal.",
        zh: "输入 {port} 的持久化字面量无效。",
    },
    InputNotInput { port } => {
        code: "compiler.input.not_input",
        message_key: "diagnostics.compiler.input.not_input",
        severity: Error,
        en: "Port {port} is not an input.",
        zh: "端口 {port} 不是输入端口。",
    },
    InputUnbound { port } => {
        code: "compiler.input.unbound",
        message_key: "diagnostics.compiler.input.unbound",
        severity: Warning,
        en: "Required input {port} is unbound.",
        zh: "必需输入 {port} 尚未绑定。",
    },
    InputUnknownPort { port } => {
        code: "compiler.input.unknown_port",
        message_key: "diagnostics.compiler.input.unknown_port",
        severity: Error,
        en: "Input port {port} is unknown.",
        zh: "输入端口 {port} 未知。",
    },
    InterfaceBasisMismatch { expected_basis, actual_basis } => {
        code: "compiler.interface.basis_mismatch",
        message_key: "diagnostics.compiler.interface.basis_mismatch",
        severity: Error,
        en: "Interface basis {actual_basis} does not match {expected_basis}.",
        zh: "接口基准 {actual_basis} 与 {expected_basis} 不匹配。",
    },
    InterfaceDuplicateLocator { port_key, locator } => {
        code: "compiler.interface.duplicate_locator",
        message_key: "diagnostics.compiler.interface.duplicate_locator",
        severity: Error,
        en: "Interface locator {locator} for port {port_key} is duplicated.",
        zh: "端口 {port_key} 的接口定位器 {locator} 重复。",
    },
    InterfaceIdentityNoneConnection { port } => {
        code: "compiler.interface.identity_none_connection",
        message_key: "diagnostics.compiler.interface.identity_none_connection",
        severity: Error,
        en: "Identity-free interface port {port} cannot have a connection.",
        zh: "无标识接口端口 {port} 不能有连接。",
    },
    InterfaceIdentityNoneOverride { port } => {
        code: "compiler.interface.identity_none_override",
        message_key: "diagnostics.compiler.interface.identity_none_override",
        severity: Error,
        en: "Identity-free interface port {port} cannot have an override.",
        zh: "无标识接口端口 {port} 不能有覆盖。",
    },
    InterfaceSchemaDependencyUnresolved {} => {
        code: "compiler.interface.schema_dependency_unresolved",
        message_key: "diagnostics.compiler.interface.schema_dependency_unresolved",
        severity: Error,
        en: "Schema-dependent interface requirements could not be resolved.",
        zh: "无法解析依赖架构的接口要求。",
    },
    InterfaceResolverFailed { resolver_id } => {
        code: "compiler.interface.resolver_failed",
        message_key: "diagnostics.compiler.interface.resolver_failed",
        severity: Error,
        en: "Interface resolver {resolver_id} failed.",
        zh: "接口解析器 {resolver_id} 失败。",
    },
    InterfaceResolverMissing { resolver_id } => {
        code: "compiler.interface.resolver_missing",
        message_key: "diagnostics.compiler.interface.resolver_missing",
        severity: Error,
        en: "Interface resolver {resolver_id} is missing.",
        zh: "缺少接口解析器 {resolver_id}。",
    },
    LoweringDeadlineExceeded { node_type } => {
        code: "compiler.lowering.deadline_exceeded",
        message_key: "diagnostics.compiler.lowering.deadline_exceeded",
        severity: Error,
        en: "Node lowering exceeded its deadline for {node_type}.",
        zh: "节点类型 {node_type} 的降低超过截止时间。",
    },
    LoweringInternalInvariant { node_type } => {
        code: "compiler.lowering.internal_invariant",
        message_key: "diagnostics.compiler.lowering.internal_invariant",
        severity: Error,
        en: "Node lowering hit an internal invariant for {node_type}.",
        zh: "节点类型 {node_type} 的降低触发内部不变量。",
    },
    LoweringExecutionIdentity {} => {
        code: "compiler.lowering.execution_identity",
        message_key: "diagnostics.compiler.lowering.execution_identity",
        severity: Error,
        en: "Lowered operation has an invalid execution identity.",
        zh: "降低后的操作具有无效的执行身份。",
    },
    LoweringResourceExhausted { node_type } => {
        code: "compiler.lowering.resource_exhausted",
        message_key: "diagnostics.compiler.lowering.resource_exhausted",
        severity: Error,
        en: "Node lowering exhausted resources for {node_type}.",
        zh: "节点类型 {node_type} 的降低耗尽资源。",
    },
    LoweringImplementationMissing { node_type } => {
        code: "compiler.lowering.implementation_missing",
        message_key: "diagnostics.compiler.lowering.implementation_missing",
        severity: Error,
        en: "Lowering implementation is missing for {node_type}.",
        zh: "节点类型 {node_type} 缺少降低实现。",
    },
    LoweringResourceConflict { resource_id } => {
        code: "compiler.lowering.resource_conflict",
        message_key: "diagnostics.compiler.lowering.resource_conflict",
        severity: Error,
        en: "Lowered resource {resource_id} conflicts with another resource.",
        zh: "降低后的资源 {resource_id} 与其他资源冲突。",
    },

    LoweringResultDuplicate { result_name } => {
        code: "compiler.lowering.result_duplicate",
        message_key: "diagnostics.compiler.lowering.result_duplicate",
        severity: Error,
        en: "Lowering result {result_name} is duplicated.",
        zh: "降低结果 {result_name} 重复。",
    },
    LoweringResultPort { port } => {
        code: "compiler.lowering.result_port",
        message_key: "diagnostics.compiler.lowering.result_port",
        severity: Error,
        en: "Lowering result port {port} is invalid.",
        zh: "降低结果端口 {port} 无效。",
    },
    NodeDisappeared { node_type } => {
        code: "compiler.node.disappeared",
        message_key: "diagnostics.compiler.node.disappeared",
        severity: Error,
        en: "Node type {node_type} disappeared during compilation.",
        zh: "节点类型 {node_type} 在编译期间消失。",
    },
    NodeManagedSingleton { managed_role } => {
        code: "compiler.node.managed_singleton",
        message_key: "diagnostics.compiler.node.managed_singleton",
        severity: Error,
        en: "Managed role {managed_role} must identify exactly one node.",
        zh: "托管角色 {managed_role} 必须只标识一个节点。",
    },
    NodeScopeMismatch { expected_scope, actual_scope } => {
        code: "compiler.node.scope_mismatch",
        message_key: "diagnostics.compiler.node.scope_mismatch",
        severity: Error,
        en: "Node scope {actual_scope} does not match {expected_scope}.",
        zh: "节点作用域 {actual_scope} 与 {expected_scope} 不匹配。",
    },
    NodeUnknown { node_type } => {
        code: "compiler.node.unknown",
        message_key: "diagnostics.compiler.node.unknown",
        severity: Error,
        en: "Node type {node_type} is unknown.",
        zh: "节点类型 {node_type} 未知。",
    },
    ParameterInvalid { parameter_key } => {
        code: "compiler.parameter.invalid",
        message_key: "diagnostics.compiler.parameter.invalid",
        severity: Error,
        en: "Parameter {parameter_key} is invalid.",
        zh: "参数 {parameter_key} 无效。",
    },
    ParameterRequired { parameter_key } => {
        code: "compiler.parameter.required",
        message_key: "diagnostics.compiler.parameter.required",
        severity: Error,
        en: "Parameter {parameter_key} is required.",
        zh: "参数 {parameter_key} 是必需的。",
    },
    ParameterUnknown { parameter_key } => {
        code: "compiler.parameter.unknown",
        message_key: "diagnostics.compiler.parameter.unknown",
        severity: Error,
        en: "Parameter {parameter_key} is unknown.",
        zh: "参数 {parameter_key} 未知。",
    },
    PlanInvalid {} => {
        code: "compiler.plan.invalid",
        message_key: "diagnostics.compiler.plan.invalid",
        severity: Error,
        en: "Execution plan is invalid.",
        zh: "执行计划无效。",
    },
    PlanValueConsumerMissing { port } => {
        code: "compiler.plan.value_consumer_missing",
        message_key: "diagnostics.compiler.plan.value_consumer_missing",
        severity: Error,
        en: "Value consumer for {port} is missing.",
        zh: "缺少端口 {port} 的值消费者。",
    },
    PlanValueProducerMissing { port } => {
        code: "compiler.plan.value_producer_missing",
        message_key: "diagnostics.compiler.plan.value_producer_missing",
        severity: Error,
        en: "Value producer for {port} is missing.",
        zh: "缺少端口 {port} 的值生产者。",
    },
    PortBindingKindMismatch { expected_kind, actual_kind } => {
        code: "compiler.port.binding_kind_mismatch",
        message_key: "diagnostics.compiler.port.binding_kind_mismatch",
        severity: Error,
        en: "Port binding kind {actual_kind} does not match {expected_kind}.",
        zh: "端口绑定类型 {actual_kind} 与 {expected_kind} 不匹配。",
    },
    PortBindingNotInstance { port } => {
        code: "compiler.port.binding_not_instance",
        message_key: "diagnostics.compiler.port.binding_not_instance",
        severity: Error,
        en: "Port binding {port} does not identify an instance.",
        zh: "端口绑定 {port} 未标识实例。",
    },
    PortInstanceNotAllowed { port } => {
        code: "compiler.port.instance_not_allowed",
        message_key: "diagnostics.compiler.port.instance_not_allowed",
        severity: Error,
        en: "Port instance {port} is not allowed.",
        zh: "不允许端口实例 {port}。",
    },
    PortOrphan { port } => {
        code: "compiler.port.orphan",
        message_key: "diagnostics.compiler.port.orphan",
        severity: Error,
        en: "Port {port} is orphaned.",
        zh: "端口 {port} 已孤立。",
    },
    PortUnknown { port } => {
        code: "compiler.port.unknown",
        message_key: "diagnostics.compiler.port.unknown",
        severity: Error,
        en: "Port {port} is unknown.",
        zh: "端口 {port} 未知。",
    },
    RegistryTypeMismatch { expected_type, actual_type } => {
        code: "compiler.registry.type_mismatch",
        message_key: "diagnostics.compiler.registry.type_mismatch",
        severity: Error,
        en: "Registry type {actual_type} does not match {expected_type}.",
        zh: "注册表类型 {actual_type} 与 {expected_type} 不匹配。",
    },
    RelationalFilterColumnMissing { field_name } => {
        code: "compiler.relational.filter_column_missing",
        message_key: "diagnostics.compiler.relational.filter_column_missing",
        severity: Error,
        en: "Filter column {field_name} is missing.",
        zh: "缺少筛选列 {field_name}。",
    },
    RelationalFilterLiteralForbidden { field_name } => {
        code: "compiler.relational.filter_literal_forbidden",
        message_key: "diagnostics.compiler.relational.filter_literal_forbidden",
        severity: Error,
        en: "Filter field {field_name} forbids a literal.",
        zh: "筛选字段 {field_name} 不允许字面量。",
    },
    RelationalFilterLiteralMissing { field_name } => {
        code: "compiler.relational.filter_literal_missing",
        message_key: "diagnostics.compiler.relational.filter_literal_missing",
        severity: Error,
        en: "Filter field {field_name} requires a literal.",
        zh: "筛选字段 {field_name} 需要字面量。",
    },
    RelationalFilterLiteralType { field_name } => {
        code: "compiler.relational.filter_literal_type",
        message_key: "diagnostics.compiler.relational.filter_literal_type",
        severity: Error,
        en: "Filter literal type is invalid for {field_name}.",
        zh: "筛选字段 {field_name} 的字面量类型无效。",
    },
    RelationalFilterOperatorInvalid { field_name } => {
        code: "compiler.relational.filter_operator_invalid",
        message_key: "diagnostics.compiler.relational.filter_operator_invalid",
        severity: Error,
        en: "Filter operator is invalid for {field_name}.",
        zh: "筛选字段 {field_name} 的操作符无效。",
    },
    RelationalInputBindingMissing { port } => {
        code: "compiler.relational.input_binding_missing",
        message_key: "diagnostics.compiler.relational.input_binding_missing",
        severity: Error,
        en: "Relational input binding for {port} is missing.",
        zh: "缺少端口 {port} 的关系输入绑定。",
    },
    SchemaParameterInvalid { parameter_key } => {
        code: "compiler.schema.parameter_invalid",
        message_key: "diagnostics.compiler.schema.parameter_invalid",
        severity: Error,
        en: "Schema parameter {parameter_key} is invalid.",
        zh: "架构参数 {parameter_key} 无效。",
    },
    SchemaProjectEmpty {} => {
        code: "compiler.schema.project_empty",
        message_key: "diagnostics.compiler.schema.project_empty",
        severity: Error,
        en: "Schema projection cannot be empty.",
        zh: "架构投影不能为空。",
    },
    SchemaProjectFieldDuplicate { field_name } => {
        code: "compiler.schema.project_field_duplicate",
        message_key: "diagnostics.compiler.schema.project_field_duplicate",
        severity: Error,
        en: "Projected field {field_name} is duplicated.",
        zh: "投影字段 {field_name} 重复。",
    },
    SchemaProjectFieldMissing { field_name } => {
        code: "compiler.schema.project_field_missing",
        message_key: "diagnostics.compiler.schema.project_field_missing",
        severity: Error,
        en: "Projected field {field_name} is missing.",
        zh: "缺少投影字段 {field_name}。",
    },
    SchemaRenameFieldMissing { source_name } => {
        code: "compiler.schema.rename_field_missing",
        message_key: "diagnostics.compiler.schema.rename_field_missing",
        severity: Error,
        en: "Rename source field {source_name} is missing.",
        zh: "缺少重命名源字段 {source_name}。",
    },
    SchemaRenameSourceDuplicate { source_name } => {
        code: "compiler.schema.rename_source_duplicate",
        message_key: "diagnostics.compiler.schema.rename_source_duplicate",
        severity: Error,
        en: "Rename source {source_name} is duplicated.",
        zh: "重命名源 {source_name} 重复。",
    },
    SchemaRenameTargetConflict { source_name, target_name } => {
        code: "compiler.schema.rename_target_conflict",
        message_key: "diagnostics.compiler.schema.rename_target_conflict",
        severity: Error,
        en: "Renaming {source_name} to {target_name} conflicts with another field.",
        zh: "将 {source_name} 重命名为 {target_name} 时与其他字段冲突。",
    },
    SchemaResolverFailed { resolver_id } => {
        code: "compiler.schema.resolver_failed",
        message_key: "diagnostics.compiler.schema.resolver_failed",
        severity: Error,
        en: "Schema resolver {resolver_id} failed.",
        zh: "架构解析器 {resolver_id} 失败。",
    },
    SchemaResolverMissing { resolver_id } => {
        code: "compiler.schema.resolver_missing",
        message_key: "diagnostics.compiler.schema.resolver_missing",
        severity: Error,
        en: "Schema resolver {resolver_id} is missing.",
        zh: "缺少架构解析器 {resolver_id}。",
    },
    SemanticInvalid {} => {
        code: "compiler.semantic.invalid",
        message_key: "diagnostics.compiler.semantic.invalid",
        severity: Error,
        en: "Semantic graph is invalid.",
        zh: "语义图无效。",
    },
    TypeIncompatible { expected_type, actual_type } => {
        code: "compiler.type.incompatible",
        message_key: "diagnostics.compiler.type.incompatible",
        severity: Error,
        en: "Type {actual_type} is incompatible with {expected_type}.",
        zh: "类型 {actual_type} 与 {expected_type} 不兼容。",
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilerDiagnosticDefinitionError {
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

impl fmt::Display for CompilerDiagnosticDefinitionError {
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

impl Error for CompilerDiagnosticDefinitionError {}

pub fn validate_compiler_diagnostic_definitions(
    definitions: &[CompilerDiagnosticDefinition],
) -> Result<(), CompilerDiagnosticDefinitionError> {
    let mut codes = BTreeSet::new();
    let mut message_keys = BTreeSet::new();

    for definition in definitions {
        if !codes.insert(definition.code) {
            return Err(CompilerDiagnosticDefinitionError::DuplicateCode {
                code: definition.code.into(),
            });
        }
        if !message_keys.insert(definition.message_key) {
            return Err(CompilerDiagnosticDefinitionError::DuplicateMessageKey {
                message_key: definition.message_key.into(),
            });
        }
        if !definition
            .templates
            .iter()
            .any(|template| template.locale == "en-US")
        {
            return Err(CompilerDiagnosticDefinitionError::MissingDefaultTemplate {
                code: definition.code.into(),
                message_key: definition.message_key.into(),
            });
        }

        let declared = canonical_names(definition.argument_names.iter().copied());
        for template in definition.templates {
            let referenced = extract_placeholders(template.text).map_err(|error| match error {
                TemplatePlaceholderError::UnmatchedBrace { brace, offset } => {
                    CompilerDiagnosticDefinitionError::UnmatchedTemplateBrace {
                        code: definition.code.into(),
                        locale: template.locale.into(),
                        brace,
                        offset,
                    }
                }
                TemplatePlaceholderError::InvalidName { name } => {
                    CompilerDiagnosticDefinitionError::InvalidTemplatePlaceholder {
                        code: definition.code.into(),
                        locale: template.locale.into(),
                        name,
                    }
                }
            })?;
            if declared != referenced {
                return Err(
                    CompilerDiagnosticDefinitionError::ArgumentTemplateMismatch {
                        code: definition.code.into(),
                        locale: template.locale.into(),
                        declared,
                        referenced,
                    },
                );
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
    fn compiler_diagnostic_definitions_are_unique_template_safe_and_dataflow_only() {
        assert_eq!(COMPILER_DIAGNOSTIC_DEFINITIONS.len(), 73);
        validate_compiler_diagnostic_definitions(COMPILER_DIAGNOSTIC_DEFINITIONS).unwrap();

        assert!(COMPILER_DIAGNOSTIC_DEFINITIONS.iter().all(|definition| {
            !definition.code.contains(".control") && !definition.code.contains(".effect")
        }));

        let codes = COMPILER_DIAGNOSTIC_DEFINITIONS
            .iter()
            .map(|definition| definition.code)
            .collect::<BTreeSet<_>>();
        let message_keys = COMPILER_DIAGNOSTIC_DEFINITIONS
            .iter()
            .map(|definition| definition.message_key)
            .collect::<BTreeSet<_>>();
        assert_eq!(codes.len(), COMPILER_DIAGNOSTIC_DEFINITIONS.len());
        assert_eq!(message_keys.len(), COMPILER_DIAGNOSTIC_DEFINITIONS.len());
        assert!(COMPILER_DIAGNOSTIC_DEFINITIONS.iter().all(|definition| {
            definition
                .templates
                .iter()
                .any(|template| template.locale == "en-US")
        }));
    }

    fn test_definition(templates: &'static [DiagnosticTemplate]) -> CompilerDiagnosticDefinition {
        CompilerDiagnosticDefinition {
            code: "compiler.test.template",
            message_key: "diagnostics.compiler.test.template",
            default_severity: DiagnosticSeverity::Error,
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
            validate_compiler_diagnostic_definitions(&[test_definition(UNMATCHED_OPEN)]),
            Err(CompilerDiagnosticDefinitionError::UnmatchedTemplateBrace { brace: '{', .. })
        ));
        assert!(matches!(
            validate_compiler_diagnostic_definitions(&[test_definition(UNMATCHED_CLOSE)]),
            Err(CompilerDiagnosticDefinitionError::UnmatchedTemplateBrace { brace: '}', .. })
        ));
        assert!(matches!(
            validate_compiler_diagnostic_definitions(&[test_definition(INVALID_NAME)]),
            Err(CompilerDiagnosticDefinitionError::InvalidTemplatePlaceholder {
                name,
                ..
            }) if name.as_ref() == "Value"
        ));
    }
}
