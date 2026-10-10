//! Assistant code actions and resource links reuse the shared rich-text renderer.
use super::ConversationPanel;
use crate::{markdown::Markdown, project::resources::ResourceCatalog};
use gpui_kit::component::{
    ActiveTheme, Sizable, WindowExt,
    clipboard::Clipboard,
    notification::Notification,
    text::{TextView, TextViewStyle},
};
use gpui_kit::{
    Context, ElementId, Overflow, SharedString, StyleRefinement, WeakEntity, Window, div,
    prelude::*, px,
};
use yss_project_identity::{ProjectResourceKind as Kind, ProjectResourceRef};

pub(super) fn view(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    streaming: bool,
    owner: WeakEntity<ConversationPanel>,
) -> TextView {
    let mut code = StyleRefinement::default()
        .pt(px(38.))
        .flex()
        .flex_col()
        .items_start()
        .whitespace_nowrap();
    code.overflow.x = Some(Overflow::Scroll);
    // The component scroll viewport keeps code actions outside scrolled content.
    code.overflow.y = Some(Overflow::Scroll);
    let mut table = StyleRefinement::default();
    table.overflow.x = Some(Overflow::Scroll);
    TextView::markdown(id, text)
        .plugin(Markdown::default().on_link_click(move |href, window, cx| {
            let _ = owner.update(cx, |view, cx| view.open_markdown_link(href, window, cx));
        }))
        .stream_fade(streaming)
        .style(TextViewStyle::default().code_block(code).table(table))
        .code_block_actions(|block, _, cx| {
            let code = block.code();
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(
                    div().max_w(px(160.)).truncate().child(
                        block
                            .lang()
                            .unwrap_or_else(|| crate::text::t("panel.assistantCode").into()),
                    ),
                )
                .child(
                    Clipboard::new("copy-code")
                        .xsmall()
                        .tooltip(crate::text::t("panel.assistantCopyCode"))
                        .accessibility_label(crate::text::t("panel.assistantCopyCode"))
                        .value_fn(move |_, _| format!("{code}\n").into()),
                )
        })
}

impl ConversationPanel {
    fn open_markdown_link(&self, href: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if href.starts_with('#') {
            return;
        }
        if !href.starts_with("yssbi://") && url::Url::parse(href).is_ok() {
            crate::markdown::links::open_external(href, window, cx);
            return;
        }
        if let Some(resource) = self
            .resource_catalog
            .as_deref()
            .and_then(|catalog| resource_link(href, catalog))
        {
            self.open_reference(&resource, cx);
        } else {
            window.push_notification(
                Notification::error(crate::text::t("panel.assistantResourceOpenFailed")),
                cx,
            );
        }
    }
}

fn resource_link(href: &str, catalog: &ResourceCatalog) -> Option<ProjectResourceRef> {
    if let Some(href) = href.strip_prefix("yssbi://") {
        let (kind, id) = href.split_once('/')?;
        let kind = match kind {
            "event_graph" => Kind::EventGraph,
            "function_graph" => Kind::FunctionGraph,
            "chart" => Kind::Chart,
            "mind" => Kind::Mind,
            "doc" => Kind::Doc,
            "database" => Kind::Database,
            _ => return None,
        };
        let resource = ProjectResourceRef {
            kind,
            id: decode_id(id)?,
        };
        return catalog.get(&resource).map(|entry| entry.resource.clone());
    }
    let path = decode_id(href.split('#').next()?)?;
    let path = path.strip_prefix("./").unwrap_or(&path);
    let mut matches = catalog
        .entries
        .iter()
        .filter(|entry| entry.resource.id == path);
    let resource = matches.next()?;
    matches.next().is_none().then(|| resource.resource.clone())
}

fn decode_id(value: &str) -> Option<String> {
    // Percent decoding is strict like decodeURIComponent; IDs stay opaque.
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' && !(bytes.next()?.is_ascii_hexdigit() && bytes.next()?.is_ascii_hexdigit())
        {
            return None;
        }
    }
    percent_encoding::percent_decode_str(value)
        .decode_utf8()
        .ok()
        .map(|value| value.into_owned())
}
