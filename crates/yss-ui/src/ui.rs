//! Product presentation shared by desktop feature crates and the application shell.
pub mod appearance;
pub mod constant_values;
pub mod input;
pub mod inspector;
pub mod keymap;
pub mod layout_store;
pub mod lifecycle;
pub mod markdown;
pub mod modal_window;
pub mod name_form;
pub mod preferences;
pub mod sidebar;
pub mod table;
pub mod text;
pub mod tool_panel;
pub mod window_chrome;

/// Install shared presentation after GPUI Kit and before application keybindings or windows.
pub fn init(cx: &mut gpui_kit::App) {
    preferences::init(cx);
    markdown::init();
    window_chrome::init(cx);
    modal_window::init(cx);
}
