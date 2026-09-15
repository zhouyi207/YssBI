use std::collections::VecDeque;
use yss_graph_analysis::{GraphAnalysis, GraphSemanticCache};
use yss_graph_document::GraphResourcePath;

// One latest snapshot per graph; cache residency never keeps every opened graph
// alive for the lifetime of a project. A concurrent miss may recompute, but no
// parsing, hashing or localization takes place under the cache lock.
const MAX_CACHED_GRAPHS: usize = 16;

pub(super) struct CachedGraphAnalysis {
    pub document_fingerprint: [u8; 32],
    pub dependency_fingerprint: [u8; 32],
    pub analysis: GraphAnalysis,
}

#[derive(Default)]
pub(super) struct GraphResolutionCache {
    pub nodes: GraphSemanticCache,
    pub analysis: Option<CachedGraphAnalysis>,
}

#[derive(Default)]
pub(super) struct GraphResolutionCaches {
    entries: VecDeque<(GraphResourcePath, GraphResolutionCache)>,
}

impl GraphResolutionCaches {
    pub fn take(&mut self, graph: &GraphResourcePath) -> GraphResolutionCache {
        self.entries
            .iter()
            .position(|(path, _)| path == graph)
            .and_then(|index| self.entries.remove(index))
            .map(|(_, cache)| cache)
            .unwrap_or_default()
    }

    pub fn put(
        &mut self,
        graph: GraphResourcePath,
        cache: GraphResolutionCache,
    ) -> Option<GraphResolutionCache> {
        let retired = self
            .entries
            .iter()
            .position(|(path, _)| path == &graph)
            .and_then(|index| self.entries.remove(index))
            .or_else(|| {
                (self.entries.len() >= MAX_CACHED_GRAPHS)
                    .then(|| self.entries.pop_front())
                    .flatten()
            });
        self.entries.push_back((graph, cache));
        retired.map(|(_, cache)| cache)
    }
}
