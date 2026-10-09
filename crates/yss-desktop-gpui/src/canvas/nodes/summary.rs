//! Bounded parameter text is formatted on projection replacement, not during pan/run repaint.
use gpui::SharedString;
use std::{
    collections::BTreeMap,
    io::{self, Write},
    sync::Arc,
};
use yss_graph_document::NodeId;
use yss_graph_editor::projection::EditorProjectionModel;

#[derive(Clone)]
pub(in crate::canvas) struct ParameterSummary {
    pub title: SharedString,
    pub value: SharedString,
}

#[derive(Default)]
pub(in crate::canvas) struct Contents {
    projection: Option<Arc<EditorProjectionModel>>,
    summaries: BTreeMap<NodeId, Box<[ParameterSummary]>>,
}

impl Contents {
    pub fn refresh(&mut self, projection: &Arc<EditorProjectionModel>) {
        if self
            .projection
            .as_ref()
            .is_some_and(|previous| Arc::ptr_eq(previous, projection))
        {
            return;
        }
        self.summaries = projection
            .nodes
            .iter()
            .filter_map(|node| {
                let rows = crate::canvas::geometry::inline_parameters(node)
                    .map(|parameter| ParameterSummary {
                        title: parameter.display.title.to_string().into(),
                        value: summary(parameter.value.as_ref()).into(),
                    })
                    .collect::<Box<[_]>>();
                (!rows.is_empty()).then_some((node.node_id, rows))
            })
            .collect();
        self.projection = Some(projection.clone());
    }

    pub fn rows(&self, id: NodeId) -> &[ParameterSummary] {
        self.summaries.get(&id).map_or(&[], |rows| rows)
    }
}

fn summary(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return "—".into();
    };
    let text = if let Some(text) = value.as_str() {
        std::borrow::Cow::Borrowed(text)
    } else {
        let mut writer = Prefix(Vec::new());
        let _ = serde_json::to_writer(&mut writer, value);
        std::borrow::Cow::Owned(String::from_utf8_lossy(&writer.0).into_owned())
    };
    let mut chars = text.chars();
    let mut prefix = chars.by_ref().take(48).collect::<String>();
    if chars.next().is_some() {
        prefix.pop();
        prefix.push('…');
    }
    prefix
}

// 49 Unicode scalars need at most 196 bytes, enough to decide whether the 48-char label truncates.
struct Prefix(Vec<u8>);
impl Write for Prefix {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(196 - self.0.len());
        if count == 0 && !bytes.is_empty() {
            return Err(io::ErrorKind::WriteZero.into());
        }
        self.0.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
