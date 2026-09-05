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
    pub blocking: bool,
    pub argument_names: &'static [&'static str],
    pub templates: &'static [DiagnosticTemplate],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphDiagnosticKind {
    ConnectionInputDirection,
    ConnectionLimit,
    ConnectionOrderForbidden,
    ConnectionOrderRequired,
    ConnectionOutputDirection,
    DependencyValueCycle,
    FunctionAbiMismatch,
    FunctionBlocked,
    FunctionBodyUnavailable,
    FunctionDependencyCycle,
    InputConflictingBindings,
    InputLiteralForbidden,
    InputLiteralInvalid,
    InputNotInput,
    InputUnbound,
    InputUnknownPort,
    InterfaceSchemaDependencyUnresolved,
    NodeUnknown,
    ParameterInvalid,
    ParameterRequired,
    ParameterUnknown,
    PortBindingKindMismatch,
    PortOrphan,
    PortUnknown,
    ResourceResolutionFailed,
    SchemaParameterInvalid,
    SemanticInvalid,
    TypeConnectionMismatch,
    TypeGenericConflict,
    TypeInputNotAccepted,
    TypeResolutionIncomplete,
}

impl GraphDiagnosticKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ConnectionInputDirection => "compiler.connection.input_direction",
            Self::ConnectionLimit => "compiler.connection.limit",
            Self::ConnectionOrderForbidden => "compiler.connection.order_forbidden",
            Self::ConnectionOrderRequired => "compiler.connection.order_required",
            Self::ConnectionOutputDirection => "compiler.connection.output_direction",
            Self::DependencyValueCycle => "compiler.dependency.value_cycle",
            Self::FunctionAbiMismatch => "compiler.function.abi_mismatch",
            Self::FunctionBlocked => "compiler.function.blocked",
            Self::FunctionBodyUnavailable => "compiler.function.body_unavailable",
            Self::FunctionDependencyCycle => "compiler.function.dependency_cycle",
            Self::InputConflictingBindings => "compiler.input.conflicting_bindings",
            Self::InputLiteralForbidden => "compiler.input.literal_forbidden",
            Self::InputLiteralInvalid => "compiler.input.literal_invalid",
            Self::InputNotInput => "compiler.input.not_input",
            Self::InputUnbound => "compiler.input.unbound",
            Self::InputUnknownPort => "compiler.input.unknown_port",
            Self::InterfaceSchemaDependencyUnresolved => {
                "compiler.interface.schema_dependency_unresolved"
            }
            Self::NodeUnknown => "compiler.node.unknown",
            Self::ParameterInvalid => "compiler.parameter.invalid",
            Self::ParameterRequired => "compiler.parameter.required",
            Self::ParameterUnknown => "compiler.parameter.unknown",
            Self::PortBindingKindMismatch => "compiler.port.binding_kind_mismatch",
            Self::PortOrphan => "compiler.port.orphan",
            Self::PortUnknown => "compiler.port.unknown",
            Self::ResourceResolutionFailed => "compiler.resource.resolution_failed",
            Self::SchemaParameterInvalid => "compiler.schema.parameter_invalid",
            Self::SemanticInvalid => "compiler.semantic.invalid",
            Self::TypeConnectionMismatch => "compiler.type.connection_mismatch",
            Self::TypeGenericConflict => "compiler.type.generic_conflict",
            Self::TypeInputNotAccepted => "compiler.type.input_not_accepted",
            Self::TypeResolutionIncomplete => "compiler.type.resolution_incomplete",
        }
    }
    pub fn definition(self) -> &'static CompilerDiagnosticDefinition {
        COMPILER_DIAGNOSTIC_DEFINITIONS
            .iter()
            .find(|definition| definition.code == self.code())
            .expect("every graph diagnostic kind has a definition")
    }
    pub fn default_severity(self) -> DiagnosticSeverity {
        self.definition().default_severity
    }
}

