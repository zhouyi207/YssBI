use super::{InspectorSection, Pager};
use gpui_kit::test::{TestSupportExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Context, Entity, IntoElement, Render, TestAppContext, Window,
    WindowOptions, div, prelude::*,
};

struct Harness {
    page: usize,
    pages: usize,
    open: bool,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                Pager::new("pager", self.page, self.pages).on_change(cx.listener(
                    |view, page: &usize, _, cx| {
                        view.page = *page;
                        cx.notify();
                    },
                )),
            )
            .child(
                InspectorSection::new("section", "Section")
                    .collapsible(
                        self.open,
                        cx.listener(|view, open: &bool, _, cx| {
                            view.open = *open;
                            cx.notify();
                        }),
                    )
                    .child(div().id("content").test_support().child("Content")),
            )
    }
}

fn open(pages: usize, cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Harness>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|_| Harness {
                page: 0,
                pages,
                open: false,
            })
        })
        .unwrap()
    })
}

#[gpui_kit::test]
fn pager_requests_neighbor_pages_within_bounds(cx: &mut TestAppContext) {
    let (window, harness) = open(3, cx);
    let click = |button: &'static str, cx: &mut TestAppContext| {
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.within("pager").click(button, cx);
        })
        .unwrap();
        cx.update(|cx| harness.read(cx).page)
    };
    assert_eq!(
        click("previous", cx),
        0,
        "the first page has no previous page"
    );
    assert_eq!(click("next", cx), 1);
    assert_eq!(click("next", cx), 2);
    assert_eq!(click("next", cx), 2, "the last page has no next page");
    assert_eq!(click("previous", cx), 1);
}

#[gpui_kit::test]
fn single_page_renders_no_pager(cx: &mut TestAppContext) {
    let (window, _) = open(1, cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("previous").is_none());
        assert!(window.try_find("next").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn collapsible_section_requests_the_toggled_state(cx: &mut TestAppContext) {
    let (window, harness) = open(1, cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("content").is_none());
        window.within("section").click("header", cx);
        window.render_frame(cx);
        assert!(window.try_find("content").is_some());
    })
    .unwrap();
    cx.update(|cx| assert!(harness.read(cx).open));

    cx.update_window(window, |_, window, cx| {
        window.within("section").click("header", cx);
        window.render_frame(cx);
        assert!(window.try_find("content").is_none());
    })
    .unwrap();
    cx.update(|cx| assert!(!harness.read(cx).open));
}
