use crate::file::{FileContent, FilePath, FileState, MAX_FILE_BYTES, bounded};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
pub const MAX_MIND_NODES: usize = 5_000;
pub type MindPath = FilePath<MindDocument>;
pub type MindState = FileState<MindDocument>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MindReference {
    Resource {
        path: String,
    },
    GraphNode {
        path: GraphResourcePath,
        #[serde(rename = "nodeId")]
        node_id: String,
    },
    Database {
        #[serde(rename = "databaseId")]
        database_id: String,
    },
}
use yss_graph_document::GraphResourcePath;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MindNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<MindReference>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MindDocument {
    pub root_id: String,
    /// Array order is sibling order. Parent links are the only hierarchy authority.
    pub nodes: Vec<MindNode>,
}

impl MindDocument {
    pub fn validate(&self) -> Result<(), String> {
        if self.nodes.is_empty() || self.nodes.len() > MAX_MIND_NODES {
            return Err("invalid mind node count".into());
        }
        let mut by_id = HashMap::new();
        for node in &self.nodes {
            if node.id.is_empty() || node.id.len() > 128 || by_id.insert(&node.id, node).is_some() {
                return Err("invalid or duplicate mind node id".into());
            }
        }
        let root = by_id.get(&self.root_id).ok_or("missing mind root")?;
        if root.parent_id.is_some() {
            return Err("mind root must not have a parent".into());
        }
        let mut connected = HashSet::from([self.root_id.as_str()]);
        for node in &self.nodes {
            let mut chain = HashSet::new();
            let mut current = node;
            while !connected.contains(current.id.as_str()) {
                if !chain.insert(current.id.as_str()) {
                    return Err("mind hierarchy contains a cycle".into());
                }
                let parent = current
                    .parent_id
                    .as_ref()
                    .ok_or("mind has more than one root")?;
                current = by_id.get(parent).ok_or("mind parent does not exist")?;
            }
            connected.extend(chain);
        }
        Ok(())
    }
}

impl FileContent for MindDocument {
    type Edit = MindEdit;
    const KIND: &'static str = "mind";
    const DIRECTORY: &'static str = yss_project_layout::MINDS_DIR;
    const EXTENSION: &'static str = yss_project_layout::MIND_EXTENSION;
    fn new(title: &str, next_id: &mut dyn FnMut() -> String) -> Self {
        let root_id = next_id();
        Self {
            root_id: root_id.clone(),
            nodes: vec![MindNode {
                id: root_id,
                parent_id: None,
                content: title.into(),
                reference: None,
            }],
        }
    }
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err("mind size limit exceeded".into());
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        value.validate()?;
        Ok(value)
    }
    fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        bounded(serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?)
    }
    fn duplicate(&self, next_id: &mut dyn FnMut() -> String) -> Self {
        let mut mind = self.clone();
        let ids = mind
            .nodes
            .iter()
            .map(|n| (n.id.clone(), next_id()))
            .collect::<HashMap<_, _>>();
        mind.root_id = ids[&mind.root_id].clone();
        for node in &mut mind.nodes {
            node.id = ids[&node.id].clone();
            node.parent_id = node.parent_id.as_ref().map(|id| ids[id].clone());
        }
        mind
    }
    fn apply(&mut self, edit: MindEdit) -> Result<(), String> {
        let mind = self;
        match edit {
            MindEdit::AddNode { node } => mind.nodes.push(node),
            MindEdit::SetContent { node_id, content } => {
                node_mut(mind, &node_id)?.content = content
            }
            MindEdit::SetReference { node_id, reference } => {
                node_mut(mind, &node_id)?.reference = reference
            }
            MindEdit::MoveNode {
                node_id,
                parent_id,
                before_id,
            } => {
                if node_id == mind.root_id {
                    return Err("cannot reparent mind root".into());
                }
                let index = mind
                    .nodes
                    .iter()
                    .position(|n| n.id == node_id)
                    .ok_or("mind node not found")?;
                let mut node = mind.nodes.remove(index);
                node.parent_id = Some(parent_id.clone());
                let insert = match before_id {
                    Some(before) => mind
                        .nodes
                        .iter()
                        .position(|n| n.id == before && n.parent_id.as_ref() == Some(&parent_id))
                        .ok_or("sibling not found")?,
                    None => mind.nodes.len(),
                };
                mind.nodes.insert(insert, node);
            }
            MindEdit::RemoveNode { node_id } => {
                if node_id == mind.root_id {
                    return Err("cannot remove mind root".into());
                }
                node_mut(mind, &node_id)?;
                let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
                for node in &mind.nodes {
                    if let Some(parent) = &node.parent_id {
                        children.entry(parent).or_default().push(&node.id);
                    }
                }
                let mut removed = HashSet::new();
                let mut pending = vec![node_id.as_str()];
                while let Some(id) = pending.pop() {
                    if removed.insert(id.to_owned()) {
                        pending.extend(children.get(id).into_iter().flatten().copied());
                    }
                }
                mind.nodes.retain(|n| !removed.contains(&n.id));
            }
        }
        mind.encode().map(|_| ())
    }
}
fn node_mut<'a>(mind: &'a mut MindDocument, id: &str) -> Result<&'a mut MindNode, String> {
    mind.nodes
        .iter_mut()
        .find(|n| n.id == id)
        .ok_or_else(|| "mind node not found".into())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum MindEdit {
    AddNode {
        node: MindNode,
    },
    SetContent {
        node_id: String,
        content: String,
    },
    SetReference {
        node_id: String,
        reference: Option<MindReference>,
    },
    MoveNode {
        node_id: String,
        parent_id: String,
        before_id: Option<String>,
    },
    RemoveNode {
        node_id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mind_tree_contract_rejects_ambiguous_hierarchy_and_renderer_state() {
        let mut document = MindDocument::new("Analysis", &mut || "root".into());
        document
            .apply(MindEdit::AddNode {
                node: MindNode {
                    id: "child".into(),
                    parent_id: Some("root".into()),
                    content: "OLS".into(),
                    reference: None,
                },
            })
            .unwrap();
        let bytes = document.encode().unwrap();
        assert_eq!(MindDocument::decode(&bytes).unwrap(), document);
        let valid = document;
        let mut invalid = valid.clone();
        invalid.nodes[1].parent_id = Some("child".into());
        assert!(invalid.validate().is_err());
        invalid.nodes[1].parent_id = Some("missing".into());
        assert!(invalid.validate().is_err());
        invalid.nodes[1].parent_id = None;
        assert!(invalid.validate().is_err());
        invalid = valid.clone();
        invalid.nodes.push(valid.nodes[1].clone());
        assert!(invalid.validate().is_err());
        for (key, value) in [
            ("selected", serde_json::json!(true)),
            ("position", serde_json::json!({"x": 10, "y": 20})),
        ] {
            let mut wire = serde_json::to_value(&valid).unwrap();
            wire["nodes"][0][key] = value;
            assert!(MindDocument::decode(&serde_json::to_vec(&wire).unwrap()).is_err());
        }
        assert!(crate::doc::DocPath::parse("docs/../escape.md").is_err());
        assert!(MindPath::parse("minds/nested/Map.yssbi-mind").is_err());
    }
}
