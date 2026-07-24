use super::{
    InterfaceResolverId, ParameterKey, PortKey, SchemaResolverId, TypeClassId, TypeConstructorId,
    TypeId, TypeParameterId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeExpr {
    Concrete(TypeId),
    Generic(TypeParameterId),
    Applied {
        constructor: TypeConstructorId,
        arguments: Vec<TypeExpr>,
    },
    Union(Vec<TypeExpr>),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeTerm {
    Expr(TypeExpr),
    Port(PortKey),
    Parameter(ParameterKey),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeConstraint {
    Equal(TypeTerm, TypeTerm),
    Assignable(TypeTerm, TypeTerm),
    Implements(TypeTerm, TypeClassId),
    ElementOf(TypeTerm, TypeTerm),
    OneOf(TypeTerm, Vec<TypeTerm>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaExpr {
    Input(PortKey),
    Project {
        input: Box<SchemaExpr>,
        columns: ColumnSelectionExpr,
    },
    Append {
        inputs: Vec<SchemaExpr>,
    },
    Rename {
        input: Box<SchemaExpr>,
        mapping: RenameExpr,
    },
    Filter {
        input: Box<SchemaExpr>,
    },
    Derived {
        resolver: SchemaResolverId,
        dependencies: Vec<SchemaDependency>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnSelectionExpr {
    All,
    Explicit(Vec<SchemaColumnRef>),
    FromParameter(ParameterKey),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaColumnRef(pub Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenameExpr {
    Explicit(Vec<ColumnRename>),
    FromParameter(ParameterKey),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnRename {
    pub from: SchemaColumnRef,
    pub to: SchemaColumnRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaDependency {
    Port(PortKey),
    Parameter(ParameterKey),
    Interface(InterfaceResolverId),
}
