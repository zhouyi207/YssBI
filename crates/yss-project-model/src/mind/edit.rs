//! Topic batch semantics; Project applies these commands to its unpublished candidate.
use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MindNodeMove {
    pub node_id: String,
    pub parent_id: String,
    pub before_id: Option<String>,
}

#[derive(Debug)]
pub struct PreparedMindCopy {
    pub edit: MindEdit,
    pub created: BTreeMap<String, String>,
}

impl MindDocument {
    pub fn prepare_subtree_copy(
        &self,
        roots: &[String],
        parent_id: &str,
        before_id: Option<&str>,
        next_id: &mut dyn FnMut() -> String,
    ) -> Result<PreparedMindCopy, String> {
        if roots.is_empty() {
            return Err("empty mind copy".into());
        }
        let tree = self.tree()?;
        tree.node(parent_id)?;
        if let Some(before) = before_id
            && tree.node(before)?.parent_id.as_deref() != Some(parent_id)
        {
            return Err("sibling not found".into());
        }
        let mut seen = HashSet::new();
        let mut source = Vec::new();
        for root in roots {
            for (node, _) in tree.walk(root)? {
                if !seen.insert(node.id.as_str()) {
                    return Err("overlapping mind copy branches".into());
                }
                source.push(node);
            }
        }
        let mut assigned = self
            .nodes
            .iter()
            .map(|node| node.id.as_str().to_owned())
            .collect::<HashSet<_>>();
        let mut created = BTreeMap::new();
        for node in &source {
            let id = next_id();
            if id.is_empty() || id.len() > 128 || !assigned.insert(id.clone()) {
                return Err("invalid copied mind identity".into());
            }
            created.insert(node.id.clone(), id);
        }
        let roots = roots.iter().map(String::as_str).collect::<HashSet<_>>();
        let nodes = source
            .into_iter()
            .map(|node| MindNode {
                id: created[&node.id].clone(),
                parent_id: Some(if roots.contains(node.id.as_str()) {
                    parent_id.into()
                } else {
                    created[node.parent_id.as_ref().expect("non-root copy has a parent")].clone()
                }),
                content: node.content.clone(),
                reference: node.reference.clone(),
            })
            .collect();
        let edit = MindEdit::AddNodes {
            nodes,
            before_id: before_id.map(String::from),
        };
        let mut candidate = self.clone();
        candidate.apply(edit.clone())?;
        Ok(PreparedMindCopy { edit, created })
    }

    pub(super) fn apply_edit(&mut self, edit: MindEdit) -> Result<(), String> {
        match edit {
            MindEdit::AddNode { node } => self.add_nodes(vec![node], None)?,
            MindEdit::AddNodes { nodes, before_id } => {
                self.add_nodes(nodes, before_id.as_deref())?
            }
            MindEdit::SetContent { node_id, content } => {
                node_mut(self, &node_id)?.content = content
            }
            MindEdit::SetReference { node_id, reference } => {
                node_mut(self, &node_id)?.reference = reference
            }
            MindEdit::MoveNode {
                node_id,
                parent_id,
                before_id,
            } => self.move_nodes(vec![MindNodeMove {
                node_id,
                parent_id,
                before_id,
            }])?,
            MindEdit::MoveNodes { moves } => self.move_nodes(moves)?,
            MindEdit::RemoveNode { node_id } => self.remove_nodes(&[node_id])?,
            MindEdit::RemoveNodes { node_ids } => self.remove_nodes(&node_ids)?,
        }
        self.encode().map(|_| ())
    }

    fn add_nodes(&mut self, nodes: Vec<MindNode>, before_id: Option<&str>) -> Result<(), String> {
        if nodes.is_empty() {
            return Err("empty mind creation".into());
        }
        let insert = if let Some(before) = before_id {
            let index = self
                .nodes
                .iter()
                .position(|node| node.id == before)
                .ok_or("sibling not found")?;
            let new_ids = nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<HashSet<_>>();
            let parent = self.nodes[index].parent_id.as_deref();
            if nodes
                .iter()
                .filter(|node| {
                    node.parent_id
                        .as_deref()
                        .is_none_or(|id| !new_ids.contains(id))
                })
                .any(|node| node.parent_id.as_deref() != parent)
            {
                return Err("sibling has a different parent".into());
            }
            index
        } else {
            self.nodes.len()
        };
        self.nodes.splice(insert..insert, nodes);
        Ok(())
    }

    fn move_nodes(&mut self, moves: Vec<MindNodeMove>) -> Result<(), String> {
        if moves.is_empty() {
            return Err("empty mind move".into());
        }
        let mut seen = HashSet::new();
        for movement in &moves {
            if movement.node_id == self.root_id || !seen.insert(&movement.node_id) {
                return Err("invalid or repeated mind move target".into());
            }
            node_mut(self, &movement.node_id)?.parent_id = Some(movement.parent_id.clone());
        }
        // All parent changes are visible before ordering; only the final tree must be valid.
        for movement in moves {
            let index = self
                .nodes
                .iter()
                .position(|node| node.id == movement.node_id)
                .ok_or("mind node not found")?;
            let node = self.nodes.remove(index);
            let insert = match movement.before_id {
                Some(before) => self
                    .nodes
                    .iter()
                    .position(|node| {
                        node.id == before
                            && node.parent_id.as_deref() == Some(movement.parent_id.as_str())
                    })
                    .ok_or("sibling not found")?,
                None => self.nodes.len(),
            };
            self.nodes.insert(insert, node);
        }
        Ok(())
    }

    fn remove_nodes(&mut self, roots: &[String]) -> Result<(), String> {
        if roots.is_empty() {
            return Err("empty mind removal".into());
        }
        let tree = self.tree()?;
        let mut selected = HashSet::new();
        let mut removed = HashSet::new();
        for root in roots {
            if root == &self.root_id || !selected.insert(root) {
                return Err("invalid mind removal target".into());
            }
            removed.extend(
                tree.walk(root)?
                    .into_iter()
                    .map(|(node, _)| node.id.clone()),
            );
        }
        self.nodes.retain(|node| !removed.contains(&node.id));
        Ok(())
    }
}

fn node_mut<'a>(mind: &'a mut MindDocument, id: &str) -> Result<&'a mut MindNode, String> {
    mind.nodes
        .iter_mut()
        .find(|node| node.id == id)
        .ok_or_else(|| "mind node not found".into())
}
