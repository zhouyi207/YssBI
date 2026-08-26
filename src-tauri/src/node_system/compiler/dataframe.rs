use super::CompilerDiagnostic;
use super::dynamic_interface::{
    InterfaceResolver, InterfaceResolverDiagnostic, InterfaceResolverError,
    InterfaceResolverMember, InterfaceResolverOutput, InterfaceResolverRequest,
    InterfaceResolverSet, SchemaFieldIdentityGuarantee,
};
use crate::graph_document::{
    DynamicMemberLocator, PortAddress, SchemaFieldIdentity, SchemaSourceIdentity,
};
use crate::node_system::protocol::{
    InterfaceResolverId, PortKey, RelationalScalarType, SchemaField, TypeExpr, TypeId,
    data_series_type,
};
use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

pub const DATAFRAME_COLUMNS_RESOLVER: &str = "yssbi.dataframe.interface.columns";
const DATAFRAME_INPUT: &str = "dataframe";

pub(crate) struct DataframeColumnsResolver;

impl InterfaceResolver for DataframeColumnsResolver {
    fn schema_dependencies(&self) -> &[PortKey] {
        static DEPENDENCIES: OnceLock<Box<[PortKey]>> = OnceLock::new();
        DEPENDENCIES.get_or_init(|| {
            vec![PortKey::new(DATAFRAME_INPUT).expect("built-in port key is valid")]
                .into_boxed_slice()
        })
    }

    fn resolve(
        &self,
        request: InterfaceResolverRequest<'_>,
    ) -> Result<InterfaceResolverOutput, InterfaceResolverError> {
        let input = PortAddress::declared(
            request.node_id,
            PortKey::new(DATAFRAME_INPUT).expect("built-in port key is valid"),
        );
        let schema = request.resolved_schemas.get(&input).ok_or_else(|| {
            InterfaceResolverError::new("dataframe input schema was not resolved")
        })?;
        let mut locators = BTreeSet::new();
        let mut members = Vec::with_capacity(schema.fields.len());
        let mut diagnostics = Vec::new();

        for field in &schema.fields {
            let (source, identity, guarantee) = match &field.lineage {
                Some(lineage) => (
                    lineage.source.clone(),
                    lineage.field.clone(),
                    SchemaFieldIdentityGuarantee::Stable,
                ),
                None => (
                    format!("snapshot:{}:{}", request.node_id, request.template.key).into(),
                    field.name.0.clone(),
                    SchemaFieldIdentityGuarantee::SnapshotScoped,
                ),
            };
            if !locators.insert((source.clone(), identity.clone())) {
                return Err(InterfaceResolverError::new(format!(
                    "duplicate dataframe schema field locator '{source}/{identity}'"
                )));
            }

            let locator = DynamicMemberLocator::SchemaField {
                source: SchemaSourceIdentity::new(source),
                field: SchemaFieldIdentity::new(identity),
            };
            let (element_type, diagnostic) = dataframe_field_type(field);
            let value_type = match element_type {
                TypeExpr::Unknown => TypeExpr::Unknown,
                element_type => data_series_type(element_type),
            };
            if let Some(diagnostic) = diagnostic {
                diagnostics.push(InterfaceResolverDiagnostic {
                    locator: locator.clone(),
                    diagnostic,
                });
            }
            members.push(InterfaceResolverMember {
                basis: request.basis.clone(),
                locator,
                label: field.name.0.to_string(),
                value_type,
                identity: guarantee,
            });
        }

        Ok(InterfaceResolverOutput {
            members: members.into_boxed_slice(),
            diagnostics: diagnostics.into_boxed_slice(),
        })
    }
}

fn dataframe_field_type(field: &SchemaField) -> (TypeExpr, Option<CompilerDiagnostic>) {
    let concrete = |id: &str| TypeExpr::Concrete(TypeId::new(id).expect("built-in type ID"));
    match field.scalar_type {
        RelationalScalarType::Boolean => (concrete("core.bool"), None),
        RelationalScalarType::Int64 => (concrete("core.int64"), None),
        RelationalScalarType::Float64 => (concrete("core.float64"), None),
        RelationalScalarType::String => (concrete("core.string"), None),
        RelationalScalarType::Date => (concrete("core.date"), None),
        RelationalScalarType::DateTime => (concrete("core.datetime"), None),
        RelationalScalarType::Unknown => (
            TypeExpr::Unknown,
            Some(CompilerDiagnostic::DataframeFieldTypeUnsupported {
                column: field.name.0.clone(),
                schema_type: "unknown".into(),
                reason: "no concrete node type is registered for the schema field".into(),
            }),
        ),
    }
}

pub(super) fn install_dataframe_interface_resolvers(set: &mut InterfaceResolverSet) {
    set.insert(
        InterfaceResolverId::new(DATAFRAME_COLUMNS_RESOLVER)
            .expect("built-in resolver ID is valid"),
        Arc::new(DataframeColumnsResolver),
    )
    .expect("built-in dataframe resolver IDs are unique");
}
