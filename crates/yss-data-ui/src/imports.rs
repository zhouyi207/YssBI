//! Import inputs and discovery are transient; Application owns every imported dataset.
mod discovery;
mod feedback;
mod inputs;
mod render;
mod samples;
mod selection;

pub use feedback::{IMPORT_FAILED, sample_import_failure};

use crate::DataServices;
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext, Context, Entity, EventEmitter, Window};
use std::sync::Arc;
use yss_application::database::samples::SampleDataset;
use yss_database_contract::{DatabaseEngineSql, DatabaseImportSource};
use yss_project_identity::ProjectInstanceId;
use yss_ui::lifecycle::LogReleased;

#[derive(Clone)]
pub struct ImportScope {
    project: ProjectInstanceId,
    lifecycle: u64,
}
impl ImportScope {
    pub fn new(project: ProjectInstanceId, lifecycle: u64) -> Self {
        Self { project, lifecycle }
    }
    pub fn project(&self) -> &ProjectInstanceId {
        &self.project
    }
    pub fn lifecycle(&self) -> u64 {
        self.lifecycle
    }
}

#[derive(Clone)]
pub enum ImportRequest {
    Source {
        source: DatabaseImportSource,
        name: Option<String>,
    },
    Sample {
        id: String,
        version: u32,
    },
}

#[derive(Clone)]
pub struct ImportSubmission {
    scope: ImportScope,
    request: ImportRequest,
}
impl ImportSubmission {
    pub fn into_parts(self) -> (ImportScope, ImportRequest) {
        (self.scope, self.request)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImportKind {
    Csv,
    Parquet,
    Excel,
    Sqlite,
    Postgres,
    Mysql,
    Mariadb,
}
impl ImportKind {
    fn label(self) -> std::borrow::Cow<'static, str> {
        match self {
            Self::Csv => yss_ui::text::t("importModal.types.csv.label"),
            Self::Parquet => yss_ui::text::t("importModal.types.parquet.label"),
            Self::Excel => yss_ui::text::t("importModal.types.xlsx.label"),
            Self::Sqlite => yss_ui::text::t("importModal.types.sqlite.label"),
            Self::Postgres => yss_ui::text::t("importModal.types.postgres.label"),
            Self::Mysql => yss_ui::text::t("importModal.types.mysql.label"),
            Self::Mariadb => yss_ui::text::t("importModal.types.mariadb.label"),
        }
    }
    fn description(self) -> std::borrow::Cow<'static, str> {
        yss_ui::text::t(match self {
            Self::Csv => "importModal.types.csv.description",
            Self::Parquet => "importModal.types.parquet.description",
            Self::Excel => "importModal.types.xlsx.description",
            Self::Sqlite => "importModal.types.sqlite.description",
            Self::Postgres => "importModal.types.postgres.description",
            Self::Mysql => "importModal.types.mysql.description",
            Self::Mariadb => "importModal.types.mariadb.description",
        })
    }
    fn icon(self) -> gpui_kit::assets::IconName {
        use gpui_kit::assets::IconName;
        match self {
            Self::Csv => IconName::FileText,
            Self::Parquet => IconName::Layers,
            Self::Excel => IconName::FileSpreadsheet,
            Self::Sqlite => IconName::Database,
            Self::Postgres | Self::Mysql | Self::Mariadb => IconName::Server,
        }
    }
    fn remote(self) -> bool {
        matches!(self, Self::Postgres | Self::Mysql | Self::Mariadb)
    }
    fn engine(self) -> DatabaseEngineSql {
        match self {
            Self::Sqlite => DatabaseEngineSql::Sqlite { auto_create: false },
            Self::Postgres => DatabaseEngineSql::Postgres { ssl: true },
            Self::Mysql | Self::Mariadb => DatabaseEngineSql::Mysql {
                charset: "utf8mb4".into(),
            },
            _ => unreachable!("only SQL sources have an engine"),
        }
    }
}

#[derive(Clone)]
enum SourceLocation {
    Excel(String),
    Sql {
        kind: ImportKind,
        connection: String,
    },
}
impl SourceLocation {
    fn request(&self, selection: String) -> DatabaseImportSource {
        match self {
            Self::Excel(path) => DatabaseImportSource::Excel {
                path: path.clone(),
                sheet: selection,
            },
            Self::Sql { kind, connection } => DatabaseImportSource::Sql {
                engine: kind.engine(),
                connection_string: connection.clone(),
                table: selection,
            },
        }
    }
}

enum ImportStage {
    Sources,
    File(ImportKind, String),
    Connection(ImportKind),
    Selection {
        source: SourceLocation,
        choices: Vec<String>,
    },
    Samples {
        entries: Vec<SampleDataset>,
        load_failed: bool,
    },
}
#[derive(Clone, PartialEq, Eq)]
enum ImportTask {
    Picker,
    Discovery,
    Import,
    ImportSample(String),
}

