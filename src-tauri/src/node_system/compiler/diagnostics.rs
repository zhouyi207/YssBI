use crate::node_system::analysis::{
    DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity, NodeDiagnostic,
};
use crate::node_system::document::{ConnectionId, NodeId, PortAddress};
use crate::node_system::protocol::{I18nKey, ManagedNodeRole, NodeScope, PortKind};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

pub(crate) type CompilerDiagnosticLocation =
    DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>;

pub(crate) type CompilerNodeDiagnostic =
    NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiagnosticTemplate {
    pub locale: &'static str,
    pub text: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompilerDiagnosticDefinition {
    pub code: &'static str,
    pub message_key: &'static str,
    pub default_severity: DiagnosticSeverity,
    pub argument_names: &'static [&'static str],
    pub templates: &'static [DiagnosticTemplate],
}

macro_rules! define_compiler_diagnostics {
    (
        $(
            $variant:ident { $($argument:ident),* $(,)? } => {
                code: $code:literal,
                message_key: $message_key:literal,
                severity: $severity:ident,
                en: $en:literal,
                zh: $zh:literal $(,)?
            }
        ),* $(,)?
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum CompilerDiagnostic {
            $(
                $variant { $($argument: Box<str>),* },
            )*
        }

        pub(crate) const COMPILER_DIAGNOSTIC_DEFINITIONS: &[CompilerDiagnosticDefinition] = &[
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

        impl CompilerDiagnostic {
            fn code(&self) -> &'static str {
                match self {
                    $(Self::$variant { .. } => $code,)*
                }
            }

            pub(crate) fn definition(&self) -> &'static CompilerDiagnosticDefinition {
                let code = self.code();
                COMPILER_DIAGNOSTIC_DEFINITIONS
                    .iter()
                    .find(|definition| definition.code == code)
                    .expect("compiler diagnostic variant has a generated definition")
            }

            fn into_arguments(self) -> DiagnosticArguments {
                match self {
                    $(
                        Self::$variant { $($argument),* } => {
                            #[allow(unused_mut)]
                            let mut arguments = DiagnosticArguments::new();
                            $(
                                arguments.insert(
                                    Box::from(stringify!($argument)),
                                    $argument,
                                );
                            )*
                            arguments
                        }
                    ),*
                }
            }

            pub(crate) fn into_node(
                self,
                primary: CompilerDiagnosticLocation,
            ) -> CompilerNodeDiagnostic {
                self.into_node_with_related(
                    primary,
                    Box::<[CompilerDiagnosticLocation]>::default(),
                )
            }

            pub(crate) fn into_node_with_related(
                self,
                primary: CompilerDiagnosticLocation,
                related: impl Into<Box<[CompilerDiagnosticLocation]>>,
            ) -> CompilerNodeDiagnostic {
                let definition = *self.definition();
                let arguments = self.into_arguments();
                let mut related = related.into().into_vec();
                related.sort_by(compare_locations);
                CompilerNodeDiagnostic {
                    code: DiagnosticCode::new(definition.code),
                    message_key: I18nKey::new(definition.message_key)
                        .expect("generated compiler diagnostic key is valid"),
                    arguments,
                    severity: definition.default_severity,
                    primary,
                    related: related.into_boxed_slice(),
                }
            }
        }
    };
}

