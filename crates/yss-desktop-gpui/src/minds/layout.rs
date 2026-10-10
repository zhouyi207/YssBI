//! Disposable horizontal tree geometry; source ordering and parent links stay untouched.
use gpui_kit::{Bounds, Pixels, point, px, size};
use std::collections::{BTreeSet, HashMap};
use yss_project_model::mind::MindDocument;

pub(super) const TOPIC_WIDTH: f32 = 240.;
const COLUMN_GAP: f32 = 64.;
const ROW_GAP: f32 = 24.;

#[derive(Clone)]
pub(super) struct TopicPlacement {
    pub index: usize,
    pub parent: Option<usize>,
    pub depth: usize,
    pub children: usize,
    pub bounds: Bounds<Pixels>,
}
pub(super) struct MindLayout {
    pub nodes: Vec<TopicPlacement>,
    pub bounds: Bounds<Pixels>,
}
impl MindLayout {
    pub fn new(document: &MindDocument, collapsed: &BTreeSet<String>) -> Self {
        let indices = document
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.id.as_str(), i))
            .collect::<HashMap<_, _>>();
        let mut children = vec![vec![]; document.nodes.len()];
        for (index, node) in document.nodes.iter().enumerate() {
            if let Some(parent) = node.parent_id.as_deref().and_then(|id| indices.get(id)) {
                children[*parent].push(index);
            }
        }
        let Some(&root) = indices.get(document.root_id.as_str()) else {
            return Self {
                nodes: vec![],
                bounds: Bounds::default(),
            };
        };
        let mut order = vec![];
        let mut pending = vec![(root, 0)];
        while let Some((index, depth)) = pending.pop() {
            order.push((index, depth));
            if !collapsed.contains(&document.nodes[index].id) {
                pending.extend(
                    children[index]
                        .iter()
                        .rev()
                        .map(|child| (*child, depth + 1)),
                );
            }
        }
        let heights = document
            .nodes
            .iter()
            .map(|node| {
                let lines = node
                    .content
                    .lines()
                    .map(|line| line.chars().count().max(1).div_ceil(26))
                    .sum::<usize>()
                    .max(1);
                (lines as f32 * 20. + 28.).clamp(64., 320.)
            })
            .collect::<Vec<_>>();
        let mut spans = heights.clone();
        for &(index, _) in order.iter().rev() {
            if !collapsed.contains(&document.nodes[index].id) {
                let branches = children[index]
                    .iter()
                    .map(|child| spans[*child])
                    .sum::<f32>()
                    + children[index].len().saturating_sub(1) as f32 * ROW_GAP;
                spans[index] = spans[index].max(branches);
            }
        }
        let mut starts = vec![0.; document.nodes.len()];
        let mut nodes = vec![];
        let mut width: f32 = TOPIC_WIDTH;
        for (index, depth) in order {
            let branches = children[index]
                .iter()
                .map(|child| spans[*child])
                .sum::<f32>()
                + children[index].len().saturating_sub(1) as f32 * ROW_GAP;
            let mut next = starts[index] + (spans[index] - branches) / 2.;
            for &child in &children[index] {
                starts[child] = next;
                next += spans[child] + ROW_GAP;
            }
            let x = depth as f32 * (TOPIC_WIDTH + COLUMN_GAP);
            width = width.max(x + TOPIC_WIDTH);
            nodes.push(TopicPlacement {
                index,
                depth,
                children: children[index].len(),
                parent: document.nodes[index]
                    .parent_id
                    .as_deref()
                    .and_then(|id| indices.get(id))
                    .copied(),
                bounds: Bounds::new(
                    point(
                        px(x),
                        px(starts[index] + (spans[index] - heights[index]) / 2.),
                    ),
                    size(px(TOPIC_WIDTH), px(heights[index])),
                ),
            });
        }
        Self {
            nodes,
            bounds: Bounds::new(point(px(0.), px(0.)), size(px(width), px(spans[root]))),
        }
    }
}
