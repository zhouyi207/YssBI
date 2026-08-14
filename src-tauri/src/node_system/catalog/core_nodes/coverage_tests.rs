use super::{CoverageDisposition, build_provider_fragment, legacy_coverage};
use crate::node_system::catalog::localization::Message;
use crate::node_system::compiler::{
    CompileCancellationToken, LoweredKernel, LoweringContext, NodeImplementation,
    ValidatedNodeConfig,
};
use crate::node_system::document::{NodeId, PortAddress};
use crate::node_system::plan::ValueRef;
use crate::node_system::protocol::{PortDirection, PortKind};
use std::collections::BTreeSet;

const LEGACY_CORE_NODES: &[&str] = &[
    "Value:Constants:Boolean",
    "Value:Constants:Int64",
    "Value:Constants:Float64",
    "Value:Constants:String",
    "Value:Conversion:Convert",
    "Data:Conversion:String to Categorical",
    "Data:Conversion:String to Float64",
    "Data:Conversion:String to Int64",
    "Data:Conversion:Int64 to String",
    "Data:Conversion:Float64 to String",
    "Data:Conversion:Int64 to Float64",
    "Data:Conversion:Float64 to Int64",
    "Data:Conversion:Int64 to Boolean",
    "Data:Conversion:Float64 to Boolean",
    "Data:Conversion:Categorical to String",
    "Data:Conversion:Int64 to Categorical",
    "Data:Conversion:Categorical to Int64",
    "Data:Conversion:Float64 to Categorical",
    "Data:Conversion:Categorical to Float64",
    "Math:Operators:Add (+)",
    "Math:Operators:Subtract (-)",
    "Math:Operators:Multiply (*)",
    "Math:Operators:Divide (/)",
    "Math:Functions:Ln",
    "Math:Functions:Log2",
    "Math:Functions:Log10",
    "Math:Functions:Exp",
    "Math:Functions:Sqrt",
    "Math:Functions:Square",
    "Logic:Comparison:Equal (==)",
    "Logic:Comparison:Not Equal (!=)",
    "Logic:Boolean:And (&&)",
    "Logic:Boolean:Or (||)",
    "Logic:Boolean:Not (!)",
    "Control Flow:Branch",
    "Control Flow:Sequence",
    "Control Flow:Do",
    "Control Flow:Merge",
    "Control Flow:Sleep",
    "Control Flow:For Loop",
    "Control Flow:Switch",
    "Control Flow:While Loop",
    "Debug:Print",
    "Debug:Data:View",
];

#[test]
fn legacy_core_catalog_has_an_explicit_complete_coverage_list() {
    let expected = LEGACY_CORE_NODES.iter().copied().collect::<BTreeSet<_>>();
    let actual = legacy_coverage()
        .iter()
        .map(|entry| entry.legacy_node_type)
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, expected);
    assert_eq!(legacy_coverage().len(), LEGACY_CORE_NODES.len());
    assert!(
        legacy_coverage()
            .iter()
            .all(|entry| !entry.stable_ids.is_empty())
    );
}

#[test]
fn migrated_coverage_entries_are_owned_by_the_provider_fragment() {
    let fragment = build_provider_fragment().expect("core built-in fixture must assemble");
    let node_ids = fragment
        .nodes
        .iter()
        .map(|node| node.protocol().type_id.as_str())
        .collect::<BTreeSet<_>>();

    for entry in legacy_coverage() {
        if entry.disposition == CoverageDisposition::MigratedHere {
            for stable_id in entry.stable_ids {
                assert!(
                    node_ids.contains(stable_id),
                    "migrated legacy node '{}' is missing stable node '{}'",
                    entry.legacy_node_type,
                    stable_id,
                );
            }
        }
    }
}

