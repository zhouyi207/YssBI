mod bindings;
mod project;
mod projects;
mod services;
mod startup;
mod window_state;
mod workbench;

use yss_assistant_ui as assistant;
use yss_data_ui::databases;
use yss_graph_ui::{canvas, catalog_rows};
use yss_plugin_ui::plugins;
use yss_resource_ui::{documents, file_session, minds};
use yss_results_ui::{charts, results};
use yss_settings_ui as settings;
use yss_ui::{appearance, keymap, modal_window, preferences, text, window_chrome};

pub use startup::Startup;
pub use window_state::WindowState;

pub fn init(cx: &mut gpui_kit::App) {
    bindings::init(cx);
}