macro_rules! define_compiler_diagnostics {
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
        pub const COMPILER_DIAGNOSTIC_DEFINITIONS: &[CompilerDiagnosticDefinition] = &[
            $(
                CompilerDiagnosticDefinition {
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

define_compiler_diagnostics! {
    DataframeFieldTypeUnsupported { column, schema_type, reason } => {
        code: "compiler.dataframe.field_type_unsupported",
        message_key: "diagnostics.compiler.dataframe.field_type_unsupported",
        severity: Warning,
        blocking: false,
        en: "Column {column} uses unsupported schema type {schema_type}: {reason}.",
        zh: "列 {column} 使用了不支持的 Schema 类型 {schema_type}：{reason}。",
    },
    ConnectionInputDirection { port } => {
        code: "compiler.connection.input_direction",
        message_key: "diagnostics.compiler.connection.input_direction",
        severity: Error,
        blocking: true,
        en: "Connection target {port} must be an input port.",
        zh: "连接目标 {port} 必须是输入端口。",
    },
    ConnectionLimit { port } => {
        code: "compiler.connection.limit",
        message_key: "diagnostics.compiler.connection.limit",
        severity: Error,
        blocking: true,
        en: "Connection limit exceeded for {port}.",
        zh: "端口 {port} 超出连接数量限制。",
    },
    ConnectionOrderForbidden { port } => {
        code: "compiler.connection.order_forbidden",
        message_key: "diagnostics.compiler.connection.order_forbidden",
        severity: Error,
        blocking: true,
        en: "Connection order is forbidden for {port}.",
        zh: "端口 {port} 不允许连接顺序。",
    },
    ConnectionOrderRequired { port } => {
        code: "compiler.connection.order_required",
        message_key: "diagnostics.compiler.connection.order_required",
        severity: Error,
        blocking: true,
        en: "Connection order is required for {port}.",
        zh: "端口 {port} 需要连接顺序。",
    },
    ConnectionOutputDirection { port } => {
        code: "compiler.connection.output_direction",
        message_key: "diagnostics.compiler.connection.output_direction",
        severity: Error,
        blocking: true,
        en: "Connection source {port} must be an output port.",
        zh: "连接源 {port} 必须是输出端口。",
    },
    DependencyValueCycle {} => {
        code: "compiler.dependency.value_cycle",
        message_key: "diagnostics.compiler.dependency.value_cycle",
        severity: Error,
        blocking: true,
        en: "Value dependencies contain a cycle.",
        zh: "值依赖包含循环。",
    },
    DocumentConnectionIdMismatch { expected_id, actual_id } => {
        code: "compiler.document.connection_id_mismatch",
        message_key: "diagnostics.compiler.document.connection_id_mismatch",
        severity: Error,
        blocking: true,
        en: "Connection ID {actual_id} does not match {expected_id}.",
        zh: "连接 ID {actual_id} 与 {expected_id} 不匹配。",
    },
    DocumentNodeIdMismatch { expected_id, actual_id } => {
        code: "compiler.document.node_id_mismatch",
        message_key: "diagnostics.compiler.document.node_id_mismatch",
        severity: Error,
        blocking: true,
        en: "Node ID {actual_id} does not match {expected_id}.",
        zh: "节点 ID {actual_id} 与 {expected_id} 不匹配。",
    },
    FunctionAbiEndpointInvalid { port } => {
        code: "compiler.function.abi.endpoint_invalid",
        message_key: "diagnostics.compiler.function.abi.endpoint_invalid",
        severity: Error,
        blocking: true,
        en: "Function ABI endpoint {port} is invalid.",
        zh: "函数 ABI 端点 {port} 无效。",
    },
    FunctionAbiLocatorInvalid { port } => {
        code: "compiler.function.abi.locator_invalid",
        message_key: "diagnostics.compiler.function.abi.locator_invalid",
        severity: Error,
        blocking: true,
        en: "Function ABI locator for {port} is invalid.",
        zh: "端口 {port} 的函数 ABI 定位器无效。",
    },
    FunctionAbiLocatorTargetMismatch { function_path } => {
        code: "compiler.function.abi.locator_target_mismatch",
        message_key: "diagnostics.compiler.function.abi.locator_target_mismatch",
        severity: Error,
        blocking: true,
        en: "Function ABI locator does not target {function_path}.",
        zh: "函数 ABI 定位器未指向 {function_path}。",
    },
    FunctionAbiManagedRoleInvalid { expected_role, actual_count } => {
        code: "compiler.function.abi.managed_role_invalid",
        message_key: "diagnostics.compiler.function.abi.managed_role_invalid",
        severity: Error,
        blocking: true,
        en: "Function ABI requires one {expected_role} node but found {actual_count}.",
        zh: "函数 ABI 需要一个 {expected_role} 节点，但找到 {actual_count} 个。",
    },
    FunctionAbiMemberDuplicate { field_name } => {
        code: "compiler.function.abi.member_duplicate",
        message_key: "diagnostics.compiler.function.abi.member_duplicate",
        severity: Error,
        blocking: true,
        en: "Function ABI member {field_name} is duplicated.",
        zh: "函数 ABI 成员 {field_name} 重复。",
    },
    FunctionAbiMemberMissing { field_name } => {
        code: "compiler.function.abi.member_missing",
        message_key: "diagnostics.compiler.function.abi.member_missing",
        severity: Error,
        blocking: true,
        en: "Function ABI member {field_name} is missing.",
        zh: "缺少函数 ABI 成员 {field_name}。",
    },
    FunctionAbiMemberUnexpected { field_name } => {
        code: "compiler.function.abi.member_unexpected",
        message_key: "diagnostics.compiler.function.abi.member_unexpected",
        severity: Error,
        blocking: true,
        en: "Function ABI member {field_name} is unexpected.",
        zh: "函数 ABI 成员 {field_name} 不符合预期。",
    },

    ResourceDisplayNameUnavailable { resource_key, reason } => {
        code: "compiler.resource.display_name_unavailable",
        message_key: "diagnostics.compiler.resource.display_name_unavailable",
        severity: Warning,
        blocking: false,
        en: "Resource {resource_key} uses the default node title: {reason}.",
        zh: "资源 {resource_key} 使用默认节点标题：{reason}。",
    },
    ResourceResolutionFailed { resource_key } => {
        code: "compiler.resource.resolution_failed",
        message_key: "diagnostics.compiler.resource.resolution_failed",
        severity: Error,
        blocking: true,
        en: "Resource {resource_key} could not be resolved.",
        zh: "无法解析资源 {resource_key}。",
    },
    FunctionAbiTargetMismatch { function_path } => {
        code: "compiler.function.abi_target_mismatch",
        message_key: "diagnostics.compiler.function.abi_target_mismatch",
        severity: Error,
        blocking: true,
        en: "Function ABI target does not match {function_path}.",
        zh: "函数 ABI 目标与 {function_path} 不匹配。",
    },
    InputConflictingBindings { port } => {
        code: "compiler.input.conflicting_bindings",
        message_key: "diagnostics.compiler.input.conflicting_bindings",
        severity: Error,
        blocking: true,
        en: "Input {port} has conflicting bindings.",
        zh: "输入 {port} 存在冲突绑定。",
    },
    InputLiteralForbidden { port } => {
        code: "compiler.input.literal_forbidden",
        message_key: "diagnostics.compiler.input.literal_forbidden",
        severity: Error,
        blocking: true,
        en: "Input {port} does not allow a literal binding.",
        zh: "输入 {port} 不允许字面量绑定。",
    },
    InputLiteralInvalid { port } => {
        code: "compiler.input.literal_invalid",
        message_key: "diagnostics.compiler.input.literal_invalid",
        severity: Error,
        blocking: true,
        en: "Input {port} has an invalid persisted literal.",
        zh: "输入 {port} 的持久化字面量无效。",
    },
    InputNotInput { port } => {
        code: "compiler.input.not_input",
        message_key: "diagnostics.compiler.input.not_input",
        severity: Error,
        blocking: true,
        en: "Port {port} is not an input.",
        zh: "端口 {port} 不是输入端口。",
    },
    InputUnbound { port } => {
        code: "compiler.input.unbound",
        message_key: "diagnostics.compiler.input.unbound",
        severity: Warning,
        blocking: true,
        en: "Required input {port} is unbound.",
        zh: "必需输入 {port} 尚未绑定。",
    },
    InputUnknownPort { port } => {
        code: "compiler.input.unknown_port",
        message_key: "diagnostics.compiler.input.unknown_port",
        severity: Error,
        blocking: true,
        en: "Input port {port} is unknown.",
        zh: "输入端口 {port} 未知。",
    },
    InterfaceBasisMismatch { expected_basis, actual_basis } => {
        code: "compiler.interface.basis_mismatch",
        message_key: "diagnostics.compiler.interface.basis_mismatch",
        severity: Error,
        blocking: true,
        en: "Interface basis {actual_basis} does not match {expected_basis}.",
        zh: "接口基准 {actual_basis} 与 {expected_basis} 不匹配。",
    },
    InterfaceDuplicateLocator { port_key, locator } => {
        code: "compiler.interface.duplicate_locator",
        message_key: "diagnostics.compiler.interface.duplicate_locator",
        severity: Error,
        blocking: true,
        en: "Interface locator {locator} for port {port_key} is duplicated.",
        zh: "端口 {port_key} 的接口定位器 {locator} 重复。",
    },
    InterfaceIdentityNoneConnection { port } => {
        code: "compiler.interface.identity_none_connection",
        message_key: "diagnostics.compiler.interface.identity_none_connection",
        severity: Error,
        blocking: true,
        en: "Identity-free interface port {port} cannot have a connection.",
        zh: "无标识接口端口 {port} 不能有连接。",
    },
    InterfaceIdentityNoneOverride { port } => {
        code: "compiler.interface.identity_none_override",
        message_key: "diagnostics.compiler.interface.identity_none_override",
        severity: Error,
        blocking: true,
        en: "Identity-free interface port {port} cannot have an override.",
        zh: "无标识接口端口 {port} 不能有覆盖。",
    },
    InterfaceSchemaDependencyUnresolved {} => {
        code: "compiler.interface.schema_dependency_unresolved",
        message_key: "diagnostics.compiler.interface.schema_dependency_unresolved",
        severity: Error,
        blocking: true,
        en: "Schema-dependent interface requirements could not be resolved.",
        zh: "无法解析依赖架构的接口要求。",
    },
    InterfaceResolverFailed { resolver_id } => {
        code: "compiler.interface.resolver_failed",
        message_key: "diagnostics.compiler.interface.resolver_failed",
        severity: Error,
        blocking: true,
        en: "Interface resolver {resolver_id} failed.",
        zh: "接口解析器 {resolver_id} 失败。",
    },
    InterfaceResolverMissing { resolver_id } => {
        code: "compiler.interface.resolver_missing",
        message_key: "diagnostics.compiler.interface.resolver_missing",
        severity: Error,
        blocking: true,
        en: "Interface resolver {resolver_id} is missing.",
        zh: "缺少接口解析器 {resolver_id}。",
    },
    LoweringDeadlineExceeded { node_type } => {
        code: "compiler.lowering.deadline_exceeded",
        message_key: "diagnostics.compiler.lowering.deadline_exceeded",
        severity: Error,
        blocking: true,
        en: "Node lowering exceeded its deadline for {node_type}.",
        zh: "节点类型 {node_type} 的降低超过截止时间。",
    },
    LoweringInternalInvariant { node_type } => {
        code: "compiler.lowering.internal_invariant",
        message_key: "diagnostics.compiler.lowering.internal_invariant",
        severity: Error,
        blocking: true,
        en: "Node lowering hit an internal invariant for {node_type}.",
        zh: "节点类型 {node_type} 的降低触发内部不变量。",
    },
    LoweringExecutionIdentity {} => {
        code: "compiler.lowering.execution_identity",
        message_key: "diagnostics.compiler.lowering.execution_identity",
        severity: Error,
        blocking: true,
        en: "Lowered operation has an invalid execution identity.",
        zh: "降低后的操作具有无效的执行身份。",
    },
    LoweringResourceExhausted { node_type } => {
        code: "compiler.lowering.resource_exhausted",
        message_key: "diagnostics.compiler.lowering.resource_exhausted",
        severity: Error,
        blocking: true,
        en: "Node lowering exhausted resources for {node_type}.",
        zh: "节点类型 {node_type} 的降低耗尽资源。",
    },
    LoweringImplementationMissing { node_type } => {
        code: "compiler.lowering.implementation_missing",
        message_key: "diagnostics.compiler.lowering.implementation_missing",
        severity: Error,
        blocking: true,
        en: "Lowering implementation is missing for {node_type}.",
        zh: "节点类型 {node_type} 缺少降低实现。",
    },
    LoweringResourceConflict { resource_id } => {
        code: "compiler.lowering.resource_conflict",
        message_key: "diagnostics.compiler.lowering.resource_conflict",
        severity: Error,
        blocking: true,
        en: "Lowered resource {resource_id} conflicts with another resource.",
        zh: "降低后的资源 {resource_id} 与其他资源冲突。",
    },

    LoweringResultDuplicate { result_name } => {
        code: "compiler.lowering.result_duplicate",
        message_key: "diagnostics.compiler.lowering.result_duplicate",
        severity: Error,
        blocking: true,
        en: "Lowering result {result_name} is duplicated.",
        zh: "降低结果 {result_name} 重复。",
    },
    LoweringResultPort { port } => {
        code: "compiler.lowering.result_port",
        message_key: "diagnostics.compiler.lowering.result_port",
        severity: Error,
        blocking: true,
        en: "Lowering result port {port} is invalid.",
        zh: "降低结果端口 {port} 无效。",
    },
    NodeDisappeared { node_type } => {
        code: "compiler.node.disappeared",
        message_key: "diagnostics.compiler.node.disappeared",
        severity: Error,
        blocking: true,
        en: "Node type {node_type} disappeared during compilation.",
        zh: "节点类型 {node_type} 在编译期间消失。",
    },
    NodeManagedSingleton { managed_role } => {
        code: "compiler.node.managed_singleton",
        message_key: "diagnostics.compiler.node.managed_singleton",
        severity: Error,
        blocking: true,
        en: "Managed role {managed_role} must identify exactly one node.",
        zh: "托管角色 {managed_role} 必须只标识一个节点。",
    },
    NodeScopeMismatch { expected_scope, actual_scope } => {
        code: "compiler.node.scope_mismatch",
        message_key: "diagnostics.compiler.node.scope_mismatch",
        severity: Error,
        blocking: true,
        en: "Node scope {actual_scope} does not match {expected_scope}.",
        zh: "节点作用域 {actual_scope} 与 {expected_scope} 不匹配。",
    },
    NodeUnknown { node_type } => {
        code: "compiler.node.unknown",
        message_key: "diagnostics.compiler.node.unknown",
        severity: Error,
        blocking: true,
        en: "Node type {node_type} is unknown.",
        zh: "节点类型 {node_type} 未知。",
    },
    ParameterInvalid { parameter_key } => {
        code: "compiler.parameter.invalid",
        message_key: "diagnostics.compiler.parameter.invalid",
        severity: Error,
        blocking: true,
        en: "Parameter {parameter_key} is invalid.",
        zh: "参数 {parameter_key} 无效。",
    },
    ParameterRequired { parameter_key } => {
        code: "compiler.parameter.required",
        message_key: "diagnostics.compiler.parameter.required",
        severity: Error,
        blocking: true,
        en: "Parameter {parameter_key} is required.",
        zh: "参数 {parameter_key} 是必需的。",
    },
    ParameterUnknown { parameter_key } => {
        code: "compiler.parameter.unknown",
        message_key: "diagnostics.compiler.parameter.unknown",
        severity: Error,
        blocking: true,
        en: "Parameter {parameter_key} is unknown.",
        zh: "参数 {parameter_key} 未知。",
    },
    PlanInvalid {} => {
        code: "compiler.plan.invalid",
        message_key: "diagnostics.compiler.plan.invalid",
        severity: Error,
        blocking: true,
        en: "Execution plan is invalid.",
        zh: "执行计划无效。",
    },
    PlanValueConsumerMissing { port } => {
        code: "compiler.plan.value_consumer_missing",
        message_key: "diagnostics.compiler.plan.value_consumer_missing",
        severity: Error,
        blocking: true,
        en: "Value consumer for {port} is missing.",
        zh: "缺少端口 {port} 的值消费者。",
    },
    PlanValueProducerMissing { port } => {
        code: "compiler.plan.value_producer_missing",
        message_key: "diagnostics.compiler.plan.value_producer_missing",
        severity: Error,
        blocking: true,
        en: "Value producer for {port} is missing.",
        zh: "缺少端口 {port} 的值生产者。",
    },
    PortBindingKindMismatch { expected_kind, actual_kind } => {
        code: "compiler.port.binding_kind_mismatch",
        message_key: "diagnostics.compiler.port.binding_kind_mismatch",
        severity: Error,
        blocking: true,
        en: "Port binding kind {actual_kind} does not match {expected_kind}.",
        zh: "端口绑定类型 {actual_kind} 与 {expected_kind} 不匹配。",
    },
    PortBindingNotInstance { port } => {
        code: "compiler.port.binding_not_instance",
        message_key: "diagnostics.compiler.port.binding_not_instance",
        severity: Error,
        blocking: true,
        en: "Port binding {port} does not identify an instance.",
        zh: "端口绑定 {port} 未标识实例。",
    },
    PortInstanceNotAllowed { port } => {
        code: "compiler.port.instance_not_allowed",
        message_key: "diagnostics.compiler.port.instance_not_allowed",
        severity: Error,
        blocking: true,
        en: "Port instance {port} is not allowed.",
        zh: "不允许端口实例 {port}。",
    },
    PortOrphan { port } => {
        code: "compiler.port.orphan",
        message_key: "diagnostics.compiler.port.orphan",
        severity: Error,
        blocking: true,
        en: "Port {port} is orphaned.",
        zh: "端口 {port} 已孤立。",
    },
    PortUnknown { port } => {
        code: "compiler.port.unknown",
        message_key: "diagnostics.compiler.port.unknown",
        severity: Error,
        blocking: true,
        en: "Port {port} is unknown.",
        zh: "端口 {port} 未知。",
    },
    RegistryTypeMismatch { expected_type, actual_type } => {
        code: "compiler.registry.type_mismatch",
        message_key: "diagnostics.compiler.registry.type_mismatch",
        severity: Error,
        blocking: true,
        en: "Registry type {actual_type} does not match {expected_type}.",
        zh: "注册表类型 {actual_type} 与 {expected_type} 不匹配。",
    },
    RelationalFilterColumnMissing { field_name } => {
        code: "compiler.relational.filter_column_missing",
        message_key: "diagnostics.compiler.relational.filter_column_missing",
        severity: Error,
        blocking: true,
        en: "Filter column {field_name} is missing.",
        zh: "缺少筛选列 {field_name}。",
    },
    RelationalFilterLiteralForbidden { field_name } => {
        code: "compiler.relational.filter_literal_forbidden",
        message_key: "diagnostics.compiler.relational.filter_literal_forbidden",
        severity: Error,
        blocking: true,
        en: "Filter field {field_name} forbids a literal.",
        zh: "筛选字段 {field_name} 不允许字面量。",
    },
    RelationalFilterLiteralMissing { field_name } => {
        code: "compiler.relational.filter_literal_missing",
        message_key: "diagnostics.compiler.relational.filter_literal_missing",
        severity: Error,
        blocking: true,
        en: "Filter field {field_name} requires a literal.",
        zh: "筛选字段 {field_name} 需要字面量。",
    },
    RelationalFilterLiteralType { field_name } => {
        code: "compiler.relational.filter_literal_type",
        message_key: "diagnostics.compiler.relational.filter_literal_type",
        severity: Error,
        blocking: true,
        en: "Filter literal type is invalid for {field_name}.",
        zh: "筛选字段 {field_name} 的字面量类型无效。",
    },
    RelationalFilterOperatorInvalid { field_name } => {
        code: "compiler.relational.filter_operator_invalid",
        message_key: "diagnostics.compiler.relational.filter_operator_invalid",
        severity: Error,
        blocking: true,
        en: "Filter operator is invalid for {field_name}.",
        zh: "筛选字段 {field_name} 的操作符无效。",
    },
    RelationalInputBindingMissing { port } => {
        code: "compiler.relational.input_binding_missing",
        message_key: "diagnostics.compiler.relational.input_binding_missing",
        severity: Error,
        blocking: true,
        en: "Relational input binding for {port} is missing.",
        zh: "缺少端口 {port} 的关系输入绑定。",
    },
    SchemaParameterInvalid { parameter_key } => {
        code: "compiler.schema.parameter_invalid",
        message_key: "diagnostics.compiler.schema.parameter_invalid",
        severity: Error,
        blocking: true,
        en: "Schema parameter {parameter_key} is invalid.",
        zh: "架构参数 {parameter_key} 无效。",
    },
    SchemaProjectEmpty {} => {
        code: "compiler.schema.project_empty",
        message_key: "diagnostics.compiler.schema.project_empty",
        severity: Error,
        blocking: true,
        en: "Schema projection cannot be empty.",
        zh: "架构投影不能为空。",
    },
    SchemaProjectFieldDuplicate { field_name } => {
        code: "compiler.schema.project_field_duplicate",
        message_key: "diagnostics.compiler.schema.project_field_duplicate",
        severity: Error,
        blocking: true,
        en: "Projected field {field_name} is duplicated.",
        zh: "投影字段 {field_name} 重复。",
    },
    SchemaProjectFieldMissing { field_name } => {
        code: "compiler.schema.project_field_missing",
        message_key: "diagnostics.compiler.schema.project_field_missing",
        severity: Error,
        blocking: true,
        en: "Projected field {field_name} is missing.",
        zh: "缺少投影字段 {field_name}。",
    },
    SchemaRenameFieldMissing { source_name } => {
        code: "compiler.schema.rename_field_missing",
        message_key: "diagnostics.compiler.schema.rename_field_missing",
        severity: Error,
        blocking: true,
        en: "Rename source field {source_name} is missing.",
        zh: "缺少重命名源字段 {source_name}。",
    },
    SchemaRenameSourceDuplicate { source_name } => {
        code: "compiler.schema.rename_source_duplicate",
        message_key: "diagnostics.compiler.schema.rename_source_duplicate",
        severity: Error,
        blocking: true,
        en: "Rename source {source_name} is duplicated.",
        zh: "重命名源 {source_name} 重复。",
    },
    SchemaRenameTargetConflict { source_name, target_name } => {
        code: "compiler.schema.rename_target_conflict",
        message_key: "diagnostics.compiler.schema.rename_target_conflict",
        severity: Error,
        blocking: true,
        en: "Renaming {source_name} to {target_name} conflicts with another field.",
        zh: "将 {source_name} 重命名为 {target_name} 时与其他字段冲突。",
    },
    SchemaResolverFailed { resolver_id } => {
        code: "compiler.schema.resolver_failed",
        message_key: "diagnostics.compiler.schema.resolver_failed",
        severity: Error,
        blocking: true,
        en: "Schema resolver {resolver_id} failed.",
        zh: "架构解析器 {resolver_id} 失败。",
    },
    SchemaResolverMissing { resolver_id } => {
        code: "compiler.schema.resolver_missing",
        message_key: "diagnostics.compiler.schema.resolver_missing",
        severity: Error,
        blocking: true,
        en: "Schema resolver {resolver_id} is missing.",
        zh: "缺少架构解析器 {resolver_id}。",
    },
    SemanticInvalid {} => {
        code: "compiler.semantic.invalid",
        message_key: "diagnostics.compiler.semantic.invalid",
        severity: Error,
        blocking: true,
        en: "Semantic graph is invalid.",
        zh: "语义图无效。",
    },
    TypeConnectionMismatch { output, input } => {
        code: "compiler.type.connection_mismatch",
        message_key: "diagnostics.compiler.type.connection_mismatch",
        severity: Error,
        blocking: true,
        en: "Resolved output type at {output} is not accepted by {input}.",
        zh: "输出端口 {output} 的已解析类型不被输入端口 {input} 接受。",
    },
    TypeGenericConflict { type_parameter } => {
        code: "compiler.type.generic_conflict",
        message_key: "diagnostics.compiler.type.generic_conflict",
        severity: Error,
        blocking: true,
        en: "Type parameter {type_parameter} received incompatible input types.",
        zh: "类型参数 {type_parameter} 收到了不兼容的输入类型。",
    },
    TypeInputNotAccepted { port } => {
        code: "compiler.type.input_not_accepted",
        message_key: "diagnostics.compiler.type.input_not_accepted",
        severity: Error,
        blocking: true,
        en: "Input value type is not accepted by port {port}.",
        zh: "输入值类型不被端口 {port} 接受。",
    },
    TypeResolutionIncomplete { port } => {
        code: "compiler.type.resolution_incomplete",
        message_key: "diagnostics.compiler.type.resolution_incomplete",
        severity: Error,
        blocking: true,
        en: "Port {port} does not have one exact resolved type.",
        zh: "端口 {port} 尚未求解为唯一确定类型。",
    },
    TypeIncompatible { expected_type, actual_type } => {
        code: "compiler.type.incompatible",
        message_key: "diagnostics.compiler.type.incompatible",
        severity: Error,
        blocking: true,
        en: "Type {actual_type} is incompatible with {expected_type}.",
        zh: "类型 {actual_type} 与 {expected_type} 不兼容。",
    },
    FunctionBodyUnavailable { function } => {
        code: "compiler.function.body_unavailable",
        message_key: "diagnostics.compiler.function.body_unavailable",
        severity: Error,
        blocking: true,
        en: "Function {function} has no available body.",
        zh: "函数 {function} 的正文不可用。",
    },
    FunctionAbiMismatch { function } => {
        code: "compiler.function.abi_mismatch",
        message_key: "diagnostics.compiler.function.abi_mismatch",
        severity: Error,
        blocking: true,
        en: "Function {function} entry or return does not match its signature.",
        zh: "函数 {function} 的入口或返回与签名不一致。",
    },
    FunctionDependencyCycle { function } => {
        code: "compiler.function.dependency_cycle",
        message_key: "diagnostics.compiler.function.dependency_cycle",
        severity: Error,
        blocking: true,
        en: "Function {function} participates in a recursive call cycle.",
        zh: "函数 {function} 参与了递归调用循环。",
    },
    FunctionBlocked { function } => {
        code: "compiler.function.blocked",
        message_key: "diagnostics.compiler.function.blocked",
        severity: Error,
        blocking: true,
        en: "Function {function} contains blocking graph problems.",
        zh: "函数 {function} 中存在阻止编译的图问题。",
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
        assert!(!COMPILER_DIAGNOSTIC_DEFINITIONS.is_empty());
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
        for kind in [
            GraphDiagnosticKind::DependencyValueCycle,
            GraphDiagnosticKind::InputUnbound,
            GraphDiagnosticKind::InterfaceSchemaDependencyUnresolved,
            GraphDiagnosticKind::NodeUnknown,
            GraphDiagnosticKind::ParameterInvalid,
            GraphDiagnosticKind::ParameterRequired,
            GraphDiagnosticKind::ParameterUnknown,
            GraphDiagnosticKind::PortBindingKindMismatch,
            GraphDiagnosticKind::PortOrphan,
            GraphDiagnosticKind::PortUnknown,
            GraphDiagnosticKind::ResourceResolutionFailed,
            GraphDiagnosticKind::SemanticInvalid,
            GraphDiagnosticKind::TypeConnectionMismatch,
            GraphDiagnosticKind::TypeGenericConflict,
            GraphDiagnosticKind::TypeInputNotAccepted,
            GraphDiagnosticKind::TypeResolutionIncomplete,
        ] {
            let definition = COMPILER_DIAGNOSTIC_DEFINITIONS
                .iter()
                .find(|definition| definition.code == kind.code())
                .expect("projection diagnostic kind has one authoritative definition");
            assert_eq!(definition.default_severity, kind.default_severity());
        }
    }

    fn test_definition(templates: &'static [DiagnosticTemplate]) -> CompilerDiagnosticDefinition {
        CompilerDiagnosticDefinition {
            code: "compiler.test.template",
            message_key: "diagnostics.compiler.test.template",
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
