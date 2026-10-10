//! Product links match react/src/shared/appLinks.ts.
use gpui::{App, Window, div, prelude::*, px, size};
use gpui_component::button::{Button, ButtonVariants};

const REPOSITORY: &str = "https://github.com/zhouyi207/YssBI";

#[derive(Clone, Copy, PartialEq)]
pub(in crate::workbench) enum HelpPage {
    Architecture,
    Documentation,
    ReleaseNotes,
    Repository,
    ReportIssue,
}

impl HelpPage {
    pub(super) fn url(self) -> String {
        let suffix = match self {
            Self::Architecture => "/blob/main/docs/architecture/ARCHITECTURE.md",
            Self::Documentation => "/blob/main/README.md",
            Self::ReleaseNotes => "/releases",
            Self::Repository => "",
            Self::ReportIssue => "/issues",
        };
        format!("{REPOSITORY}{suffix}")
    }
}

pub(super) fn show_about(window: &mut Window, cx: &mut App) {
    crate::modal_window::open(
        crate::text::t("native.workbench.aboutTitle"),
        size(px(480.), px(280.)),
        window,
        cx,
        |_, _| {
            crate::modal_window::ModalContent::new(|_, _| {
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(format!("YssBI {}", env!("CARGO_PKG_VERSION")))
                    .child(crate::text::t("native.workbench.appDescription"))
                    .child(
                        div().flex().gap_2().children([
                            Button::new("about-repository")
                                .link()
                                .label(crate::text::t("menubar.githubRepository"))
                                .on_click(|_, _, cx| cx.open_url(&HelpPage::Repository.url())),
                            Button::new("about-issue")
                                .link()
                                .label(crate::text::t("menubar.reportIssue"))
                                .on_click(|_, _, cx| cx.open_url(&HelpPage::ReportIssue.url())),
                        ]),
                    )
            })
            .confirm(crate::text::t("common.close"), |_, _, _| true)
        },
    );
}
