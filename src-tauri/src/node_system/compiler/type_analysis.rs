use crate::node_system::analysis::DiagnosticLocation;
use crate::node_system::document::{ConnectionId, NodeId, PortAddress};
use crate::node_system::protocol::{
    NodeProtocol, ParameterKey, PortKey, TypeClassId, TypeConstraint, TypeConstructorId, TypeExpr,
    TypeId, TypeParameterId, TypeTerm,
};
use std::collections::{BTreeMap, VecDeque};

pub(crate) struct TypeAnalysisIssue {
    pub code: &'static str,
    pub location: DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>,
    pub detail: String,
}

#[derive(Debug, Clone)]
enum VariableOrigin {
    Port,
    Generic(TypeParameterId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TypeValue {
    Variable(usize),
    Concrete(crate::node_system::protocol::TypeId),
    Applied {
        constructor: crate::node_system::protocol::TypeConstructorId,
        arguments: Vec<TypeValue>,
    },
    Union(Vec<TypeValue>),
    Unknown,
}

#[derive(Debug, Clone)]
enum ConstraintKind {
    Equal(TypeValue, TypeValue),
    Assignable(TypeValue, TypeValue),
    OneOf(TypeValue, Vec<TypeValue>),
    Implements(TypeValue, TypeClassId),
    ElementOf(TypeValue, TypeValue),
}

#[derive(Debug, Clone)]
struct Constraint {
    kind: ConstraintKind,
    location: DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>,
}

pub trait TypeEnvironment {
    fn concrete_implements(&self, value_type: &TypeId, class: &TypeClassId) -> Option<bool>;
    fn constructor_arity(&self, constructor: &TypeConstructorId) -> Option<usize>;

    fn applied_implements(
        &self,
        _constructor: &TypeConstructorId,
        _class: &TypeClassId,
    ) -> Option<bool> {
        None
    }
}

/// A deterministic constraint graph. Ports and node-scoped generic parameters are
/// variables; equality components are maintained by union-find and constraints are
/// consumed in insertion order by a worklist.
pub struct TypeConstraintGraph {
    variables: Vec<VariableOrigin>,
    port_variables: BTreeMap<PortAddress, usize>,
    constraints: Vec<Constraint>,
}

impl TypeConstraintGraph {
    pub fn new() -> Self {
        Self {
            variables: Vec::new(),
            port_variables: BTreeMap::new(),
            constraints: Vec::new(),
        }
    }

    pub(crate) fn add_node(
        &mut self,
        node_id: NodeId,
        protocol: &NodeProtocol,
        ports: impl Iterator<Item = PortAddress>,
    ) {
        let generic_variables: BTreeMap<_, _> = protocol
            .interface
            .type_parameters
            .iter()
            .map(|id| {
                (
                    id.clone(),
                    self.variable(VariableOrigin::Generic(id.clone())),
                )
            })
            .collect();
        let parameter_types: BTreeMap<_, _> = protocol
            .parameters
            .parameters
            .iter()
            .map(|parameter| (parameter.key.clone(), parameter.value_type.clone()))
            .collect();
        let ports = ports.collect::<Vec<_>>();

        for address in &ports {
            let variable = self.variable(VariableOrigin::Port);
            self.port_variables.insert(address.clone(), variable);
            if let Some(spec) = protocol
                .interface
                .ports
                .iter()
                .find(|spec| spec.key == *port_template(address))
            {
                let declared = instantiate(&spec.value_type, &generic_variables);
                self.constraints.push(Constraint {
                    kind: ConstraintKind::Equal(TypeValue::Variable(variable), declared),
                    location: DiagnosticLocation::Port(address.clone()),
                });
            }
        }

        for constraint in protocol.interface.type_constraints.iter() {
            self.add_protocol_constraint(
                node_id,
                constraint,
                &ports,
                &parameter_types,
                &generic_variables,
            );
        }
    }

    pub(crate) fn add_connection(
        &mut self,
        connection_id: ConnectionId,
        output: &PortAddress,
        input: &PortAddress,
    ) {
        let (Some(&source), Some(&target)) = (
            self.port_variables.get(output),
            self.port_variables.get(input),
        ) else {
            return;
        };
        self.constraints.push(Constraint {
            kind: ConstraintKind::Assignable(
                TypeValue::Variable(source),
                TypeValue::Variable(target),
            ),
            location: DiagnosticLocation::Connection(connection_id),
        });
    }

    pub(crate) fn add_literal(&mut self, address: &PortAddress, value_type: &TypeExpr) {
        let Some(&target) = self.port_variables.get(address) else {
            return;
        };
        self.constraints.push(Constraint {
            kind: ConstraintKind::Assignable(
                instantiate(value_type, &BTreeMap::new()),
                TypeValue::Variable(target),
            ),
            location: DiagnosticLocation::Port(address.clone()),
        });
    }

    pub(crate) fn solve(
        &self,
        environment: &dyn TypeEnvironment,
    ) -> (BTreeMap<PortAddress, TypeExpr>, Vec<TypeAnalysisIssue>) {
        let mut solver = Solver::new(self.variables.len());
        let mut worklist: VecDeque<_> = self.constraints.iter().cloned().collect();
        let mut predicates = VecDeque::new();
        let mut issues = Vec::new();
        while let Some(constraint) = worklist.pop_front() {
            if matches!(
                &constraint.kind,
                ConstraintKind::Implements(_, _) | ConstraintKind::ElementOf(_, _)
            ) {
                predicates.push_back(constraint);
                continue;
            }
            let result = match constraint.kind {
                ConstraintKind::Equal(left, right) => solver.equal(left, right),
                ConstraintKind::Assignable(source, target) => solver.assignable(source, target),
                ConstraintKind::OneOf(subject, alternatives) => {
                    solver.one_of(subject, alternatives)
                }
                ConstraintKind::Implements(_, _) | ConstraintKind::ElementOf(_, _) => {
                    Err("internal type predicate scheduling error".into())
                }
            };
            push_issue(&mut issues, constraint.location, result);
        }
        while let Some(constraint) = predicates.pop_front() {
            let result = match constraint.kind {
                ConstraintKind::Implements(value, class) => {
                    solver.implements(value, &class, environment)
                }
                ConstraintKind::ElementOf(element, collection) => {
                    solver.element_of(element, collection, environment)
                }
                _ => unreachable!("only predicate constraints are deferred"),
            };
            push_issue(&mut issues, constraint.location, result);
        }

        let facts = self
            .port_variables
            .iter()
            .map(|(address, &variable)| {
                (
                    address.clone(),
                    solver.fact(TypeValue::Variable(variable), &self.variables),
                )
            })
            .collect();
        (facts, issues)
    }

    fn variable(&mut self, origin: VariableOrigin) -> usize {
        let index = self.variables.len();
        self.variables.push(origin);
        index
    }

    fn add_protocol_constraint(
        &mut self,
        node_id: NodeId,
        constraint: &TypeConstraint,
        ports: &[PortAddress],
        parameter_types: &BTreeMap<ParameterKey, TypeExpr>,
        generics: &BTreeMap<TypeParameterId, usize>,
    ) {
        let location = DiagnosticLocation::Node(node_id);
        match constraint {
            TypeConstraint::Equal(left, right) => {
                self.expand_binary(
                    left,
                    right,
                    ports,
                    parameter_types,
                    generics,
                    |left, right| ConstraintKind::Equal(left, right),
                    location,
                );
            }
            TypeConstraint::Assignable(left, right) => {
                self.expand_binary(
                    left,
                    right,
                    ports,
                    parameter_types,
                    generics,
                    |left, right| ConstraintKind::Assignable(left, right),
                    location,
                );
            }
            TypeConstraint::OneOf(subject, alternatives) => {
                for subject in self.resolve_term(subject, ports, parameter_types, generics) {
                    let candidates = alternatives
                        .iter()
                        .flat_map(|term| self.resolve_term(term, ports, parameter_types, generics))
                        .collect();
                    self.constraints.push(Constraint {
                        kind: ConstraintKind::OneOf(subject, candidates),
                        location: location.clone(),
                    });
                }
            }
            TypeConstraint::Implements(term, class) => {
                for value in self.resolve_term(term, ports, parameter_types, generics) {
                    self.constraints.push(Constraint {
                        kind: ConstraintKind::Implements(value, class.clone()),
                        location: location.clone(),
                    });
                }
            }
            TypeConstraint::ElementOf(element, collection) => self.expand_binary(
                element,
                collection,
                ports,
                parameter_types,
                generics,
                ConstraintKind::ElementOf,
                location,
            ),
        }
    }

    fn expand_binary(
        &mut self,
        left: &TypeTerm,
        right: &TypeTerm,
        ports: &[PortAddress],
        parameter_types: &BTreeMap<ParameterKey, TypeExpr>,
        generics: &BTreeMap<TypeParameterId, usize>,
        make: impl Fn(TypeValue, TypeValue) -> ConstraintKind,
        location: DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>,
    ) {
        let left = self.resolve_term(left, ports, parameter_types, generics);
        let right = self.resolve_term(right, ports, parameter_types, generics);
        for left in left {
            for right in &right {
                self.constraints.push(Constraint {
                    kind: make(left.clone(), right.clone()),
                    location: location.clone(),
                });
            }
        }
    }

    fn resolve_term(
        &self,
        term: &TypeTerm,
        ports: &[PortAddress],
        parameter_types: &BTreeMap<ParameterKey, TypeExpr>,
        generics: &BTreeMap<TypeParameterId, usize>,
    ) -> Vec<TypeValue> {
        match term {
            TypeTerm::Expr(expr) => vec![instantiate(expr, generics)],
            TypeTerm::Port(key) => ports
                .iter()
                .filter(|address| port_template(address) == key)
                .filter_map(|address| self.port_variables.get(address).copied())
                .map(TypeValue::Variable)
                .collect(),
            TypeTerm::Parameter(key) => parameter_types
                .get(key)
                .map(|expr| vec![instantiate(expr, generics)])
                .unwrap_or_default(),
        }
    }
}

fn port_template(address: &PortAddress) -> &PortKey {
    match &address.port {
        crate::node_system::document::PortRef::Declared { key } => key,
        crate::node_system::document::PortRef::Instance { template, .. } => template,
    }
}

fn instantiate(expr: &TypeExpr, generics: &BTreeMap<TypeParameterId, usize>) -> TypeValue {
    match expr {
        TypeExpr::Concrete(id) => TypeValue::Concrete(id.clone()),
        TypeExpr::Generic(id) => generics
            .get(id)
            .copied()
            .map(TypeValue::Variable)
            .unwrap_or(TypeValue::Unknown),
        TypeExpr::Applied {
            constructor,
            arguments,
        } => TypeValue::Applied {
            constructor: constructor.clone(),
            arguments: arguments
                .iter()
                .map(|argument| instantiate(argument, generics))
                .collect(),
        },
        TypeExpr::Union(values) => TypeValue::Union(
            values
                .iter()
                .map(|value| instantiate(value, generics))
                .collect(),
        ),
        TypeExpr::Unknown => TypeValue::Unknown,
    }
}

#[derive(Clone)]
struct Solver {
    parent: Vec<usize>,
    rank: Vec<u8>,
    bindings: Vec<Option<TypeValue>>,
}

impl Solver {
    fn new(count: usize) -> Self {
        Self {
            parent: (0..count).collect(),
            rank: vec![0; count],
            bindings: vec![None; count],
        }
    }

    fn root(&mut self, variable: usize) -> usize {
        if self.parent[variable] != variable {
            self.parent[variable] = self.root(self.parent[variable]);
        }
        self.parent[variable]
    }

    fn resolve(&mut self, value: TypeValue) -> TypeValue {
        match value {
            TypeValue::Variable(variable) => {
                let root = self.root(variable);
                match self.bindings[root].clone() {
                    Some(binding) => self.resolve(binding),
                    None => TypeValue::Variable(root),
                }
            }
            TypeValue::Applied {
                constructor,
                arguments,
            } => TypeValue::Applied {
                constructor,
                arguments: arguments
                    .into_iter()
                    .map(|argument| self.resolve(argument))
                    .collect(),
            },
            TypeValue::Union(values) => TypeValue::Union(
                values
                    .into_iter()
                    .map(|value| self.resolve(value))
                    .collect(),
            ),
            value => value,
        }
    }

    fn equal(&mut self, left: TypeValue, right: TypeValue) -> Result<(), String> {
        let left = self.resolve(left);
        let right = self.resolve(right);
        match (left, right) {
            (TypeValue::Unknown, _) | (_, TypeValue::Unknown) => Ok(()),
            (TypeValue::Variable(left), TypeValue::Variable(right)) => self.union(left, right),
            (TypeValue::Variable(variable), value) | (value, TypeValue::Variable(variable)) => {
                self.bind(variable, value)
            }
            (TypeValue::Concrete(left), TypeValue::Concrete(right)) if left == right => Ok(()),
            (
                TypeValue::Applied {
                    constructor: left_constructor,
                    arguments: left_arguments,
                },
                TypeValue::Applied {
                    constructor: right_constructor,
                    arguments: right_arguments,
                },
            ) if left_constructor == right_constructor
                && left_arguments.len() == right_arguments.len() =>
            {
                for (left, right) in left_arguments.into_iter().zip(right_arguments) {
                    self.equal(left, right)?;
                }
                Ok(())
            }
            (TypeValue::Union(left), TypeValue::Union(right)) if left.len() == right.len() => {
                for (left, right) in left.into_iter().zip(right) {
                    self.equal(left, right)?;
                }
                Ok(())
            }
            (left, right) => Err(format!(
                "{} is not equal to {}",
                display_type(&left),
                display_type(&right)
            )),
        }
    }

    fn assignable(&mut self, source: TypeValue, target: TypeValue) -> Result<(), String> {
        let source = self.resolve(source);
        let target = self.resolve(target);
        match (source, target) {
            (TypeValue::Unknown, _) | (_, TypeValue::Unknown) => Ok(()),
            (TypeValue::Variable(left), TypeValue::Variable(right)) => self.union(left, right),
            (TypeValue::Variable(variable), value) | (value, TypeValue::Variable(variable)) => {
                self.bind(variable, value)
            }
            (TypeValue::Concrete(source), TypeValue::Concrete(target)) if source == target => {
                Ok(())
            }
            (
                TypeValue::Applied {
                    constructor: source_constructor,
                    arguments: source_arguments,
                },
                TypeValue::Applied {
                    constructor: target_constructor,
                    arguments: target_arguments,
                },
            ) if source_constructor == target_constructor
                && source_arguments.len() == target_arguments.len() =>
            {
                for (source, target) in source_arguments.into_iter().zip(target_arguments) {
                    self.assignable(source, target)?;
                }
                Ok(())
            }
            (TypeValue::Union(sources), target) => {
                for source in sources {
                    self.assignable(source, target.clone())?;
                }
                Ok(())
            }
            (source, TypeValue::Union(targets)) => {
                for target in targets {
                    let mut trial = self.clone();
                    if trial.assignable(source.clone(), target).is_ok() {
                        *self = trial;
                        return Ok(());
                    }
                }
                Err(format!(
                    "{} matches no union alternative",
                    display_type(&source)
                ))
            }
            (source, target) => Err(format!(
                "{} is not assignable to {}",
                display_type(&source),
                display_type(&target)
            )),
        }
    }

    fn one_of(&mut self, subject: TypeValue, alternatives: Vec<TypeValue>) -> Result<(), String> {
        let subject = self.resolve(subject);
        if alternatives.is_empty() {
            return Err("one-of constraint has no alternatives".into());
        }
        if alternatives.len() == 1 {
            return self.equal(subject, alternatives.into_iter().next().unwrap());
        }
        if matches!(subject, TypeValue::Variable(_)) {
            return self.equal(subject, TypeValue::Union(alternatives));
        }
        for alternative in alternatives {
            let mut trial = self.clone();
            if trial.equal(subject.clone(), alternative).is_ok() {
                *self = trial;
                return Ok(());
            }
        }
        Err(format!(
            "{} matches no one-of alternative",
            display_type(&subject)
        ))
    }

    fn implements(
        &mut self,
        value: TypeValue,
        class: &TypeClassId,
        environment: &dyn TypeEnvironment,
    ) -> Result<(), String> {
        let value = self.resolve(value);
        match value {
            TypeValue::Unknown | TypeValue::Variable(_) => Ok(()),
            TypeValue::Concrete(value_type) => {
                match environment.concrete_implements(&value_type, class) {
                    Some(true) => Ok(()),
                    Some(false) => Err(format!("{value_type} does not implement {class}")),
                    None => Err(format!("unknown concrete type '{value_type}'")),
                }
            }
            TypeValue::Applied {
                constructor,
                arguments,
            } => {
                validate_constructor(environment, &constructor, arguments.len())?;
                match environment.applied_implements(&constructor, class) {
                    Some(true) => Ok(()),
                    Some(false) => Err(format!(
                        "{} does not implement {class}",
                        display_type(&TypeValue::Applied {
                            constructor,
                            arguments,
                        })
                    )),
                    None => Ok(()),
                }
            }
            TypeValue::Union(values) => {
                for value in values {
                    self.implements(value, class, environment)?;
                }
                Ok(())
            }
        }
    }

    fn element_of(
        &mut self,
        element: TypeValue,
        collection: TypeValue,
        environment: &dyn TypeEnvironment,
    ) -> Result<(), String> {
        let element = self.resolve(element);
        let collection = self.resolve(collection);
        if let TypeValue::Union(elements) = element {
            for element in elements {
                self.element_of(element, collection.clone(), environment)?;
            }
            return Ok(());
        }
        match collection {
            TypeValue::Unknown | TypeValue::Variable(_) => Ok(()),
            TypeValue::Applied {
                constructor,
                arguments,
            } => {
                validate_constructor(environment, &constructor, arguments.len())?;
                self.one_of(element, arguments)
            }
            TypeValue::Union(collections) => {
                for collection in collections {
                    let mut trial = self.clone();
                    if trial
                        .element_of(element.clone(), collection, environment)
                        .is_ok()
                    {
                        *self = trial;
                        return Ok(());
                    }
                }
                Err(format!(
                    "{} is not an element of any union alternative",
                    display_type(&element)
                ))
            }
            TypeValue::Concrete(value_type) => Err(format!(
                "concrete type '{value_type}' has no registered element types"
            )),
        }
    }

    fn union(&mut self, left: usize, right: usize) -> Result<(), String> {
        let mut left = self.root(left);
        let mut right = self.root(right);
        if left == right {
            return Ok(());
        }
        if self.rank[left] < self.rank[right]
            || (self.rank[left] == self.rank[right] && left > right)
        {
            std::mem::swap(&mut left, &mut right);
        }
        let left_binding = self.bindings[left].take();
        let right_binding = self.bindings[right].take();
        self.parent[right] = left;
        if self.rank[left] == self.rank[right] {
            self.rank[left] += 1;
        }
        match (left_binding, right_binding) {
            (Some(left_value), Some(right_value)) => {
                self.bindings[left] = Some(left_value.clone());
                self.equal(left_value, right_value)
            }
            (binding, None) | (None, binding) => {
                self.bindings[left] = binding;
                Ok(())
            }
        }
    }

    fn bind(&mut self, variable: usize, value: TypeValue) -> Result<(), String> {
        let root = self.root(variable);
        let value = self.resolve(value);
        if value == TypeValue::Variable(root) {
            return Ok(());
        }
        if occurs(root, &value) {
            return Err("recursive type binding".into());
        }
        if let Some(existing) = self.bindings[root].clone() {
            self.equal(existing, value)
        } else {
            self.bindings[root] = Some(value);
            Ok(())
        }
    }

    fn fact(&mut self, value: TypeValue, origins: &[VariableOrigin]) -> TypeExpr {
        match self.resolve(value) {
            TypeValue::Variable(variable) => {
                let root = self.root(variable);
                origins
                    .iter()
                    .enumerate()
                    .find_map(|(index, origin)| {
                        (self.root(index) == root)
                            .then_some(origin)
                            .and_then(|origin| match origin {
                                VariableOrigin::Generic(id) => Some(TypeExpr::Generic(id.clone())),
                                VariableOrigin::Port => None,
                            })
                    })
                    .unwrap_or(TypeExpr::Unknown)
            }
            TypeValue::Concrete(id) => TypeExpr::Concrete(id),
            TypeValue::Applied {
                constructor,
                arguments,
            } => TypeExpr::Applied {
                constructor,
                arguments: arguments
                    .into_iter()
                    .map(|argument| self.fact(argument, origins))
                    .collect(),
            },
            TypeValue::Union(values) => TypeExpr::Union(
                values
                    .into_iter()
                    .map(|value| self.fact(value, origins))
                    .collect(),
            ),
            TypeValue::Unknown => TypeExpr::Unknown,
        }
    }
}

fn push_issue(
    issues: &mut Vec<TypeAnalysisIssue>,
    location: DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>,
    result: Result<(), String>,
) {
    if let Err(detail) = result {
        issues.push(TypeAnalysisIssue {
            code: "compiler.type.incompatible",
            location,
            detail,
        });
    }
}

fn validate_constructor(
    environment: &dyn TypeEnvironment,
    constructor: &TypeConstructorId,
    actual_arity: usize,
) -> Result<(), String> {
    match environment.constructor_arity(constructor) {
        Some(expected_arity) if expected_arity == actual_arity => Ok(()),
        Some(expected_arity) => Err(format!(
            "type constructor '{constructor}' expects {expected_arity} arguments, found {actual_arity}"
        )),
        None => Err(format!("unknown type constructor '{constructor}'")),
    }
}

fn occurs(variable: usize, value: &TypeValue) -> bool {
    match value {
        TypeValue::Variable(candidate) => *candidate == variable,
        TypeValue::Applied { arguments, .. } | TypeValue::Union(arguments) => {
            arguments.iter().any(|argument| occurs(variable, argument))
        }
        TypeValue::Concrete(_) | TypeValue::Unknown => false,
    }
}

fn display_type(value: &TypeValue) -> String {
    match value {
        TypeValue::Variable(variable) => format!("?{variable}"),
        TypeValue::Concrete(id) => id.to_string(),
        TypeValue::Applied {
            constructor,
            arguments,
        } => format!(
            "{}<{}>",
            constructor,
            arguments
                .iter()
                .map(display_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeValue::Union(values) => values
            .iter()
            .map(display_type)
            .collect::<Vec<_>>()
            .join(" | "),
        TypeValue::Unknown => "unknown".into(),
    }
}