#[test]
fn protocols_use_unique_stable_port_and_parameter_keys() {
    let fragment = build_provider_fragment().expect("core built-in fixture must assemble");
    for node in fragment.nodes {
        let ports = node
            .protocol()
            .interface
            .ports
            .iter()
            .map(|port| port.key.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(ports.len(), node.protocol().interface.ports.len());

        let parameters = node
            .protocol()
            .parameters
            .parameters
            .iter()
            .map(|parameter| parameter.key.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            parameters.len(),
            node.protocol().parameters.parameters.len()
        );
    }
}

#[test]
fn view_data_has_no_data_output_or_fragment_result() {
    let fragment = build_provider_fragment().expect("core built-in fixture must assemble");
    let node = fragment
        .nodes
        .iter()
        .find(|node| node.protocol().type_id.as_str() == "yssbi.debug.view")
        .expect("View Data is registered");
    let protocol = node.protocol();
    assert_eq!(
        protocol
            .interface
            .ports
            .iter()
            .map(|port| { (port.key.as_str(), port.direction, port.kind,) })
            .collect::<Vec<_>>(),
        [
            ("enter", PortDirection::Input, PortKind::Control),
            ("data", PortDirection::Input, PortKind::Data),
            ("then", PortDirection::Output, PortKind::Control),
        ]
    );

    let implementation = node
        .implementation()
        .expect("View Data has compiler lowering")
        .as_any()
        .downcast_ref::<NodeImplementation>()
        .expect("View Data uses the native compiler lowerer");
    let data = PortAddress::declared(
        NodeId::from_uuid(uuid::Uuid::from_u128(1)),
        protocol.interface.ports[1].key.clone(),
    );
    let cancellation = CompileCancellationToken::new();
    let parameters = ValidatedNodeConfig::empty();
    let lowered = implementation
        .lowerer
        .lower(&LoweringContext {
            cancellation: &cancellation,
            node_id: data.node_id,
            protocol,
            parameters: &parameters,
            inputs: &[(data, ValueRef::new(0))],
            outputs: &[],
        })
        .expect("View Data lowering succeeds");
    let LoweredKernel::Kernel(kernel) = lowered.kernel else {
        panic!("View Data lowers to a kernel fragment");
    };
    assert!(kernel.metadata.results.is_empty());
}

#[test]
fn every_legacy_core_entry_has_current_behavioral_or_structural_evidence() {
    const RUNTIME: &str = "node_system::runtime::builtin_tests";
    const COMPILER: &str = "node_system::compiler::tests";
    const PRODUCTION: &str = "project::structured_control_production_tests";
    const COVERAGE: &[(&str, &str, &[&str])] = &[
        (
            "Value:Constants:Boolean",
            RUNTIME,
            &["constant_kernels_resolve_compiled_parameters_by_plan_handle"],
        ),
        (
            "Value:Constants:Int64",
            RUNTIME,
            &["constant_kernels_resolve_compiled_parameters_by_plan_handle"],
        ),
        (
            "Value:Constants:Float64",
            RUNTIME,
            &["constant_kernels_resolve_compiled_parameters_by_plan_handle"],
        ),
        (
            "Value:Constants:String",
            RUNTIME,
            &["constant_kernels_resolve_compiled_parameters_by_plan_handle"],
        ),
        (
            "Value:Conversion:Convert",
            RUNTIME,
            &["scalar_convert_kernel_covers_supported_targets_and_errors"],
        ),
        (
            "Data:Conversion:String to Categorical",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:String to Float64",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:String to Int64",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:Int64 to String",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:Float64 to String",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:Int64 to Float64",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:Float64 to Int64",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:Int64 to Boolean",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:Float64 to Boolean",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:Categorical to String",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:Int64 to Categorical",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:Categorical to Int64",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:Float64 to Categorical",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Data:Conversion:Categorical to Float64",
            RUNTIME,
            &["series_conversion_kernels_cover_every_legacy_conversion"],
        ),
        (
            "Math:Operators:Add (+)",
            RUNTIME,
            &["numeric_kernels_execute_int64_and_float64_operations"],
        ),
        (
            "Math:Operators:Subtract (-)",
            RUNTIME,
            &["numeric_kernels_execute_int64_and_float64_operations"],
        ),
        (
            "Math:Operators:Multiply (*)",
            RUNTIME,
            &["numeric_kernels_execute_int64_and_float64_operations"],
        ),
        (
            "Math:Operators:Divide (/)",
            RUNTIME,
            &[
                "numeric_kernels_execute_int64_and_float64_operations",
                "builtin_kernels_report_division_by_zero_and_type_errors",
            ],
        ),
        (
            "Math:Functions:Ln",
            RUNTIME,
            &["unary_math_kernels_execute_each_legacy_operation"],
        ),
        (
            "Math:Functions:Log2",
            RUNTIME,
            &["unary_math_kernels_execute_each_legacy_operation"],
        ),
        (
            "Math:Functions:Log10",
            RUNTIME,
            &["unary_math_kernels_execute_each_legacy_operation"],
        ),
        (
            "Math:Functions:Exp",
            RUNTIME,
            &["unary_math_kernels_execute_each_legacy_operation"],
        ),
        (
            "Math:Functions:Sqrt",
            RUNTIME,
            &["unary_math_kernels_execute_each_legacy_operation"],
        ),
        (
            "Math:Functions:Square",
            RUNTIME,
            &["unary_math_kernels_execute_each_legacy_operation"],
        ),
        (
            "Logic:Comparison:Equal (==)",
            RUNTIME,
            &["equal_kernel_covers_bool_int_string_and_float"],
        ),
        (
            "Logic:Comparison:Not Equal (!=)",
            RUNTIME,
            &["compare_and_logic_kernels_execute_through_the_run_scheduler"],
        ),
        (
            "Logic:Boolean:And (&&)",
            RUNTIME,
            &["compare_and_logic_kernels_execute_through_the_run_scheduler"],
        ),
        (
            "Logic:Boolean:Or (||)",
            RUNTIME,
            &["compare_and_logic_kernels_execute_through_the_run_scheduler"],
        ),
        (
            "Logic:Boolean:Not (!)",
            RUNTIME,
            &["compare_and_logic_kernels_execute_through_the_run_scheduler"],
        ),
        (
            "Control Flow:Branch",
            PRODUCTION,
            &[
                "builtin_branch_executes_only_selected_effect_branch_and_binds_result",
                "builtin_branch_false_path_executes_only_selected_effect_and_binds_result",
            ],
        ),
        (
            "Control Flow:Sequence",
            COMPILER,
            &["builtin_multi_output_sequence_outside_branch_keeps_walker_order"],
        ),
        (
            "Control Flow:Do",
            PRODUCTION,
            &["builtin_effect_edge_orders_real_builtins_independent_of_document_insertion"],
        ),
        (
            "Control Flow:Merge",
            COMPILER,
            &["branch_continuation_allows_multi_output_sequence_suffix_after_merge"],
        ),
        (
            "Control Flow:Sleep",
            RUNTIME,
            &["do_sleep_print_and_view_leaf_kernels_preserve_contracts"],
        ),
        (
            "Control Flow:For Loop",
            PRODUCTION,
            &["builtin_loop_carries_initial_and_subsequent_values_across_observable_iterations"],
        ),
        (
            "Control Flow:Switch",
            RUNTIME,
            &[
                "nested_branch_sequence_switch_executes_only_n_way_match",
                "nested_branch_sequence_switch_executes_default_when_no_case_matches",
            ],
        ),
        (
            "Control Flow:While Loop",
            PRODUCTION,
            &["builtin_loop_reports_iteration_limit_without_committing_result"],
        ),
        (
            "Debug:Print",
            RUNTIME,
            &[
                "do_sleep_print_and_view_scheduler_contracts",
                "print_observer_and_trace_preserve_exact_first_second_third_order",
                "real_graph_connection_overrides_print_protocol_default_at_runtime",
                "print_protocol_has_default_and_ordered_chain_contract",
            ],
        ),
        (
            "Debug:Data:View",
            RUNTIME,
            &[
                "do_sleep_print_and_view_scheduler_contracts",
                "view_data_opens_exact_input_result_without_materialization",
            ],
        ),
    ];

    let expected = LEGACY_CORE_NODES.iter().copied().collect::<BTreeSet<_>>();
    let actual = COVERAGE
        .iter()
        .map(|(legacy, _, _)| *legacy)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "behavior evidence must classify every legacy core entry"
    );
    let focused_suites = BTreeSet::from([RUNTIME, COMPILER, PRODUCTION]);
    for (legacy, suite, tests) in COVERAGE {
        assert!(
            focused_suites.contains(suite),
            "legacy core entry '{legacy}' must name a runnable focused suite"
        );
        assert!(
            !tests.is_empty() && tests.iter().all(|test| !test.trim().is_empty()),
            "legacy core entry '{legacy}' must name explicit behavior or structural evidence"
        );
    }
}

#[test]
fn every_migrated_node_has_localized_search_terms() {
    let fragment = build_provider_fragment().expect("core built-in fixture must assemble");
    let aliases = fragment
        .messages
        .iter()
        .filter(|(_, _, message)| matches!(message, Message::Aliases(_)))
        .map(|(locale, key, _)| (*locale, *key))
        .collect::<BTreeSet<_>>();

    for node in fragment.nodes {
        let key = node
            .protocol()
            .catalog
            .aliases_key
            .as_ref()
            .expect("core nodes expose aliases and technical terms");
        assert!(aliases.contains(&("en-US", key.as_str())));
        assert!(aliases.contains(&("zh-CN", key.as_str())));
    }
}
