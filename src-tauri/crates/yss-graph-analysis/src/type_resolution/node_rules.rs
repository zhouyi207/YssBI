//! Declared node typing rules, numeric shapes and their input coercions.
use super::domains::{exact_type_expr, state_from_candidates};
use crate::parameter_projection::{effective_json_parameter, effective_text_parameter};
use crate::{GraphInputCoercion, GraphPortSemanticFact};
use std::collections::{BTreeMap, BTreeSet};
use yss_graph_document::{PortAddress, PortRef};
use yss_node_protocol::{
    InputCoercionKind, NodeTypingSpec, PortKey, PortSelector, ResolvedType, TypeConflict,
    TypeState, TypeUnknownReason,
};
use yss_node_registry::NodeRegistry;

pub(super) fn apply_node_rule(
    rule: &NodeTypingSpec,
    node: &yss_graph_document::DocumentNode,
    ports: &[GraphPortSemanticFact],
    states: &mut BTreeMap<PortAddress, TypeState>,
    registry: &NodeRegistry,
    constant_type: Option<&yss_data_contract::ValueType>,
    coercions: &mut Vec<GraphInputCoercion>,
) {
    match rule {
        NodeTypingSpec::Fixed => {}
        NodeTypingSpec::ColumnOutput {
            input,
            column,
            output,
        } => {
            let selected = effective_text_parameter(node, column, registry);
            let scalar = selected.and_then(|selected| {
                declared_port(ports, input)?
                    .schema_state
                    .exact()?
                    .fields
                    .iter()
                    .find(|field| field.name.0.as_ref() == selected)
                    .map(|field| field.scalar_type)
            });
            let nominal = match scalar {
                Some(yss_node_protocol::RelationalScalarType::Known(semantic)) => {
                    Some(semantic.type_id())
                }
                _ => None,
            };
            let state = nominal
                .map(|nominal| {
                    TypeState::Exact(ResolvedType::Applied {
                        constructor: "core.data_series".parse().expect("static series type"),
                        arguments: Box::new([ResolvedType::Nominal(
                            nominal.parse().expect("static scalar type"),
                        )]),
                    })
                })
                .unwrap_or(TypeState::Unknown(TypeUnknownReason::UnresolvedUpstream));
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::Identity { input, output } => {
            let state = declared_port(ports, input)
                .and_then(|port| states.get(&port.address))
                .cloned()
                .unwrap_or(TypeState::Unknown(TypeUnknownReason::UnconnectedInput));
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::NumericFold { inputs, output, .. } => {
            let selected = selected_ports(ports, inputs);
            let state = numeric_fold_state(&selected, states);
            if let Some(output) = declared_port(ports, output) {
                if let Some(result) = state.exact() {
                    coercions.extend(numeric_coercions(&selected, states, result));
                }
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::BinaryPredicate {
            left,
            right,
            output,
        } => {
            let selected = [left, right].map(|key| declared_port(ports, key));
            let result = match selected {
                [Some(left), Some(right)] => {
                    let left_state = &states[&left.address];
                    let right_state = &states[&right.address];
                    match (left_state.domain(), right_state.domain()) {
                        (Some(a), Some(b)) => {
                            let candidates = a
                                .iter()
                                .flat_map(|a| {
                                    b.iter().filter_map(move |b| binary_predicate_result(a, b))
                                })
                                .collect::<Vec<_>>();
                            let result = if candidates.is_empty() {
                                TypeState::Conflict(TypeConflict::IncompatibleInputs)
                            } else {
                                state_from_candidates(candidates)
                            };
                            if let Some(ResolvedType::Applied { .. }) = result.exact() {
                                for port in [left, right] {
                                    if matches!(
                                        states[&port.address].exact(),
                                        Some(ResolvedType::Nominal(_))
                                    ) {
                                        coercions.push(GraphInputCoercion {
                                            address: port.address.clone(),
                                            kind: InputCoercionKind::BroadcastScalarToSeries,
                                        });
                                    }
                                }
                            }
                            result
                        }
                        (None, _) => left_state.clone(),
                        (_, None) => right_state.clone(),
                    }
                }
                _ => TypeState::Unknown(TypeUnknownReason::UnconnectedInput),
            };
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), result);
            }
        }
        NodeTypingSpec::ShapePreservingNumeric { input, output } => {
            let state = declared_port(ports, input)
                .and_then(|port| states.get(&port.address))
                .map(shape_preserving_numeric_state)
                .unwrap_or(TypeState::Unknown(TypeUnknownReason::UnconnectedInput));
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::ShapePreservingConversion {
            input,
            parameter,
            output,
        } => {
            let target = effective_text_parameter(node, parameter, registry);
            if target == Some("auto") {
                // The demand pass has already constrained this output before cache lookup.
                // Input meaning must never select the automatic target.
                return;
            }
            let target = target
                .filter(|value| {
                    yss_node_protocol::SemanticType::ALL
                        .iter()
                        .any(|semantic| semantic.type_id() == *value)
                })
                .and_then(|value| yss_node_protocol::TypeId::new(value).ok());
            let input = declared_port(ports, input).and_then(|port| states.get(&port.address));
            let state = match (target, input) {
                (None, _) => TypeState::Conflict(TypeConflict::UnsupportedParameter),
                (Some(target), Some(state)) => match state.domain() {
                    Some(domain) => {
                        state_from_candidates(domain.iter().filter_map(|source| match source {
                            ResolvedType::Nominal(_) => Some(ResolvedType::Nominal(target.clone())),
                            ResolvedType::Applied {
                                constructor,
                                arguments,
                            } if constructor.as_str() == "core.data_series"
                                && arguments.len() == 1 =>
                            {
                                Some(ResolvedType::Applied {
                                    constructor: constructor.clone(),
                                    arguments: Box::new([ResolvedType::Nominal(target.clone())]),
                                })
                            }
                            _ => None,
                        }))
                    }
                    None => state.clone(),
                },
                (_, None) => TypeState::Unknown(TypeUnknownReason::UnconnectedInput),
            };
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::ConstantOutput { parameter, output } => {
            let state = if constant_type.is_some()
                || effective_json_parameter(node, parameter, registry).is_some()
            {
                constant_type
                    .and_then(|data_type| {
                        yss_graph_type_mapping::type_expr_from_data_type(data_type).ok()
                    })
                    .and_then(|value| exact_type_expr(&value))
                    .map(TypeState::Exact)
                    .unwrap_or(TypeState::Unknown(TypeUnknownReason::MissingResource))
            } else {
                TypeState::Conflict(TypeConflict::MissingParameter)
            };
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), state);
            }
        }
    }
}

