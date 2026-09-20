//! Bidirectional candidate constraints for automatic conversion outputs. These are
//! temporary solver facts; final types, diagnostics and specialization stay in the
//! normal forward resolution pass.

use super::*;

type Domain = Option<BTreeSet<ResolvedType>>;

fn domain(state: &TypeState) -> Domain {
    match state {
        TypeState::Conflict(_) => Some(BTreeSet::new()),
        _ => state
            .domain()
            .map(|values| values.iter().cloned().collect()),
    }
}

fn state(domain: &Domain) -> TypeState {
    match domain {
        Some(values) if values.is_empty() => TypeState::Conflict(TypeConflict::IncompatibleInputs),
        Some(values) => state_from_candidates(values.iter().cloned()),
        None => TypeState::Unknown(TypeUnknownReason::UnresolvedUpstream),
    }
}

fn narrow(
    domains: &mut BTreeMap<PortAddress, Domain>,
    port: &PortAddress,
    restriction: &Domain,
) -> bool {
    let Some(restriction) = restriction else {
        return false;
    };
    let Some(current) = domains.get_mut(port) else {
        return false;
    };
    let next = current.as_ref().map_or_else(
        || restriction.clone(),
        |current| current.intersection(restriction).cloned().collect(),
    );
    if current.as_ref() == Some(&next) {
        return false;
    }
    *current = Some(next);
    true
}

fn generic_parameters(pattern: &TypeExpr, parameters: &mut BTreeSet<TypeParameterId>) {
    match pattern {
        TypeExpr::Generic(parameter) => {
            parameters.insert(parameter.clone());
        }
        TypeExpr::Applied { arguments, .. } | TypeExpr::Union(arguments) => {
            for argument in arguments {
                generic_parameters(argument, parameters);
            }
        }
        _ => {}
    }
}

fn shape(value: &ResolvedType) -> u8 {
    match value {
        ResolvedType::Nominal(_) => 1,
        ResolvedType::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.data_series" && arguments.len() == 1 => 2,
        _ => 0,
    }
}

