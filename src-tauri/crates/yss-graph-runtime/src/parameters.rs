use yss_graph_editor::projection::{EditorParameterGroupModel, project_parameter_groups};
use yss_graph_editor::{MutationConflict, merge_parameters_with_registry};
use yss_node_protocol::{NodeTypeId, ParameterValues};

use crate::{GraphRuntimeState, localize_parameter_facts};

#[derive(Clone, Debug, PartialEq)]
pub struct NodeCreationForm {
    pub values: ParameterValues,
    pub groups: Box<[EditorParameterGroupModel]>,
    pub port_counts: yss_node_protocol::InitialPortCounts,
    pub ports: Box<[yss_node_catalog::NodeCreationPort]>,
}

impl GraphRuntimeState {
    /// Prepare an unconnected form without creating a node, history entry or graph draft.
    pub fn node_creation_form(
        &self,
        node_type: &NodeTypeId,
        values: ParameterValues,
        port_counts: yss_node_protocol::InitialPortCounts,
        locale: &str,
    ) -> Result<NodeCreationForm, MutationConflict> {
        let protocol = self
            .registry()
            .protocol(node_type)
            .filter(|protocol| !protocol.catalog.hidden && protocol.managed_role.is_none())
            .ok_or_else(|| {
                MutationConflict::CatalogDescriptorInvalid(
                    "node definition is not available for creation".into(),
                )
            })?;
        let values = merge_parameters_with_registry(
            self.registry(),
            protocol,
            &ParameterValues::new(),
            values,
        )?;
        let port_counts = protocol
            .interface
            .initial_port_counts(&port_counts)
            .map_err(|error| MutationConflict::InvalidEditorMutation(error.to_string().into()))?;
        let (mut groups, mut parameters) =
            yss_graph_analysis::project_parameter_form(protocol, &values);
        localize_parameter_facts(
            protocol,
            &self.components.catalog,
            locale,
            &mut groups,
            &mut parameters,
        );
        Ok(NodeCreationForm {
            values,
            groups: project_parameter_groups(&groups, &parameters),
            port_counts,
            ports: yss_node_catalog::node_creation_ports(protocol),
        })
    }
}