pub(super) fn declared_port<'a>(
    ports: &'a [GraphPortSemanticFact],
    key: &PortKey,
) -> Option<&'a GraphPortSemanticFact> {
    ports.iter().find(
        |port| matches!(&port.address.port, PortRef::Declared { key: actual } if actual == key),
    )
}

fn selected_ports<'a>(
    ports: &'a [GraphPortSemanticFact],
    selectors: &[PortSelector],
) -> Vec<&'a GraphPortSemanticFact> {
    selectors
        .iter()
        .flat_map(|selector| {
            ports
                .iter()
                .filter(move |port| match (selector, &port.address.port) {
                    (PortSelector::Declared(expected), PortRef::Declared { key }) => {
                        key == expected
                    }
                    (PortSelector::AllInstances(expected), PortRef::Instance { template, .. }) => {
                        template == expected
                    }
                    _ => false,
                })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
enum NumericShape {
    Scalar,
    Series,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct NumericType {
    shape: NumericShape,
}

pub(super) fn binary_predicate_result(
    left: &ResolvedType,
    right: &ResolvedType,
) -> Option<ResolvedType> {
    fn element(value: &ResolvedType) -> Option<(&yss_node_protocol::TypeId, bool)> {
        match value {
            ResolvedType::Nominal(id) => Some((id, false)),
            ResolvedType::Applied {
                constructor,
                arguments,
            } if constructor.as_str() == "core.data_series" => match arguments.as_ref() {
                [ResolvedType::Nominal(id)] => Some((id, true)),
                _ => None,
            },
            _ => None,
        }
    }
    let (left, left_series) = element(left)?;
    let (right, right_series) = element(right)?;
    if left != right {
        return None;
    }
    let binary = ResolvedType::Nominal("core.binary".parse().expect("binary type"));
    Some(if left_series || right_series {
        ResolvedType::Applied {
            constructor: "core.data_series".parse().expect("series type"),
            arguments: Box::new([binary]),
        }
    } else {
        binary
    })
}

fn numeric_fold_state(
    ports: &[&GraphPortSemanticFact],
    states: &BTreeMap<PortAddress, TypeState>,
) -> TypeState {
    if ports.is_empty() {
        return TypeState::Unknown(TypeUnknownReason::UnconnectedInput);
    }
    let mut accumulated = BTreeSet::<NumericType>::new();
    for (index, port) in ports.iter().enumerate() {
        let Some(state) = states.get(&port.address) else {
            return TypeState::Unknown(TypeUnknownReason::UnresolvedUpstream);
        };
        let Some(domain) = state.domain() else {
            return match state {
                TypeState::Conflict(_) => TypeState::Conflict(TypeConflict::IncompatibleInputs),
                TypeState::Unknown(reason) => TypeState::Unknown(*reason),
                TypeState::Exact(_) | TypeState::Constrained(_) => {
                    TypeState::Unknown(TypeUnknownReason::UnresolvedUpstream)
                }
            };
        };
        let candidates = domain
            .iter()
            .filter_map(numeric_type)
            .collect::<BTreeSet<_>>();
        if candidates.is_empty() {
            return TypeState::Conflict(TypeConflict::IncompatibleInputs);
        }
        if index == 0 {
            accumulated = candidates;
            continue;
        }
        accumulated = accumulated
            .iter()
            .flat_map(|left| {
                candidates
                    .iter()
                    .map(move |right| join_numeric(*left, *right))
            })
            .collect();
    }
    state_from_candidates(accumulated.into_iter().map(resolved_numeric_type))
}

fn join_numeric(left: NumericType, right: NumericType) -> NumericType {
    NumericType {
        shape: if left.shape == NumericShape::Series || right.shape == NumericShape::Series {
            NumericShape::Series
        } else {
            NumericShape::Scalar
        },
    }
}

fn shape_preserving_numeric_state(input: &TypeState) -> TypeState {
    let Some(domain) = input.domain() else {
        return input.clone();
    };
    let candidates = domain
        .iter()
        .filter_map(numeric_type)
        .map(|value| resolved_numeric_type(NumericType { shape: value.shape }))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        TypeState::Conflict(TypeConflict::IncompatibleInputs)
    } else {
        state_from_candidates(candidates)
    }
}

fn numeric_coercions(
    ports: &[&GraphPortSemanticFact],
    states: &BTreeMap<PortAddress, TypeState>,
    result: &ResolvedType,
) -> Vec<GraphInputCoercion> {
    let Some(result) = numeric_type(result) else {
        return Vec::new();
    };
    let mut coercions = Vec::new();
    for port in ports {
        let Some(input) = states.get(&port.address).and_then(TypeState::exact) else {
            continue;
        };
        let Some(input) = numeric_type(input) else {
            continue;
        };
        if input.shape == NumericShape::Scalar && result.shape == NumericShape::Series {
            coercions.push(GraphInputCoercion {
                address: port.address.clone(),
                kind: InputCoercionKind::BroadcastScalarToSeries,
            });
        }
    }
    coercions
}

fn numeric_type(value: &ResolvedType) -> Option<NumericType> {
    match value {
        ResolvedType::Nominal(id) if id.as_str() == "core.numeric" => Some(NumericType {
            shape: NumericShape::Scalar,
        }),
        ResolvedType::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == yss_node_protocol::DATA_SERIES_CONSTRUCTOR_ID
            && matches!(arguments.as_ref(), [ResolvedType::Nominal(id)] if id.as_str() == "core.numeric") =>
        {
            Some(NumericType {
                shape: NumericShape::Series,
            })
        }
        _ => None,
    }
}

fn resolved_numeric_type(value: NumericType) -> ResolvedType {
    let element = ResolvedType::Nominal(
        yss_node_protocol::TypeId::new("core.numeric").expect("semantic type ID"),
    );
    match value.shape {
        NumericShape::Scalar => element,
        NumericShape::Series => ResolvedType::Applied {
            constructor: yss_node_protocol::TypeConstructorId::new(
                yss_node_protocol::DATA_SERIES_CONSTRUCTOR_ID,
            )
            .expect("series constructor"),
            arguments: Box::new([element]),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_join_is_commutative_associative_and_idempotent() {
        let values = [
            NumericType {
                shape: NumericShape::Scalar,
            },
            NumericType {
                shape: NumericShape::Scalar,
            },
            NumericType {
                shape: NumericShape::Series,
            },
            NumericType {
                shape: NumericShape::Series,
            },
        ];
        for left in values {
            assert_eq!(join_numeric(left, left), left);
            for right in values {
                assert_eq!(join_numeric(left, right), join_numeric(right, left));
                for third in values {
                    assert_eq!(
                        join_numeric(join_numeric(left, right), third,),
                        join_numeric(left, join_numeric(right, third),)
                    );
                }
            }
        }
    }
}
