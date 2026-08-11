use super::dynamic_interface::{
    InterfaceResolver, InterfaceResolverError, InterfaceResolverMember, InterfaceResolverRequest,
    InterfaceResolverSet, SchemaFieldIdentityGuarantee,
};
use crate::node_system::document::{
    DynamicMemberLocator, PortAddress, SchemaFieldIdentity, SchemaSourceIdentity,
};
use crate::node_system::protocol::{InterfaceResolverId, PortKey};
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
    ) -> Result<Box<[InterfaceResolverMember]>, InterfaceResolverError> {
        let input = PortAddress::declared(
            request.node_id,
            PortKey::new(DATAFRAME_INPUT).expect("built-in port key is valid"),
        );
        let schema = request.resolved_schemas.get(&input).ok_or_else(|| {
            InterfaceResolverError::new("dataframe input schema was not resolved")
        })?;
        let mut locators = BTreeSet::new();
        let mut members = Vec::with_capacity(schema.fields.len());

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

            members.push(InterfaceResolverMember {
                basis: request.basis.clone(),
                locator: DynamicMemberLocator::SchemaField {
                    source: SchemaSourceIdentity(source),
                    field: SchemaFieldIdentity(identity),
                },
                label: field.name.0.to_string(),
                identity: guarantee,
            });
        }

        Ok(members.into_boxed_slice())
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
