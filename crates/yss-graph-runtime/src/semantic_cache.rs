use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Weak;
use yss_graph_analysis::{GraphAnalysis, GraphSemanticCache};
use yss_graph_document::{GraphDocument, GraphResourcePath};

// One latest snapshot per graph; cache residency never keeps every opened graph
// alive for the lifetime of a project. A concurrent miss may recompute, but no
// parsing, hashing or localization takes place under the cache lock.
const MAX_CACHED_GRAPHS: usize = 16;

pub(super) struct CachedGraphAnalysis {
    pub document_identity: Weak<GraphDocument>,
    pub document_fingerprint: [u8; 32],
    pub dependency_fingerprint: [u8; 32],
    pub observation_fingerprint: [u8; 32],
    pub analysis: GraphAnalysis,
}

#[derive(Default)]
pub(super) struct GraphResolutionCache {
    pub nodes: GraphSemanticCache,
    pub analysis: Option<CachedGraphAnalysis>,
}

pub(super) struct GraphResolutionCaches {
    entries: LruCache<GraphResourcePath, GraphResolutionCache>,
}

impl Default for GraphResolutionCaches {
    fn default() -> Self {
        Self {
            entries: LruCache::new(NonZeroUsize::new(MAX_CACHED_GRAPHS).unwrap()),
        }
    }
}

impl GraphResolutionCaches {
    pub fn take(&mut self, graph: &GraphResourcePath) -> GraphResolutionCache {
        self.entries.pop(graph).unwrap_or_default()
    }

    pub fn put(
        &mut self,
        graph: GraphResourcePath,
        cache: GraphResolutionCache,
    ) -> Option<GraphResolutionCache> {
        // Return both replaced and evicted snapshots so the caller drops them off-lock.
        self.entries.push(graph, cache).map(|(_, cache)| cache)
    }
}
