//! Native math nodes are prepared during background Markdown parsing.
use super::{Markdown, typesetting};

use std::sync::{Arc, LazyLock};

use gpui::{App, Hsla, IntoElement, Pixels, SharedString, Window, div, prelude::*, svg};
use gpui_component::{
    ActiveTheme,
    menu::{ContextMenu, ContextMenuExt, PopupMenuItem},
    text::{
        InlineElement, InlineRenderContext, MarkdownExtensions, MarkdownNode, MarkdownParseContext,
        MarkdownPlugin, TextView, markdown_ast,
    },
};

pub(super) fn extensions() -> MarkdownExtensions {
    static EXTENSIONS: LazyLock<MarkdownExtensions> = LazyLock::new(|| {
        MarkdownExtensions::default()
            .plugin(InlineMath)
            .block_parser(parse_display)
    });
    EXTENSIONS.clone()
}

struct MathNode {
    formula: Option<Arc<typesetting::Formula>>,
    display: bool,
    offset: usize,
}

enum DisplayPart {
    Prose { offset: usize, source: String },
    Formula(MarkdownNode),
}

fn parse_math(
    name: &'static str,
    node: &markdown_ast::Node,
    cx: &MarkdownParseContext<'_>,
) -> Option<MarkdownNode> {
    let source = cx.node_source(node)?;
    let (latex, display) = match node {
        markdown_ast::Node::InlineMath(math) => (math.value.as_str(), source.starts_with("$$")),
        markdown_ast::Node::Math(math) => (math.value.as_str(), true),
        markdown_ast::Node::Code(code) if code.lang.as_deref() == Some("math") => {
            (code.value.as_str(), true)
        }
        _ => return None,
    };
    Some(
        MarkdownNode::new(
            name,
            MathNode {
                formula: typesetting::prepare(latex, display),
                display,
                offset: cx.offset() + node.position()?.start.offset,
            },
        )
        .text(source.to_owned())
        .markdown(source.to_owned())
        .accessibility_label(latex.to_owned()),
    )
}

struct InlineMath;

impl MarkdownPlugin for InlineMath {
    fn name(&self) -> &str {
        "yss-inline-math"
    }

    fn parse(
        &self,
        node: &markdown_ast::Node,
        cx: &MarkdownParseContext<'_>,
    ) -> Option<MarkdownNode> {
        match node {
            markdown_ast::Node::InlineMath(_) => parse_math("yss-inline-math", node, cx),
            _ => None,
        }
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        _: &mut Window,
        _: &mut App,
    ) -> Option<InlineElement> {
        let math = node.data::<MathNode>()?;
        let formula = math.formula.as_ref()?;
        let font_size = context.font_size() * 1.05;
        let color = context.text_style().color;
        if math.display {
            Some(InlineElement::new(display_formula(
                node, formula, font_size, color,
            )))
        } else {
            Some(
                InlineElement::new(
                    formula_container(node).child(formula_image(formula, font_size, color)),
                )
                .with_baseline(font_size * formula.baseline),
            )
        }
    }
}

fn parse_display(node: &markdown_ast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
    match node {
        markdown_ast::Node::Paragraph(paragraph) => split_display_paragraph(paragraph, node, cx),
        markdown_ast::Node::Math(_) | markdown_ast::Node::Code(_) => {
            parse_math("yss-display-math", node, cx)
        }
        _ => None,
    }
}

pub(super) fn render(
    node: &MarkdownNode,
    prose: &Markdown,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    if let Some(parts) = node.data::<Vec<DisplayPart>>() {
        return div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .children(parts.iter().map(|part| {
                match part {
                    DisplayPart::Prose { offset, source } => TextView::markdown(
                        SharedString::from(format!("math-prose-{offset}")),
                        source.clone(),
                    )
                    .plugin(prose.clone())
                    .into_any_element(),
                    DisplayPart::Formula(formula) => render_display(formula, window, cx),
                }
            }))
            .into_any_element();
    }
    render_display(node, window, cx)
}

fn split_display_paragraph(
    paragraph: &markdown_ast::Paragraph,
    node: &markdown_ast::Node,
    cx: &MarkdownParseContext<'_>,
) -> Option<MarkdownNode> {
    let position = node.position()?;
    let mut cursor = position.start.offset;
    let mut parts = Vec::new();
    for child in &paragraph.children {
        if !matches!(child, markdown_ast::Node::InlineMath(_))
            || !cx.node_source(child)?.starts_with("$$")
        {
            continue;
        }
        let span = child.position()?;
        let prose = cx.source().get(cursor..span.start.offset)?.trim();
        if !prose.is_empty() {
            parts.push(DisplayPart::Prose {
                offset: cx.offset() + cursor,
                source: prose.to_owned(),
            });
        }
        parts.push(DisplayPart::Formula(parse_math(
            "yss-display-math",
            child,
            cx,
        )?));
        cursor = span.end.offset;
    }
    if parts.is_empty() {
        return None;
    }
    let prose = cx.source().get(cursor..position.end.offset)?.trim();
    if !prose.is_empty() {
        parts.push(DisplayPart::Prose {
            offset: cx.offset() + cursor,
            source: prose.to_owned(),
        });
    }
    // The inline flow measures objects at max-content width. Lift same-line $$
    // formulas to blocks so wide equations scroll within the actual text column.
    let source = cx.node_source(node)?.to_owned();
    Some(
        MarkdownNode::new("yss-display-math", parts)
            .text(source.clone())
            .markdown(source),
    )
}

fn render_display(node: &MarkdownNode, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
    let font_size = window.text_style().font_size.to_pixels(window.rem_size()) * 1.05;
    if let Some(math) = node.data::<MathNode>()
        && let Some(formula) = &math.formula
    {
        return display_formula(node, formula, font_size, window.text_style().color)
            .into_any_element();
    }
    formula_container(node)
        .text_color(cx.theme().danger)
        .child(node.as_text().to_owned())
        .into_any_element()
}

fn formula_image(formula: &typesetting::Formula, font_size: Pixels, color: Hsla) -> gpui::Svg {
    svg()
        .data(&formula.svg)
        .w(font_size * formula.width)
        .h(font_size * formula.height)
        .flex_shrink_0()
        .text_color(color)
}

fn formula_container(node: &MarkdownNode) -> ContextMenu<gpui::Stateful<gpui::Div>> {
    let math = node.data::<MathNode>();
    let offset = math.map_or(0, |math| math.offset);
    let source = node.as_markdown().to_owned();
    div()
        .id(SharedString::from(format!("math-{offset}")))
        .when(math.is_some_and(|math| math.display), |view| {
            view.overflow_x_scroll()
        })
        .context_menu(move |menu, _, _| {
            let source = source.clone();
            menu.item(
                PopupMenuItem::new(crate::text::t("native.markdown.copyFormula")).on_click(
                    move |_, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(source.clone()));
                    },
                ),
            )
        })
}

fn display_formula(
    node: &MarkdownNode,
    formula: &typesetting::Formula,
    font_size: Pixels,
    color: Hsla,
) -> impl IntoElement + use<> {
    formula_container(node)
        .w_full()
        .min_w_0()
        .my(font_size)
        .py(font_size * 0.25)
        .child(
            div()
                .min_w_full()
                .w(font_size * formula.width)
                .flex()
                .justify_center()
                .child(formula_image(formula, font_size, color)),
        )
}
