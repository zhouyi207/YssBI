//! Type domains and generic bindings shared by connection checks and both solver passes.
use crate::GraphPortSemanticFact;
use std::collections::{BTreeMap, BTreeSet};
use yss_graph_document::PortAddress;
use yss_node_protocol::{
    PortDirection, ResolvedType, TypeDomain, TypeExpr, TypeParameterId, TypeState,
    TypeUnknownReason,
};
use yss_node_registry::TypeRegistry;

const MAX_DOMAIN_SIZE: usize = 128;

impl GraphPortSemanticFact {
    /// Outputs use the resolved domain; inputs use the accepted domain instead
    /// of the narrower type of their currently connected value.
    pub fn connection_type(&self) -> TypeExpr {
        let domain = match self.direction {
            PortDirection::Output => self.type_state.domain(),
            PortDirection::Input => self.accepted_domain.as_ref().map(TypeDomain::types),
        };
        match domain {
            Some([value]) => resolved_type_expr(value),
            Some(values) => TypeExpr::Union(values.iter().map(resolved_type_expr).collect()),
            None => self.accepted_type.clone(),
        }
    }
}

pub(crate) fn resolved_type_expr(value: &ResolvedType) -> TypeExpr {
    match value {
        ResolvedType::Nominal(id) => TypeExpr::Concrete(id.clone()),
        ResolvedType::Applied {
            constructor,
            arguments,
        } => TypeExpr::Applied {
            constructor: constructor.clone(),
            arguments: arguments.iter().map(resolved_type_expr).collect(),
        },
    }
}

/// Reject disjoint domains using the same class expansion and assignability as
/// resolution. Unresolved generics remain connectable, but do not erase known
/// container shapes or turn a disjoint constrained domain into a wildcard.
pub fn type_patterns_can_connect(
    source: &TypeExpr,
    target: &TypeExpr,
    types: &TypeRegistry,
) -> bool {
    if let (Some(sources), Some(targets)) = (
        expand_pattern(source, types, &BTreeMap::new()),
        expand_pattern(target, types, &BTreeMap::new()),
    ) {
        return sources.iter().any(|source| targets.contains(source));
    }
    match (source, target) {
        (TypeExpr::Union(sources), target) => sources
            .iter()
            .any(|source| type_patterns_can_connect(source, target, types)),
        (source, TypeExpr::Union(targets)) => targets
            .iter()
            .any(|target| type_patterns_can_connect(source, target, types)),
        (TypeExpr::Unknown | TypeExpr::Generic(_), _)
        | (_, TypeExpr::Unknown | TypeExpr::Generic(_)) => true,
        (
            TypeExpr::Applied {
                constructor: source_constructor,
                arguments: source_arguments,
            },
            TypeExpr::Applied {
                constructor: target_constructor,
                arguments: target_arguments,
            },
        ) => {
            source_constructor == target_constructor
                && source_arguments.len() == target_arguments.len()
                && source_arguments
                    .iter()
                    .zip(target_arguments)
                    .all(|(source, target)| type_patterns_can_connect(source, target, types))
        }
        _ => false,
    }
}

pub(super) fn bind_input_generics(
    ports: &[GraphPortSemanticFact],
    states: &BTreeMap<PortAddress, TypeState>,
) -> (
    BTreeMap<TypeParameterId, TypeDomain>,
    BTreeSet<TypeParameterId>,
) {
    let mut bindings = BTreeMap::<TypeParameterId, TypeDomain>::new();
    let mut conflicts = BTreeSet::new();
    for port in ports
        .iter()
        .filter(|port| port.direction == PortDirection::Input)
    {
        let Some(state) = states.get(&port.address) else {
            continue;
        };
        bind_pattern_generics(&port.accepted_type, state, &mut bindings, &mut conflicts);
    }
    (bindings, conflicts)
}