pub struct ImportDialog {
    services: Arc<DataServices>,
    scope: ImportScope,
    stage: ImportStage,
    task: Option<ImportTask>,
    generation: u64,
    name: Entity<InputState>,
    delimiter: Entity<InputState>,
    infer_rows: Entity<InputState>,
    has_header: bool,
    connection: inputs::ConnectionInputs,
    notifications: yss_ui::notifications::Notifications,
    confirming_close: bool,
}
impl ImportDialog {
    pub fn new(
        services: Arc<DataServices>,
        scope: ImportScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            services,
            scope,
            stage: ImportStage::Sources,
            task: None,
            generation: 0,
            name: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(yss_ui::text::t("native.imports.namePlaceholder"))
            }),
            delimiter: cx.new(|cx| InputState::new(window, cx).default_value(",")),
            infer_rows: cx.new(|cx| InputState::new(window, cx).default_value("1000")),
            has_header: true,
            connection: inputs::ConnectionInputs::new(window, cx),
            notifications: yss_ui::notifications::Notifications::new(window),
            confirming_close: false,
        }
    }
    pub fn busy(&self) -> bool {
        self.task.is_some()
    }
    fn choose_kind(&mut self, kind: ImportKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }

        if kind.remote() {
            self.connection.select_kind(kind, window, cx);
            self.stage = ImportStage::Connection(kind);
        } else {
            self.choose_file(kind, window, cx);
        }
        cx.notify();
    }
    fn back(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.stage = match &self.stage {
            ImportStage::Selection {
                source: SourceLocation::Excel(path),
                ..
            } => ImportStage::File(ImportKind::Excel, path.clone()),
            ImportStage::Selection {
                source: SourceLocation::Sql { kind, connection },
                ..
            } => {
                if kind.remote() {
                    ImportStage::Connection(*kind)
                } else {
                    ImportStage::File(*kind, connection.clone())
                }
            }
            _ => ImportStage::Sources,
        };

        cx.notify();
    }
    fn optional_name(&self, cx: &gpui_kit::App) -> Option<String> {
        let name = self.name.read(cx).value().trim().to_owned();
        (!name.is_empty()).then_some(name)
    }
    fn submit_file(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        if self.busy() {
            return;
        }
        let ImportStage::File(kind, path) = &self.stage else {
            return;
        };
        let source = match kind {
            ImportKind::Csv => match self.csv_source(path.clone(), cx) {
                Ok(source) => source,
                Err(error) => {
                    self.notifications.error(error, cx);
                    cx.notify();
                    return;
                }
            },
            ImportKind::Parquet => DatabaseImportSource::Parquet {
                path: path.clone(),
                columns: None,
            },
            _ => {
                self.discover_file(window, cx);
                return;
            }
        };
        self.submit(
            ImportRequest::Source {
                source,
                name: self.optional_name(cx),
            },
            window,
            cx,
        );
    }
    fn select_table(
        &mut self,
        selection: String,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.generation != generation {
            return;
        }
        let ImportStage::Selection { source, choices } = &self.stage else {
            return;
        };
        if !choices.contains(&selection) {
            return;
        }
        let source = source.request(selection);
        self.submit(
            ImportRequest::Source {
                source,
                name: self.optional_name(cx),
            },
            window,
            cx,
        );
    }
    fn submit(&mut self, request: ImportRequest, _: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.task = Some(match &request {
            ImportRequest::Sample { id, .. } => ImportTask::ImportSample(id.clone()),
            ImportRequest::Source { .. } => ImportTask::Import,
        });

        cx.emit(ImportSubmission {
            scope: self.scope.clone(),
            request,
        });
        cx.notify();
    }

    pub fn reject_submission(&mut self, cx: &mut Context<Self>) {
        self.task = None;
        self.notifications
            .error(yss_ui::text::t("native.imports.busy"), cx);
        cx.notify();
    }

    pub fn finished(
        &mut self,
        outcome: Result<(), &'static str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.task = None;
        match outcome {
            Err(key) => self.notifications.error(yss_ui::text::translate(key), cx),
            Ok(()) => yss_ui::modal_window::close_child(window, cx),
        }
        cx.notify();
    }
    pub fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.busy() || self.confirming_close {
            return false;
        }
        if !self.connection.dirty(cx)
            && self.name.read(cx).value().is_empty()
            && self.has_header
            && self.delimiter.read(cx).value() == ","
            && self.infer_rows.read(cx).value() == "1000"
        {
            return true;
        }
        self.confirming_close = true;
        let prompt = yss_ui::modal_window::prompt(
            &yss_ui::text::t("native.imports.closeTitle"),
            Some(&yss_ui::text::t("native.imports.closeMessage")),
            &[
                &yss_ui::text::t("common.close"),
                &yss_ui::text::t("native.imports.keepEditing"),
            ],
            window,
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let choice = prompt.await;
            view.update_in(cx, |view, window, cx| {
                view.confirming_close = false;
                if matches!(choice, Ok(0)) {
                    yss_ui::modal_window::close(window, cx);
                }
                cx.notify();
            })
            .log_released();
        })
        .detach();
        false
    }
}

impl EventEmitter<ImportSubmission> for ImportDialog {}
