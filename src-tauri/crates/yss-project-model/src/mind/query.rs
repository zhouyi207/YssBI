//! Disposable indexes over the canonical topic tree, never persisted query state.
use super::*;

#[derive(Debug)]
pub struct MindTree<'a> {
    document: &'a MindDocument,
    nodes: HashMap<&'a str, &'a MindNode>,
    children: HashMap<&'a str, Vec<&'a str>>,
}

#[derive(Debug)]
pub struct MindOutlineEntry<'a> {
    pub node: &'a MindNode,
    pub depth: usize,
    pub child_count: usize,
}

#[derive(Debug)]
pub struct MindOutlinePage<'a> {
    pub entries: Vec<MindOutlineEntry<'a>>,
    /// Eligible topics within the requested depth, including the selected root.
    pub total: usize,
    pub subtree_count: usize,
}

#[derive(Debug)]
pub struct MindTopicDetail<'a> {
    pub node: &'a MindNode,
    pub path: Vec<&'a str>,
    pub child_ids: Vec<&'a str>,
}

#[derive(Debug)]
pub struct MindSearchPage<'a> {
    pub topics: Vec<MindTopicDetail<'a>>,
    pub total: usize,
}

impl MindDocument {
    pub fn tree(&self) -> Result<MindTree<'_>, String> {
        self.validate()?;
        let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
        for node in &self.nodes {
            if let Some(parent) = &node.parent_id {
                children.entry(parent).or_default().push(&node.id);
            }
        }
        Ok(MindTree {
            document: self,
            nodes: self
                .nodes
                .iter()
                .map(|node| (node.id.as_str(), node))
                .collect(),
            children,
        })
    }
}

impl<'a> MindTree<'a> {
    pub fn node(&self, id: &str) -> Result<&'a MindNode, String> {
        self.nodes
            .get(id)
            .copied()
            .ok_or_else(|| "mind node not found".into())
    }

    pub fn outline(
        &self,
        root: Option<&str>,
        depth: usize,
        offset: usize,
        limit: usize,
    ) -> Result<MindOutlinePage<'a>, String> {
        if limit == 0 {
            return Err("empty mind query page".into());
        }
        let walk = self.walk(root.unwrap_or(&self.document.root_id))?;
        let subtree_count = walk.len();
        let selected = walk
            .into_iter()
            .filter(|(_, level)| *level <= depth)
            .collect::<Vec<_>>();
        let total = selected.len();
        let entries = selected
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|(node, depth)| MindOutlineEntry {
                node,
                depth,
                child_count: self.children.get(node.id.as_str()).map_or(0, Vec::len),
            })
            .collect();
        Ok(MindOutlinePage {
            entries,
            total,
            subtree_count,
        })
    }

    pub fn find(
        &self,
        root: Option<&str>,
        query: &str,
        offset: usize,
        limit: usize,
    ) -> Result<MindSearchPage<'a>, String> {
        if query.is_empty() || limit == 0 {
            return Err("empty mind search".into());
        }
        let query = query.to_lowercase();
        let matching = self
            .walk(root.unwrap_or(&self.document.root_id))?
            .into_iter()
            .map(|(node, _)| node)
            .filter(|node| node.content.to_lowercase().contains(&query))
            .collect::<Vec<_>>();
        let total = matching.len();
        let topics = matching
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|node| self.detail(node))
            .collect();
        Ok(MindSearchPage { topics, total })
    }

    pub fn inspect(&self, ids: &[String]) -> Result<Vec<MindTopicDetail<'a>>, String> {
        let mut seen = HashSet::new();
        ids.iter()
            .map(|id| {
                if !seen.insert(id) {
                    return Err("duplicate mind topic".into());
                }
                self.node(id).map(|node| self.detail(node))
            })
            .collect()
    }

    fn detail(&self, node: &'a MindNode) -> MindTopicDetail<'a> {
        let mut path = vec![node.id.as_str()];
        let mut current = node;
        while let Some(parent) = current.parent_id.as_deref() {
            path.push(parent);
            current = self.nodes[parent];
        }
        path.reverse();
        MindTopicDetail {
            node,
            path,
            child_ids: self
                .children
                .get(node.id.as_str())
                .cloned()
                .unwrap_or_default(),
        }
    }

    pub(super) fn walk(&self, root: &str) -> Result<Vec<(&'a MindNode, usize)>, String> {
        let root = self.node(root)?;
        let mut pending = vec![(root.id.as_str(), 0)];
        let mut nodes = Vec::new();
        while let Some((id, depth)) = pending.pop() {
            nodes.push((self.nodes[id], depth));
            if let Some(children) = self.children.get(id) {
                pending.extend(children.iter().rev().map(|id| (*id, depth + 1)));
            }
        }
        Ok(nodes)
    }
}