define_compiler_diagnostics! {
    ConnectionInputDirection { port } => {
        code: "compiler.connection.input_direction",
        message_key: "diagnostics.compiler.connection.input_direction",
        severity: Error,
        en: "Connection target {port} must be an input port.",
        zh: "连接目标 {port} 必须是输入端口。",
    },
    ConnectionKindMismatch { source_kind, target_kind } => {
        code: "compiler.connection.kind_mismatch",
        message_key: "diagnostics.compiler.connection.kind_mismatch",
        severity: Error,
        en: "Connection kind {source_kind} is incompatible with {target_kind}.",
        zh: "连接类型 {source_kind} 与 {target_kind} 不兼容。",
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
    ControlAmbiguousOutput { port } => {
        code: "compiler.control.ambiguous_output",
        message_key: "diagnostics.compiler.control.ambiguous_output",
        severity: Error,
        en: "Control output {port} has multiple successors.",
        zh: "控制输出 {port} 有多个后继。",
    },
    ControlBranchContinuationAmbiguous {} => {
        code: "compiler.control.branch.continuation_ambiguous",
        message_key: "diagnostics.compiler.control.branch.continuation_ambiguous",
        severity: Error,
        en: "Branch continuation is ambiguous.",
        zh: "分支延续路径不明确。",
    },
    ControlCallAbiInvalid { function_path } => {
        code: "compiler.control.call.abi_invalid",
        message_key: "diagnostics.compiler.control.call.abi_invalid",
        severity: Error,
        en: "Function ABI is invalid for {function_path}.",
        zh: "函数 {function_path} 的 ABI 无效。",
    },
    ControlCallAbiMemberMissing { field_name } => {
        code: "compiler.control.call.abi_member_missing",
        message_key: "diagnostics.compiler.control.call.abi_member_missing",
        severity: Error,
        en: "Function ABI member {field_name} is missing.",
        zh: "缺少函数 ABI 成员 {field_name}。",
    },
    ControlCallAbiMissing { function_path } => {
        code: "compiler.control.call.abi_missing",
        message_key: "diagnostics.compiler.control.call.abi_missing",
        severity: Error,
        en: "Function ABI is missing for {function_path}.",
        zh: "函数 {function_path} 缺少 ABI。",
    },
    ControlCallEndpointInvalid { port } => {
        code: "compiler.control.call.endpoint_invalid",
        message_key: "diagnostics.compiler.control.call.endpoint_invalid",
        severity: Error,
        en: "Call endpoint {port} is invalid.",
        zh: "调用端点 {port} 无效。",
    },
    ControlCallLocatorDuplicate { function_path, parameter_id, port } => {
        code: "compiler.control.call.locator_duplicate",
        message_key: "diagnostics.compiler.control.call.locator_duplicate",
        severity: Error,
        en: "Call locator for parameter {parameter_id} in {function_path} is duplicated at {port}.",
        zh: "函数 {function_path} 的参数 {parameter_id} 调用定位器在端口 {port} 重复。",
    },
    ControlCallLocatorInvalid { port } => {
        code: "compiler.control.call.locator_invalid",
        message_key: "diagnostics.compiler.control.call.locator_invalid",
        severity: Error,
        en: "Call locator for {port} is invalid.",
        zh: "端口 {port} 的调用定位器无效。",
    },
    ControlCallLocatorTargetMismatch { function_path } => {
        code: "compiler.control.call.locator_target_mismatch",
        message_key: "diagnostics.compiler.control.call.locator_target_mismatch",
        severity: Error,
        en: "Call locator does not target {function_path}.",
        zh: "调用定位器未指向 {function_path}。",
    },
    ControlCallMemberMissing { member_role, member_id } => {
        code: "compiler.control.call.member_missing",
        message_key: "diagnostics.compiler.control.call.member_missing",
        severity: Error,
        en: "Call {member_role} member {member_id} is missing.",
        zh: "缺少调用{member_role}成员 {member_id}。",
    },
    ControlCallMemberUnexpected { member_role, member_id } => {
        code: "compiler.control.call.member_unexpected",
        message_key: "diagnostics.compiler.control.call.member_unexpected",
        severity: Error,
        en: "Call {member_role} member {member_id} is unexpected.",
        zh: "调用{member_role}成员 {member_id} 不符合预期。",
    },
    ControlCallResourceParameterMissing { parameter_key } => {
        code: "compiler.control.call.resource_parameter_missing",
        message_key: "diagnostics.compiler.control.call.resource_parameter_missing",
        severity: Error,
        en: "Call resource parameter {parameter_key} is missing.",
        zh: "缺少调用资源参数 {parameter_key}。",
    },
    ControlCallTargetInvalid { function_path } => {
        code: "compiler.control.call.target_invalid",
        message_key: "diagnostics.compiler.control.call.target_invalid",
        severity: Error,
        en: "Call target {function_path} is invalid.",
        zh: "调用目标 {function_path} 无效。",
    },
    ControlCallValueMissing { port } => {
        code: "compiler.control.call.value_missing",
        message_key: "diagnostics.compiler.control.call.value_missing",
        severity: Error,
        en: "Call value for {port} is missing.",
        zh: "缺少端口 {port} 的调用值。",
    },
    ControlControlPortRequired { port_key, expected_direction } => {
        code: "compiler.control.control_port_required",
        message_key: "diagnostics.compiler.control.control_port_required",
        severity: Error,
        en: "Control port {port_key} with direction {expected_direction} is required.",
        zh: "需要方向为 {expected_direction} 的控制端口 {port_key}。",
    },
    ControlCycle {} => {
        code: "compiler.control.cycle",
        message_key: "diagnostics.compiler.control.cycle",
        severity: Error,
        en: "Control flow contains an unsupported cycle.",
        zh: "控制流包含不支持的循环。",
    },
    ControlDataPortRequired { port_key, expected_direction } => {
        code: "compiler.control.data_port_required",
        message_key: "diagnostics.compiler.control.data_port_required",
        severity: Error,
        en: "Data port {port_key} with direction {expected_direction} is required.",
        zh: "需要方向为 {expected_direction} 的数据端口 {port_key}。",
    },
    ControlEntryOutputRequired {} => {
        code: "compiler.control.entry.output_required",
        message_key: "diagnostics.compiler.control.entry.output_required",
        severity: Error,
        en: "Entry node requires a control output.",
        zh: "入口节点需要控制输出。",
    },
    ControlLeafWithoutOperation {} => {
        code: "compiler.control.leaf_without_operation",
        message_key: "diagnostics.compiler.control.leaf_without_operation",
        severity: Error,
        en: "Control leaf has no operation.",
        zh: "控制叶节点没有操作。",
    },
    ControlLoopMaxIterationsRequired { parameter_key } => {
        code: "compiler.control.loop.max_iterations_required",
        message_key: "diagnostics.compiler.control.loop.max_iterations_required",
        severity: Error,
        en: "Loop requires a positive {parameter_key} value.",
        zh: "循环需要正数参数 {parameter_key}。",
    },
    ControlManagedRoleMismatch { expected_role, actual_role } => {
        code: "compiler.control.managed_role_mismatch",
        message_key: "diagnostics.compiler.control.managed_role_mismatch",
        severity: Error,
        en: "Managed role {actual_role} does not match {expected_role}.",
        zh: "托管角色 {actual_role} 与 {expected_role} 不匹配。",
    },
    ControlMemberGroupAmbiguous { field_name } => {
        code: "compiler.control.member_group_ambiguous",
        message_key: "diagnostics.compiler.control.member_group_ambiguous",
        severity: Error,
        en: "Member group {field_name} is ambiguous.",
        zh: "成员组 {field_name} 不明确。",
    },
    ControlMemberGroupCountInvalid { field_name } => {
        code: "compiler.control.member_group_count_invalid",
        message_key: "diagnostics.compiler.control.member_group_count_invalid",
        severity: Error,
        en: "Member group {field_name} has an invalid member count.",
        zh: "成员组 {field_name} 的成员数量无效。",
    },
    ControlMemberGroupDirectionInvalid { field_name } => {
        code: "compiler.control.member_group_direction_invalid",
        message_key: "diagnostics.compiler.control.member_group_direction_invalid",
        severity: Error,
        en: "Member group {field_name} has an invalid direction.",
        zh: "成员组 {field_name} 的方向无效。",
    },
    ControlMemberGroupIdentityAmbiguous { field_name } => {
        code: "compiler.control.member_group_identity_ambiguous",
        message_key: "diagnostics.compiler.control.member_group_identity_ambiguous",
        severity: Error,
        en: "Member group identity {field_name} is ambiguous.",
        zh: "成员组标识 {field_name} 不明确。",
    },
    ControlMemberGroupIncomplete { field_name } => {
        code: "compiler.control.member_group_incomplete",
        message_key: "diagnostics.compiler.control.member_group_incomplete",
        severity: Error,
        en: "Member group {field_name} is incomplete.",
        zh: "成员组 {field_name} 不完整。",
    },
    ControlMemberGroupMissing { field_name } => {
        code: "compiler.control.member_group_missing",
        message_key: "diagnostics.compiler.control.member_group_missing",
        severity: Error,
        en: "Member group {field_name} is missing.",
        zh: "缺少成员组 {field_name}。",
    },
    ControlNoEntry {} => {
        code: "compiler.control.no_entry",
        message_key: "diagnostics.compiler.control.no_entry",
        severity: Error,
        en: "Control graph has no entry.",
        zh: "控制图没有入口。",
    },
    ControlReturnInputRequired {} => {
        code: "compiler.control.return.input_required",
        message_key: "diagnostics.compiler.control.return.input_required",
        severity: Error,
        en: "Function return requires a control input.",
        zh: "函数返回节点需要控制输入。",
    },
    ControlReturnHasSuccessor {} => {
        code: "compiler.control.return_has_successor",
        message_key: "diagnostics.compiler.control.return_has_successor",
        severity: Error,
        en: "Function return cannot have a successor.",
        zh: "函数返回节点不能有后继。",
    },
    ControlSharedRegion {} => {
        code: "compiler.control.shared_region",
        message_key: "diagnostics.compiler.control.shared_region",
        severity: Error,
        en: "A node cannot belong to multiple control regions.",
        zh: "节点不能属于多个控制区域。",
    },
    ControlUnreachable {} => {
        code: "compiler.control.unreachable",
        message_key: "diagnostics.compiler.control.unreachable",
        severity: Error,
        en: "Control node is unreachable.",
        zh: "控制节点不可达。",
    },
    ControlUnstructuredContinuation {} => {
        code: "compiler.control.unstructured_continuation",
        message_key: "diagnostics.compiler.control.unstructured_continuation",
        severity: Error,
        en: "Control continuation is not structured.",
        zh: "控制延续路径不是结构化的。",
    },
    ControlValueMissing { port } => {
        code: "compiler.control.value_missing",
        message_key: "diagnostics.compiler.control.value_missing",
        severity: Error,
        en: "Control value for {port} is missing.",
        zh: "缺少端口 {port} 的控制值。",
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

    ResourceResolutionFailed { resource_key, reason } => {
        code: "compiler.resource.resolution_failed",
        message_key: "diagnostics.compiler.resource.resolution_failed",
        severity: Error,
        en: "Resource {resource_key} could not be resolved: {reason}.",
        zh: "无法解析资源 {resource_key}：{reason}。",
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
        severity: Error,
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
    LoweringEffectContract {} => {
        code: "compiler.lowering.effect_contract",
        message_key: "diagnostics.compiler.lowering.effect_contract",
        severity: Error,
        en: "Lowered operation violates its effect contract.",
        zh: "降低后的操作违反其效果契约。",
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
    PlanEffectConsumerMissing { port } => {
        code: "compiler.plan.effect_consumer_missing",
        message_key: "diagnostics.compiler.plan.effect_consumer_missing",
        severity: Error,
        en: "Effect consumer for {port} is missing.",
        zh: "缺少端口 {port} 的效果消费者。",
    },
    PlanEffectProducerMissing { port } => {
        code: "compiler.plan.effect_producer_missing",
        message_key: "diagnostics.compiler.plan.effect_producer_missing",
        severity: Error,
        en: "Effect producer for {port} is missing.",
        zh: "缺少端口 {port} 的效果生产者。",
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

pub(crate) fn validate_compiler_diagnostic_definitions(
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

pub(crate) const fn managed_node_role_name(role: Option<ManagedNodeRole>) -> &'static str {
    match role {
        Some(ManagedNodeRole::EventBegin) => "event_begin",
        Some(ManagedNodeRole::FunctionEntry) => "function_entry",
        Some(ManagedNodeRole::FunctionReturn) => "function_return",
        None => "none",
    }
}

pub(crate) const fn node_scope_name(scope: NodeScope) -> &'static str {
    match scope {
        NodeScope::Any => "any",
        NodeScope::Event => "event",
        NodeScope::Function => "function",
    }
}

pub(crate) const fn port_kind_name(kind: PortKind) -> &'static str {
    match kind {
        PortKind::Data => "data",
        PortKind::Control => "control",
        PortKind::Effect => "effect",
    }
}

pub(crate) fn compare_diagnostics(
    left: &CompilerNodeDiagnostic,
    right: &CompilerNodeDiagnostic,
) -> Ordering {
    compare_locations(&left.primary, &right.primary)
        .then_with(|| left.code.cmp(&right.code))
        .then_with(|| left.arguments.cmp(&right.arguments))
        .then_with(|| compare_related_locations(&left.related, &right.related))
}

fn compare_related_locations(
    left: &[CompilerDiagnosticLocation],
    right: &[CompilerDiagnosticLocation],
) -> Ordering {
    let mut left = left.iter().collect::<Vec<_>>();
    let mut right = right.iter().collect::<Vec<_>>();
    left.sort_by(|left, right| compare_locations(left, right));
    right.sort_by(|left, right| compare_locations(left, right));

    left.iter()
        .zip(&right)
        .find_map(|(left, right)| {
            let ordering = compare_locations(left, right);
            (ordering != Ordering::Equal).then_some(ordering)
        })
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn compare_locations(
    left: &CompilerDiagnosticLocation,
    right: &CompilerDiagnosticLocation,
) -> Ordering {
    let rank = |location: &CompilerDiagnosticLocation| match location {
        DiagnosticLocation::Graph => 0,
        DiagnosticLocation::Node(_) => 1,
        DiagnosticLocation::Port(_) => 2,
        DiagnosticLocation::Connection(_) => 3,
        DiagnosticLocation::Parameter { .. } => 4,
        DiagnosticLocation::Resource(_) => 5,
    };

    rank(left)
        .cmp(&rank(right))
        .then_with(|| match (left, right) {
            (DiagnosticLocation::Graph, DiagnosticLocation::Graph) => Ordering::Equal,
            (DiagnosticLocation::Node(left), DiagnosticLocation::Node(right)) => left.cmp(right),
            (DiagnosticLocation::Port(left), DiagnosticLocation::Port(right)) => left.cmp(right),
            (DiagnosticLocation::Connection(left), DiagnosticLocation::Connection(right)) => {
                left.cmp(right)
            }
            (
                DiagnosticLocation::Parameter {
                    node_id: left_node,
                    key: left_key,
                },
                DiagnosticLocation::Parameter {
                    node_id: right_node,
                    key: right_key,
                },
            ) => left_node
                .cmp(right_node)
                .then_with(|| left_key.cmp(right_key)),
            (DiagnosticLocation::Resource(left), DiagnosticLocation::Resource(right)) => {
                left.cmp(right)
            }
            _ => Ordering::Equal,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use uuid::Uuid;

    #[test]
    fn compiler_diagnostic_definitions_are_unique_and_template_safe() {
        assert_eq!(COMPILER_DIAGNOSTIC_DEFINITIONS.len(), 109);
        validate_compiler_diagnostic_definitions(COMPILER_DIAGNOSTIC_DEFINITIONS).unwrap();

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
        assert!(!codes.contains("compiler.function.abi_member_duplicate"));
        assert!(!message_keys.contains("diagnostics.compiler.function.abi_member_duplicate"));
        assert!(COMPILER_DIAGNOSTIC_DEFINITIONS.iter().all(|definition| {
            definition
                .templates
                .iter()
                .any(|template| template.locale == "en-US")
        }));
    }

    #[test]
    fn compiler_diagnostic_constructor_canonicalizes_related_locations() {
        let forward = CompilerDiagnostic::InputUnbound {
            port: "test/input".into(),
        }
        .into_node_with_related(
            DiagnosticLocation::Graph,
            vec![
                DiagnosticLocation::Resource("functions/zeta".into()),
                DiagnosticLocation::Node(NodeId::from_uuid(Uuid::from_u128(2))),
                DiagnosticLocation::Graph,
            ],
        );
        let reverse = CompilerDiagnostic::InputUnbound {
            port: "test/input".into(),
        }
        .into_node_with_related(
            DiagnosticLocation::Graph,
            vec![
                DiagnosticLocation::Graph,
                DiagnosticLocation::Node(NodeId::from_uuid(Uuid::from_u128(2))),
                DiagnosticLocation::Resource("functions/zeta".into()),
            ],
        );

        assert_eq!(forward.related, reverse.related);
        assert_eq!(
            forward.related.as_ref(),
            [
                DiagnosticLocation::Graph,
                DiagnosticLocation::Node(NodeId::from_uuid(Uuid::from_u128(2))),
                DiagnosticLocation::Resource("functions/zeta".into()),
            ]
        );
        assert_eq!(
            serde_json::to_vec(&forward).unwrap(),
            serde_json::to_vec(&reverse).unwrap()
        );
    }

    #[test]
    fn malformed_template_placeholders_are_typed_definition_errors() {
        let definition = |text| CompilerDiagnosticDefinition {
            code: "compiler.test.template",
            message_key: "diagnostics.compiler.test.template",
            default_severity: DiagnosticSeverity::Error,
            argument_names: &["value"],
            templates: Box::leak(
                vec![DiagnosticTemplate {
                    locale: "en-US",
                    text,
                }]
                .into_boxed_slice(),
            ),
        };

        assert!(matches!(
            validate_compiler_diagnostic_definitions(&[definition("Broken {value")]),
            Err(CompilerDiagnosticDefinitionError::UnmatchedTemplateBrace { brace: '{', .. })
        ));
        assert!(matches!(
            validate_compiler_diagnostic_definitions(&[definition("Broken value}")]),
            Err(CompilerDiagnosticDefinitionError::UnmatchedTemplateBrace { brace: '}', .. })
        ));
        assert!(matches!(
            validate_compiler_diagnostic_definitions(&[definition("Broken {Value}")]),
            Err(CompilerDiagnosticDefinitionError::InvalidTemplatePlaceholder {
                name,
                ..
            }) if name.as_ref() == "Value"
        ));
    }

    #[test]
    fn compiler_diagnostic_comparator_uses_only_canonical_fields() {
        let baseline = CompilerDiagnostic::InputUnbound {
            port: "test/input".into(),
        }
        .into_node(DiagnosticLocation::Graph);

        let mut later_primary = baseline.clone();
        later_primary.primary = DiagnosticLocation::Node(NodeId::from_uuid(Uuid::from_u128(1)));
        assert_eq!(
            compare_diagnostics(&baseline, &later_primary),
            Ordering::Less
        );

        let mut earlier_code = baseline.clone();
        earlier_code.code = DiagnosticCode::new("compiler.input.alpha");
        let mut later_code = baseline.clone();
        later_code.code = DiagnosticCode::new("compiler.input.zeta");
        assert_eq!(
            compare_diagnostics(&earlier_code, &later_code),
            Ordering::Less
        );

        let mut earlier_arguments = baseline.clone();
        earlier_arguments
            .arguments
            .insert("port".into(), "alpha".into());
        let mut later_arguments = baseline.clone();
        later_arguments
            .arguments
            .insert("port".into(), "zeta".into());
        assert_eq!(
            compare_diagnostics(&earlier_arguments, &later_arguments),
            Ordering::Less
        );

        let mut left_related = baseline.clone();
        left_related.related = vec![
            DiagnosticLocation::Resource("functions/zeta".into()),
            DiagnosticLocation::Graph,
        ]
        .into_boxed_slice();
        let mut right_related = baseline.clone();
        right_related.related = vec![
            DiagnosticLocation::Graph,
            DiagnosticLocation::Resource("functions/zeta".into()),
        ]
        .into_boxed_slice();
        assert_eq!(
            compare_diagnostics(&left_related, &right_related),
            Ordering::Equal
        );

        right_related.related = vec![
            DiagnosticLocation::Graph,
            DiagnosticLocation::Resource("functions/omega".into()),
        ]
        .into_boxed_slice();
        assert_eq!(
            compare_diagnostics(&right_related, &left_related),
            Ordering::Less
        );

        let mut different_presentation = baseline.clone();
        different_presentation.message_key =
            I18nKey::new("diagnostics.compiler.presentation.changed").unwrap();
        different_presentation.severity = DiagnosticSeverity::Information;
        assert_eq!(
            compare_diagnostics(&baseline, &different_presentation),
            Ordering::Equal
        );
        assert_eq!(
            compare_diagnostics(&different_presentation, &baseline),
            Ordering::Equal
        );
    }

    #[test]
    fn reviewed_compiler_diagnostics_emit_precise_named_facts() {
        let cases = [
            (
                CompilerDiagnostic::ControlDataPortRequired {
                    port_key: "condition".into(),
                    expected_direction: "input".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                BTreeMap::from([
                    (Box::from("expected_direction"), Box::from("input")),
                    (Box::from("port_key"), Box::from("condition")),
                ]),
            ),
            (
                CompilerDiagnostic::ControlControlPortRequired {
                    port_key: "body".into(),
                    expected_direction: "output".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                BTreeMap::from([
                    (Box::from("expected_direction"), Box::from("output")),
                    (Box::from("port_key"), Box::from("body")),
                ]),
            ),
            (
                CompilerDiagnostic::FunctionAbiManagedRoleInvalid {
                    expected_role: "function_entry".into(),
                    actual_count: "2".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                BTreeMap::from([
                    (Box::from("actual_count"), Box::from("2")),
                    (Box::from("expected_role"), Box::from("function_entry")),
                ]),
            ),
            (
                CompilerDiagnostic::ControlCallMemberMissing {
                    member_role: "argument".into(),
                    member_id: "customer_id".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                BTreeMap::from([
                    (Box::from("member_id"), Box::from("customer_id")),
                    (Box::from("member_role"), Box::from("argument")),
                ]),
            ),
            (
                CompilerDiagnostic::ControlCallMemberUnexpected {
                    member_role: "result".into(),
                    member_id: "total".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                BTreeMap::from([
                    (Box::from("member_id"), Box::from("total")),
                    (Box::from("member_role"), Box::from("result")),
                ]),
            ),
            (
                CompilerDiagnostic::ControlCallLocatorDuplicate {
                    function_path: "functions/customer".into(),
                    parameter_id: "customer_id".into(),
                    port: "node/arguments/1".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                BTreeMap::from([
                    (Box::from("function_path"), Box::from("functions/customer")),
                    (Box::from("parameter_id"), Box::from("customer_id")),
                    (Box::from("port"), Box::from("node/arguments/1")),
                ]),
            ),
            (
                CompilerDiagnostic::InterfaceDuplicateLocator {
                    port_key: "fields".into(),
                    locator: r#"{"kind":"schema_field","source":"source","field":"id"}"#.into(),
                }
                .into_node(DiagnosticLocation::Graph),
                BTreeMap::from([
                    (
                        Box::from("locator"),
                        Box::from(r#"{"kind":"schema_field","source":"source","field":"id"}"#),
                    ),
                    (Box::from("port_key"), Box::from("fields")),
                ]),
            ),
            (
                CompilerDiagnostic::InputUnbound {
                    port: "node/input".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                BTreeMap::from([(Box::from("port"), Box::from("node/input"))]),
            ),
            (
                CompilerDiagnostic::LoweringInternalInvariant {
                    node_type: "test.node".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                BTreeMap::from([(Box::from("node_type"), Box::from("test.node"))]),
            ),
            (
                CompilerDiagnostic::LoweringResultDuplicate {
                    result_name: "summary".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                BTreeMap::from([(Box::from("result_name"), Box::from("summary"))]),
            ),
            (
                CompilerDiagnostic::NodeDisappeared {
                    node_type: "test.node".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                BTreeMap::from([(Box::from("node_type"), Box::from("test.node"))]),
            ),
        ];

        for (diagnostic, expected_arguments) in cases {
            assert_eq!(diagnostic.arguments, expected_arguments);
        }
    }

    #[test]
    fn compiler_diagnostic_constructor_emits_only_declared_arguments() {
        let cases = [
            (
                CompilerDiagnostic::NodeUnknown {
                    node_type: "test.unknown".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                "compiler.node.unknown",
                "diagnostics.compiler.node.unknown",
                BTreeMap::from([(Box::from("node_type"), Box::from("test.unknown"))]),
            ),
            (
                CompilerDiagnostic::InputUnbound {
                    port: "test/input".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                "compiler.input.unbound",
                "diagnostics.compiler.input.unbound",
                BTreeMap::from([(Box::from("port"), Box::from("test/input"))]),
            ),
            (
                CompilerDiagnostic::TypeIncompatible {
                    expected_type: "core.integer".into(),
                    actual_type: "core.string".into(),
                }
                .into_node(DiagnosticLocation::Graph),
                "compiler.type.incompatible",
                "diagnostics.compiler.type.incompatible",
                BTreeMap::from([
                    (Box::from("actual_type"), Box::from("core.string")),
                    (Box::from("expected_type"), Box::from("core.integer")),
                ]),
            ),
        ];

        for (diagnostic, code, message_key, arguments) in cases {
            assert_eq!(diagnostic.code.as_str(), code);
            assert_eq!(diagnostic.message_key.as_str(), message_key);
            assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
            assert_eq!(diagnostic.arguments, arguments);
            assert!(!diagnostic.arguments.contains_key("detail"));
            assert!(diagnostic.related.is_empty());
        }
    }
}
