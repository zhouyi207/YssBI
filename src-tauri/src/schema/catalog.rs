use crate::application::catalog_query::CatalogQueryResult;
use crate::node_system::catalog::LocalizedCatalogDto;

impl From<CatalogQueryResult> for LocalizedCatalogDto {
    fn from(result: CatalogQueryResult) -> Self {
        let (project_instance_id, registry_fingerprint, resource_publication_revision, catalog) =
            result.into_transport_parts().into_fields();

        catalog.into_dto(
            project_instance_id.as_ref(),
            registry_fingerprint.as_ref().to_owned(),
            resource_publication_revision,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::catalog::{
        CatalogResourcePath, LocalizedCatalog, LocalizedCatalogItemDto, LocalizedCategoryDto,
        NodeCreationDescriptor, ResourceBoundCreateArgsDto,
    };
    use crate::node_system::protocol::NodeTypeId;
    use crate::project::ResourceRevision;
    use serde_json::json;

    #[test]
    fn catalog_mapper_preserves_metadata_and_resource_bound_wire_shape() {
        let node_type_id = NodeTypeId::new("yssbi.project.function.call").unwrap();
        let resource_path = CatalogResourcePath::new("functions/Opaque.yssbi-function");
        let resource_revision = ResourceRevision::new(7);
        let catalog = LocalizedCatalog {
            locale: "zh-CN".into(),
            categories: vec![LocalizedCategoryDto {
                category_id: "functions".into(),
                parent_category_id: None,
                order: 3,
                title: "Functions".into(),
                search_text: "functions".into(),
            }],
            items: vec![LocalizedCatalogItemDto {
                node_type_id: node_type_id.as_str().into(),
                title: "Opaque Function".into(),
                documentation: None,
                category_id: "functions".into(),
                icon_id: "function".into(),
                style_id: "default".into(),
                aliases: Vec::new(),
                technical_terms: Vec::new(),
                backend_search_text: Vec::new(),
                resource_names: vec!["Opaque Function".into()],
                ports: Vec::new(),
                parameters: Vec::new(),
                resource_path: Some(resource_path.clone()),
                resource_revision: Some(resource_revision),
                creation: NodeCreationDescriptor::ResourceBound {
                    node_type_id: node_type_id.clone(),
                    resource_path: resource_path.clone(),
                    resource_revision,
                    create_args: ResourceBoundCreateArgsDto::Function,
                },
            }],
        };
        let wire = serde_json::to_value(LocalizedCatalogDto::from(CatalogQueryResult::new(
            "project-1".into(),
            "registry-1".into(),
            11,
            catalog,
        )))
        .unwrap();

        assert_eq!(wire["projectInstanceId"], "project-1");
        assert_eq!(wire["registryFingerprint"], "registry-1");
        assert_eq!(wire["resourcePublicationRevision"], 11);
        assert_eq!(wire["locale"], "zh-CN");
        assert_eq!(wire["categories"][0]["categoryId"], "functions");
        assert_eq!(
            wire["items"][0]["resourcePath"],
            "functions/Opaque.yssbi-function"
        );
        assert_eq!(wire["items"][0]["resourceRevision"], 7);
        assert_eq!(
            wire["items"][0]["creation"],
            json!({
                "kind": "resourceBound",
                "nodeTypeId": "yssbi.project.function.call",
                "resourcePath": "functions/Opaque.yssbi-function",
                "resourceRevision": 7,
                "createArgs": { "kind": "function" },
            })
        );
        assert!(wire.get("project_instance_id").is_none());
        assert!(wire.get("registry_fingerprint").is_none());
        assert!(wire.get("resource_publication_revision").is_none());
    }
}