pub(super) fn bind_pattern_generics(
    pattern: &TypeExpr,
    state: &TypeState,
    bindings: &mut BTreeMap<TypeParameterId, TypeDomain>,
    conflicts: &mut BTreeSet<TypeParameterId>,
) {
    let Some(domain) = state.domain() else {
        return;
    };
    match pattern {
        TypeExpr::Generic(parameter) => {
            let candidates = domain.iter().cloned().collect::<BTreeSet<_>>();
            let merged = match bindings.get(parameter) {
                Some(existing) => existing
                    .types()
                    .iter()
                    .filter(|value| candidates.contains(*value))
                    .cloned()
                    .collect(),
                None => candidates,
            };
            if let Some(domain) = TypeDomain::new(merged) {
                bindings.insert(parameter.clone(), domain);
            } else {
                conflicts.insert(parameter.clone());
            }
        }
        TypeExpr::Applied {
            constructor,
            arguments,
        } => {
            for (index, argument) in arguments.iter().enumerate() {
                let nested = domain
                    .iter()
                    .filter_map(|value| match value {
                        ResolvedType::Applied {
                            constructor: actual_constructor,
                            arguments: actual_arguments,
                        } if actual_constructor == constructor => actual_arguments.get(index),
                        ResolvedType::Nominal(_) | ResolvedType::Applied { .. } => None,
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !nested.is_empty() {
                    bind_pattern_generics(
                        argument,
                        &state_from_candidates(nested),
                        bindings,
                        conflicts,
                    );
                }
            }
        }
        TypeExpr::Union(members) => {
            // A scalar-or-series port binds the series element parameter from either shape.
            // Scalar alternatives are explicit, so frames and models cannot become elements.
            for member in members {
                let TypeExpr::Applied {
                    constructor,
                    arguments,
                } = member
                else {
                    continue;
                };
                if constructor.as_str() != "core.data_series" {
                    continue;
                }
                let [TypeExpr::Generic(parameter)] = arguments.as_slice() else {
                    continue;
                };
                let candidates = domain
                    .iter()
                    .filter_map(|value| match value {
                        ResolvedType::Nominal(id)
                            if members.iter().any(
                                |m| matches!(m,TypeExpr::Concrete(allowed) if allowed==id),
                            ) =>
                        {
                            Some(value.clone())
                        }
                        ResolvedType::Applied {
                            constructor: actual,
                            arguments,
                        } if actual == constructor => arguments.first().cloned(),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if !candidates.is_empty() {
                    bind_pattern_generics(
                        &TypeExpr::Generic(parameter.clone()),
                        &state_from_candidates(candidates),
                        bindings,
                        conflicts,
                    );
                }
            }
        }
        TypeExpr::Concrete(_) | TypeExpr::Class(_) | TypeExpr::Unknown => {}
    }
}

pub(super) fn state_from_pattern(
    pattern: &TypeExpr,
    types: &TypeRegistry,
    bindings: &BTreeMap<TypeParameterId, TypeDomain>,
) -> TypeState {
    expand_pattern(pattern, types, bindings).map_or(
        TypeState::Unknown(TypeUnknownReason::UnsupportedDeclaration),
        state_from_candidates,
    )
}

pub(super) fn expand_pattern(
    pattern: &TypeExpr,
    types: &TypeRegistry,
    bindings: &BTreeMap<TypeParameterId, TypeDomain>,
) -> Option<Vec<ResolvedType>> {
    let values = match pattern {
        TypeExpr::Concrete(id) => vec![ResolvedType::Nominal(id.clone())],
        TypeExpr::Class(class) => types
            .class_members(class)
            .map(|registration| ResolvedType::Nominal(registration.id.clone()))
            .collect(),
        TypeExpr::Generic(parameter) => bindings.get(parameter)?.types().to_vec(),
        TypeExpr::Applied {
            constructor,
            arguments,
        } => {
            let mut products = vec![Vec::new()];
            for argument in arguments {
                let candidates = expand_pattern(argument, types, bindings)?;
                let mut next = Vec::new();
                for product in &products {
                    for candidate in &candidates {
                        if next.len() >= MAX_DOMAIN_SIZE {
                            return None;
                        }
                        let mut product = product.clone();
                        product.push(candidate.clone());
                        next.push(product);
                    }
                }
                products = next;
            }
            products
                .into_iter()
                .map(|arguments| ResolvedType::Applied {
                    constructor: constructor.clone(),
                    arguments: arguments.into_boxed_slice(),
                })
                .collect()
        }
        TypeExpr::Union(members) => {
            let mut values = Vec::new();
            for member in members {
                values.extend(expand_pattern(member, types, bindings)?);
                if values.len() > MAX_DOMAIN_SIZE {
                    return None;
                }
            }
            values
        }
        TypeExpr::Unknown => return None,
    };
    (!values.is_empty()).then_some(values)
}

pub(super) fn exact_type_expr(value: &TypeExpr) -> Option<ResolvedType> {
    match value {
        TypeExpr::Concrete(id) => Some(ResolvedType::Nominal(id.clone())),
        TypeExpr::Applied {
            constructor,
            arguments,
        } => Some(ResolvedType::Applied {
            constructor: constructor.clone(),
            arguments: arguments
                .iter()
                .map(exact_type_expr)
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
        }),
        TypeExpr::Class(_) | TypeExpr::Generic(_) | TypeExpr::Union(_) | TypeExpr::Unknown => None,
    }
}

pub(super) fn state_from_candidates(values: impl IntoIterator<Item = ResolvedType>) -> TypeState {
    let Some(domain) = TypeDomain::new(values) else {
        return TypeState::Unknown(TypeUnknownReason::UnsupportedDeclaration);
    };
    match domain.types() {
        [value] => TypeState::Exact(value.clone()),
        _ => TypeState::Constrained(domain),
    }
}
