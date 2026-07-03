use yssbi_lib::execution::{ExecutionEvent, Presentation};

fn main() {
    let event = ExecutionEvent::OpenSourceWindow {
        source_id: "window_test".into(),
        presentation: Presentation::Inspector,
        window_title: "View: (null)".into(),
    };
    println!("{}", serde_json::to_string_pretty(&event).unwrap());
}
