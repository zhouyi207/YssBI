use std::sync::Arc;

use thiserror::Error;

use super::session_slot::{ApplicationState, SessionCaptureError};
use crate::execution::plan::{PlanGraphId, PlanOutputRef, PlanPortAddress};
use crate::execution::result::{ExecutionResultQueryError, PinResultHistorySnapshot, ResultId};
use crate::graph_document::{GraphResourcePath, PortAddress};

pub struct ResultPinQuery {
    graph_path: GraphResourcePath,
    output: PortAddress,
}

impl ResultPinQuery {
    pub fn new(graph_path: GraphResourcePath, output: PortAddress) -> Self {
        Self { graph_path, output }
    }
}

#[derive(Debug, Error)]
pub enum ResultQueryApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error(transparent)]
    Execution(#[from] ExecutionResultQueryError),
}

impl ApplicationState {
    pub fn query_result(
        &self,
        result_id: ResultId,
    ) -> Result<Option<Arc<crate::execution::result::StoredResult>>, ResultQueryApplicationError>
    {
        let captured = self.capture_session()?;
        Ok(captured.execution().query_result(result_id))
    }

    pub fn query_pin_result_history(
        &self,
        query: ResultPinQuery,
    ) -> Result<Box<[PinResultHistorySnapshot]>, ResultQueryApplicationError> {
        let captured = self.capture_session()?;
        let output = PlanOutputRef::new(
            PlanGraphId::from_existing(query.graph_path.as_str().to_owned().into_boxed_str()),
            PlanPortAddress::from_existing(query.output.to_string().into_boxed_str()),
        );
        captured
            .execution()
            .query_pin_result_history(&output)
            .map_err(ResultQueryApplicationError::Execution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::plan::{PlanGraphId, PlanPortAddress};

    #[test]
    fn result_query_maps_opaque_graph_and_port_identities() {
        let graph =
            GraphResourcePath::new("events/main.yssbi-event").expect("test graph path is valid");
        let address = PortAddress::declared(
            crate::graph_document::NodeId::from_uuid(uuid::Uuid::nil()),
            yss_graph_protocol::PortKey::new("result").expect("valid port key"),
        );
        let query = ResultPinQuery::new(graph, address.clone());
        let plan = PlanOutputRef::new(
            PlanGraphId::from_existing(query.graph_path.as_str().to_owned().into_boxed_str()),
            PlanPortAddress::from_existing(query.output.to_string().into_boxed_str()),
        );
        assert_eq!(plan.graph().as_str(), "events/main.yssbi-event");
        assert_eq!(plan.port().as_str(), address.to_string());
    }
}
