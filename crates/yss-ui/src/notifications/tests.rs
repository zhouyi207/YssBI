use super::Notifications;
use crate::name_form::NameForm;
use gpui_kit::component::{WindowExt, button::Button};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext, Context, Entity, IntoElement, Render, TestAppContext, Window,
    WindowOptions, div, prelude::*,
};
use std::time::Duration;

struct RetryView {
    notifications: Notifications,
    retries: usize,
}

impl Render for RetryView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                Button::new("fail")
                    .label("Fail")
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.notifications
                            .error_with_retry("Read failed", cx, |view, _, cx| {
                                view.retries += 1;
                                cx.notify();
                            });
                    })),
            )
    }
}

fn open_retry(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<RetryView>) {
    cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|_| RetryView {
                notifications: Notifications::new(window),
                retries: 0,
            })
        })
        .expect("open notification test window")
    })
}

fn count(window: AnyWindowHandle, cx: &mut TestAppContext) -> usize {
    cx.update_window(window, |_, window, cx| window.notifications(cx).len())
        .expect("read notifications")
}

#[gpui_kit::test]
fn form_failure_preserves_input_and_notifies_its_own_window(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, form) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            NameForm::new("Unsubmitted name".into(), window, cx)
        })
        .expect("open name form")
    });
    let (other_window, _) = open_retry(cx);
    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.submitting(cx);
            form.finish(Some("native.workbench.fileResourceFailed"), cx);
        });
    });
    assert_eq!(count(window, cx), 1);
    assert_eq!(count(other_window, cx), 0);
    cx.update(|cx| {
        assert!(!form.read(cx).busy());
        assert_eq!(form.read(cx).value(cx).as_deref(), Some("Unsubmitted name"));
    });
    cx.update_window(window, |_, window, cx| {
        let notification = window.notifications(cx)[0].entity_id();
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(window.try_find("name-form-error").is_none());
        assert_eq!(window.notifications(cx)[0].entity_id(), notification);
    })
    .expect("render form after failure");
}

#[gpui_kit::test]
fn repeated_failure_replaces_toast_and_retry_invokes_owner(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (window, view) = open_retry(cx);
    for _ in 0..2 {
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("fail", cx);
        })
        .expect("trigger failure through button");
        assert_eq!(count(window, cx), 1);
    }
    cx.executor().advance_clock(Duration::from_secs(6));
    cx.run_until_parked();
    assert_eq!(
        count(window, cx),
        1,
        "retry stays available until dismissal"
    );
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("retry", cx);
    })
    .expect("retry through toast action");
    cx.update(|cx| assert_eq!(view.read(cx).retries, 1));
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(count(window, cx), 0);
}

#[gpui_kit::test]
fn deferred_feedback_does_not_keep_closed_owner_alive(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, _) = open_retry(cx);
    cx.update_window(window, |_, window, cx| {
        let transient = cx.new(|_| RetryView {
            notifications: Notifications::new(window),
            retries: 0,
        });
        transient.update(cx, |view, cx| {
            view.notifications.error("Obsolete failure", cx)
        });
        drop(transient);
    })
    .expect("release feedback owner");
    assert_eq!(count(window, cx), 0);
}
