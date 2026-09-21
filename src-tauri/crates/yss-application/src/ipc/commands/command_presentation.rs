use crate::ipc::{channel::presentation::PresentationChannels, error::CommandError};
use crate::{presentation::UiError, session::ApplicationState};
use tauri::{State, WebviewWindow};
use yss_project_identity::ProjectInstanceId;
use yss_ui_contract::*;

pub(crate) fn ui_error(error: UiError) -> CommandError {
    CommandError::expected(error.code())
}

#[tauri::command]
pub async fn inspect_ui(
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    request: InspectUiRequest,
) -> Result<UiInspection, CommandError> {
    let application = application.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        application
            .inspect_ui(&project_instance_id, request)
            .map_err(ui_error)
    })
    .await
    .map_err(CommandError::internal)?
}

#[tauri::command]
pub async fn update_ui(
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    request: UpdateUiRequest,
) -> Result<UiUpdate, CommandError> {
    let application = application.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        application
            .update_ui(&project_instance_id, request)
            .map_err(ui_error)
    })
    .await
    .map_err(CommandError::internal)?
}

#[tauri::command]
pub async fn request_ui_intent(
    window: WebviewWindow,
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    request: RequestUiIntent,
) -> Result<UiIntentReceipt, CommandError> {
    let application = application.inner().clone();
    let caller = format!("window:{}", window.label());
    tauri::async_runtime::spawn_blocking(move || {
        application
            .request_ui_intent(&project_instance_id, &caller, request)
            .map_err(ui_error)
    })
    .await
    .map_err(CommandError::internal)?
}

#[tauri::command]
pub async fn activate_ui_element(
    window: WebviewWindow,
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    request: ActivateUiRequest,
) -> Result<UiIntentReceipt, CommandError> {
    let application = application.inner().clone();
    let caller = format!("window:{}", window.label());
    tauri::async_runtime::spawn_blocking(move || {
        application
            .activate_ui_element(&project_instance_id, &caller, request)
            .map_err(ui_error)
    })
    .await
    .map_err(CommandError::internal)?
}

#[tauri::command]
pub fn subscribe_ui(
    window: WebviewWindow,
    application: State<'_, ApplicationState>,
    channels: State<'_, PresentationChannels>,
    project_instance_id: ProjectInstanceId,
    workbench: bool,
    channel: tauri::ipc::Channel<UiEvent>,
) -> Result<String, CommandError> {
    if workbench && window.label() != "main" {
        return Err(ui_error(UiError::Workbench));
    }
    let session = application
        .ui_session(&project_instance_id)
        .map_err(ui_error)?;
    let (sender, receiver) = tokio::sync::broadcast::channel(128);
    let subscription = session
        .presentation
        .subscribe(std::sync::Arc::new(move |event| {
            let _ = sender.send(event);
        }))
        .map_err(ui_error)?;
    if workbench {
        session.presentation.attach_workbench();
    }
    channels.subscribe(
        &window,
        receiver,
        move || {
            drop(subscription);
            if workbench {
                session.presentation.detach_workbench();
            }
        },
        channel,
    )
}

#[tauri::command]
pub fn unsubscribe_ui(
    window: WebviewWindow,
    channels: State<'_, PresentationChannels>,
    subscription_id: String,
) {
    channels.unsubscribe(window.label(), &subscription_id);
}

#[tauri::command]
pub fn pending_ui_intents(
    window: WebviewWindow,
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
) -> Result<Vec<UiIntentReceipt>, CommandError> {
    if window.label() != "main" {
        return Err(ui_error(UiError::Workbench));
    }
    application
        .ui_session(&project_instance_id)
        .map_err(ui_error)?
        .presentation
        .pending()
        .map_err(ui_error)
}

#[tauri::command]
pub fn settle_ui_intent(
    window: WebviewWindow,
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    status: UiIntentStatus,
) -> Result<bool, CommandError> {
    if window.label() != "main" {
        return Err(ui_error(UiError::Workbench));
    }
    application
        .ui_session(&project_instance_id)
        .map_err(ui_error)?
        .presentation
        .settle_intent(&id, window.label(), status)
        .map_err(ui_error)
}