pub(super) fn automatic_output_constraints(
    document: &GraphDocument,
    index: &super::super::document_index::DocumentIndex<'_>,
    registry: &NodeRegistry,
    nodes: &[GraphNodeSemanticFact],
) -> BTreeMap<PortAddress, TypeState> {
    let automatic = document
        .nodes
        .values()
        .filter_map(|node| {
            let NodeTypingSpec::ShapePreservingConversion {
                parameter, output, ..
            } = &registry.protocol(&node.node_type)?.typing
            else {
                return None;
            };
            (conversion_target(node, parameter, registry) == Some("auto"))
                .then(|| PortAddress::declared(node.id, output.clone()))
        })
        .collect::<BTreeSet<_>>();
    if automatic.is_empty() {
        return BTreeMap::new();
    }

    // Disconnected components cannot constrain an automatic output. Limit the
    // reverse pass to its connected components; forward resolution still visits all nodes.
    let mut neighbors = BTreeMap::<NodeId, Vec<NodeId>>::new();
    for connection in document.connections.values() {
        let source = connection.output.node_id;
        let target = connection.input.node_id;
        neighbors.entry(source).or_default().push(target);
        neighbors.entry(target).or_default().push(source);
    }
    let mut connected = automatic
        .iter()
        .map(|port| port.node_id)
        .collect::<BTreeSet<_>>();
    let mut pending = connected.iter().copied().collect::<VecDeque<_>>();
    while let Some(node) = pending.pop_front() {
        for neighbor in neighbors.get(&node).into_iter().flatten() {
            if connected.insert(*neighbor) {
                pending.push_back(*neighbor);
            }
        }
    }
    let nodes = nodes
        .iter()
        .filter(|node| connected.contains(&node.node_id))
        .map(|node| (node.node_id, node))
        .collect::<BTreeMap<_, _>>();
    let mut domains = BTreeMap::new();
    for node in nodes.values() {
        for port in node.ports.iter().filter(|port| !port.orphan) {
            let mut initial = domain(&state_from_pattern(
                &port.accepted_type,
                registry.types(),
                &BTreeMap::new(),
            ));
            if port.direction == PortDirection::Input && !index.incoming.contains_key(&port.address)
            {
                let literal = document
                    .input_states
                    .get(&port.address)
                    .and_then(|input| input.literal_override.as_ref())
                    .or(port.protocol_default.as_ref());
                if let Some(literal) =
                    literal.and_then(|literal| exact_type_expr(&literal.value_type))
                {
                    let literal = BTreeSet::from([literal]);
                    initial = Some(initial.map_or_else(
                        || literal.clone(),
                        |values| values.intersection(&literal).cloned().collect(),
                    ));
                }
            }
            domains.insert(port.address.clone(), initial);
        }
    }
    let mut edges = BTreeMap::<PortAddress, Vec<PortAddress>>::new();
    for connection in document.connections.values() {
        if domains.contains_key(&connection.output) && domains.contains_key(&connection.input) {
            edges
                .entry(connection.output.clone())
                .or_default()
                .push(connection.input.clone());
            edges
                .entry(connection.input.clone())
                .or_default()
                .push(connection.output.clone());
        }
    }
    let mut queue = nodes.keys().copied().collect::<VecDeque<_>>();
    let mut queued = nodes.keys().copied().collect::<BTreeSet<_>>();
    // Domains only narrow. A work queue reaches a fixed point without a graph-size
    // dependent iteration cutoff, including arbitrary-length transparent chains.
    while let Some(id) = queue.pop_front() {
        queued.remove(&id);
        let node = nodes[&id];
        let Some(document_node) = document.nodes.get(&id) else {
            continue;
        };
        let Some(protocol) = registry.protocol(&document_node.node_type) else {
            continue;
        };
        let mut dirty = BTreeSet::new();
        let mut bindings = BTreeMap::new();
        let mut conflicts = BTreeSet::new();
        for port in node.ports.iter().filter(|port| !port.orphan) {
            if let Some(values) = domains[&port.address].as_ref() {
                if values.is_empty() {
                    generic_parameters(&port.accepted_type, &mut conflicts);
                } else {
                    bind_pattern_generics(
                        &port.accepted_type,
                        &state_from_candidates(values.iter().cloned()),
                        &mut bindings,
                        &mut conflicts,
                    );
                }
            }
        }
        for port in node.ports.iter().filter(|port| !port.orphan) {
            let mut parameters = BTreeSet::new();
            generic_parameters(&port.accepted_type, &mut parameters);
            let restriction = if !parameters.is_disjoint(&conflicts) {
                Some(BTreeSet::new())
            } else {
                expand_pattern(&port.accepted_type, registry.types(), &bindings)
                    .map(|values| values.into_iter().collect())
            };
            if narrow(&mut domains, &port.address, &restriction) {
                dirty.insert(id);
            }
        }
        let mut states = node
            .ports
            .iter()
            .filter(|port| !port.orphan)
            .map(|port| (port.address.clone(), state(&domains[&port.address])))
            .collect::<BTreeMap<_, _>>();
        apply_node_rule(
            &protocol.typing,
            document_node,
            &node.ports,
            &mut states,
            registry,
            document,
            &mut Vec::new(),
        );
        for port in node
            .ports
            .iter()
            .filter(|port| !port.orphan && port.direction == PortDirection::Output)
        {
            if let Some(value) = states.get(&port.address)
                && narrow(&mut domains, &port.address, &domain(value))
            {
                dirty.insert(id);
            }
        }
        match &protocol.typing {
            NodeTypingSpec::BinaryPredicate {
                left,
                right,
                output,
            } => {
                if let (Some(left), Some(right), Some(output)) = (
                    declared_port(&node.ports, left),
                    declared_port(&node.ports, right),
                    declared_port(&node.ports, output),
                ) && !left.orphan
                    && !right.orphan
                    && !output.orphan
                    && let (Some(a), Some(b), Some(results)) = (
                        &domains[&left.address],
                        &domains[&right.address],
                        &domains[&output.address],
                    )
                {
                    let mut allowed_left = BTreeSet::new();
                    let mut allowed_right = BTreeSet::new();
                    let mut allowed_output = BTreeSet::new();
                    for a in a {
                        for b in b {
                            if let Some(result) = binary_predicate_result(a, b)
                                && results.contains(&result)
                            {
                                allowed_left.insert(a.clone());
                                allowed_right.insert(b.clone());
                                allowed_output.insert(result);
                            }
                        }
                    }
                    for (port, values) in [
                        (left, allowed_left),
                        (right, allowed_right),
                        (output, allowed_output),
                    ] {
                        if narrow(&mut domains, &port.address, &Some(values)) {
                            dirty.insert(id);
                        }
                    }
                }
            }
            NodeTypingSpec::Identity { input, output }
            | NodeTypingSpec::ShapePreservingConversion { input, output, .. } => {
                if let (Some(input), Some(output)) = (
                    declared_port(&node.ports, input),
                    declared_port(&node.ports, output),
                ) && !input.orphan
                    && !output.orphan
                {
                    let left = domains[&input.address].clone();
                    let right = domains[&output.address].clone();
                    if matches!(protocol.typing, NodeTypingSpec::Identity { .. }) {
                        if narrow(&mut domains, &input.address, &right) {
                            dirty.insert(id);
                        }
                        if narrow(&mut domains, &output.address, &left) {
                            dirty.insert(id);
                        }
                    } else if let (Some(left), Some(right)) = (left, right) {
                        // Conversion changes meaning, never shape: downstream meaning
                        // must not leak through the conversion into its source.
                        let left_shapes = left.iter().fold(0, |mask, value| mask | shape(value));
                        let right_shapes = right.iter().fold(0, |mask, value| mask | shape(value));
                        let input_types = Some(
                            left.into_iter()
                                .filter(|value| shape(value) & right_shapes != 0)
                                .collect(),
                        );
                        let output_types = Some(
                            right
                                .into_iter()
                                .filter(|value| shape(value) & left_shapes != 0)
                                .collect(),
                        );
                        if narrow(&mut domains, &input.address, &input_types) {
                            dirty.insert(id);
                        }
                        if narrow(&mut domains, &output.address, &output_types) {
                            dirty.insert(id);
                        }
                    }
                }
            }
            _ => {}
        }
        for port in node.ports.iter().filter(|port| !port.orphan) {
            for neighbor in edges.get(&port.address).into_iter().flatten() {
                let left = domains[&port.address].clone();
                let right = domains[neighbor].clone();
                if narrow(&mut domains, &port.address, &right) {
                    dirty.insert(id);
                }
                if narrow(&mut domains, neighbor, &left) {
                    dirty.insert(neighbor.node_id);
                }
            }
        }
        for dirty in dirty {
            if queued.insert(dirty) {
                queue.push_back(dirty);
            }
        }
    }
    automatic
        .into_iter()
        .filter_map(|address| {
            domains
                .get(&address)
                .map(|domain| (address.clone(), state(domain)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_data_contract::{DataValue, SemanticType, ValueType};
    use yss_graph_document::{
        ConnectionId, ConstantId, DocumentConnection, DocumentNode, GraphConstant, NodePosition,
        ParameterValues,
    };
    use yss_graph_resource_contract::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};

    fn node(document: &mut GraphDocument, kind: &str) -> NodeId {
        let id = NodeId::new();
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        id
    }

    fn constant(
        document: &mut GraphDocument,
        node: NodeId,
        semantic: SemanticType,
        value: DataValue,
    ) {
        let id = ConstantId::from_uuid(node.as_uuid());
        document.constants.insert(
            id,
            GraphConstant {
                id,
                name: "source".into(),
                data_type: ValueType::Scalar(semantic),
                data_value: value,
                tabular: None,
                description: String::new(),
                tags: vec![],
            },
        );
        document.nodes.get_mut(&node).unwrap().parameters.insert(
            "constant".parse().unwrap(),
            serde_json::json!(id.to_string()),
        );
    }

    fn connect(
        document: &mut GraphDocument,
        source: NodeId,
        output: &str,
        target: NodeId,
        input: &str,
    ) -> ConnectionId {
        let id = ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(source, output.parse().unwrap()),
                input: PortAddress::declared(target, input.parse().unwrap()),
                order: None,
            },
        );
        id
    }

    fn resolve(
        document: &GraphDocument,
        convert: NodeId,
        cache: &mut GraphSemanticCache,
    ) -> TypeState {
        let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
        let resources = ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            ResourceCatalogFingerprint::from_bytes([0; 32]),
        );
        let incremental = crate::resolve_graph_semantics_with_cache(
            document,
            &builtin.registry,
            &resources,
            cache,
        );
        assert_eq!(
            incremental,
            crate::resolve_graph_semantics(document, &builtin.registry, &resources)
        );
        incremental
            .node(convert)
            .unwrap()
            .ports
            .iter()
            .find(|port| port.address == PortAddress::declared(convert, "output".parse().unwrap()))
            .unwrap()
            .type_state
            .clone()
    }

    fn exact(kind: &str) -> TypeState {
        TypeState::Exact(ResolvedType::Nominal(kind.parse().unwrap()))
    }

    #[test]
    fn automatic_conversion_intersects_consumers_and_invalidates_cache_on_reconnection() {
        let mut document = GraphDocument::default();
        let source = node(&mut document, "yssbi.constant.get");
        constant(
            &mut document,
            source,
            SemanticType::Text,
            DataValue::String("001".into()),
        );
        let convert = node(&mut document, "yssbi.value.convert");
        connect(&mut document, source, "value", convert, "input");
        let mut cache = GraphSemanticCache::default();
        assert!(matches!(
            resolve(&document, convert, &mut cache),
            TypeState::Constrained(_)
        ));
        let view = node(&mut document, "yssbi.debug.view");
        connect(&mut document, convert, "output", view, "data");
        assert!(matches!(
            resolve(&document, convert, &mut cache),
            TypeState::Constrained(_)
        ));
        let numeric = node(&mut document, "yssbi.logic.less");
        let threshold = node(&mut document, "yssbi.constant.get");
        constant(
            &mut document,
            threshold,
            SemanticType::Numeric,
            DataValue::Integer(1),
        );
        connect(&mut document, threshold, "value", numeric, "right");
        let numeric_edge = connect(&mut document, convert, "output", numeric, "left");
        assert_eq!(
            resolve(&document, convert, &mut cache),
            exact("core.numeric")
        );
        let binary = node(&mut document, "yssbi.logic.not");
        let binary_edge = connect(&mut document, convert, "output", binary, "input");
        assert!(matches!(
            resolve(&document, convert, &mut cache),
            TypeState::Conflict(_)
        ));
        document.connections.remove(&numeric_edge);
        assert_eq!(
            resolve(&document, convert, &mut cache),
            exact("core.binary")
        );
        document.connections.remove(&binary_edge);
        assert!(matches!(
            resolve(&document, convert, &mut cache),
            TypeState::Constrained(_)
        ));
        assert!(
            !document.nodes[&convert]
                .parameters
                .contains_key(&"target_type".parse().unwrap())
        );
        document.nodes.get_mut(&convert).unwrap().parameters.insert(
            "target_type".parse().unwrap(),
            serde_json::json!("core.text"),
        );
        assert_eq!(resolve(&document, convert, &mut cache), exact("core.text"));
    }

    #[test]
    fn automatic_conversion_follows_generic_constraints_through_reroutes_and_preserves_shape() {
        let mut document = GraphDocument::default();
        let source = node(&mut document, "yssbi.constant.get");
        constant(
            &mut document,
            source,
            SemanticType::Text,
            DataValue::String("1".into()),
        );
        let convert = node(&mut document, "yssbi.value.convert");
        connect(&mut document, source, "value", convert, "input");
        let mut previous = convert;
        for _ in 0..12 {
            let reroute = node(&mut document, "yssbi.core.reroute");
            connect(&mut document, previous, "output", reroute, "input");
            previous = reroute;
        }
        let equal = node(&mut document, "yssbi.logic.equal");
        connect(&mut document, previous, "output", equal, "left");
        let other = node(&mut document, "yssbi.constant.get");
        constant(
            &mut document,
            other,
            SemanticType::Binary,
            DataValue::Bool(true),
        );
        connect(&mut document, other, "value", equal, "right");
        let mut cache = GraphSemanticCache::default();
        assert_eq!(
            resolve(&document, convert, &mut cache),
            exact("core.binary")
        );
        constant(
            &mut document,
            other,
            SemanticType::Numeric,
            DataValue::Integer(1),
        );
        assert_eq!(
            resolve(&document, convert, &mut cache),
            exact("core.numeric")
        );
        for semantic in [
            SemanticType::Categorical,
            SemanticType::Ordinal,
            SemanticType::Datetime,
            SemanticType::Identifier,
        ] {
            constant(
                &mut document,
                other,
                semantic,
                DataValue::String("1".into()),
            );
            assert_eq!(
                resolve(&document, convert, &mut cache),
                exact(semantic.type_id())
            );
        }
        let id = ConstantId::from_uuid(source.as_uuid());
        let constant = document.constants.get_mut(&id).unwrap();
        let element = ValueType::Scalar(SemanticType::Text);
        constant.data_type = ValueType::DataSeries(Box::new(element.clone()));
        constant.data_value = DataValue::String((r#"{"value":["1"]}"#).into());
        assert_eq!(
            resolve(&document, convert, &mut cache),
            TypeState::Exact(ResolvedType::Applied {
                constructor: "core.data_series".parse().unwrap(),
                arguments: Box::new([ResolvedType::Nominal("core.identifier".parse().unwrap())]),
            }),
            "comparison broadcasts the scalar without changing the converted series shape"
        );
    }
}
