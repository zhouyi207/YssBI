//! Import inputs and discovery are transient; Application owns every imported dataset.
mod discovery;
mod feedback;
mod inputs;
mod render;
mod samples;
mod selection;

pub(crate) use feedback::{IMPORT_FAILED, sample_import_failure};

use crate::{services::NativeServices, workbench::Workbench};
use gpui::{AppContext, Context, Entity, PromptLevel, WeakEntity, Window};
use gpui_component::{WindowExt, input::InputState};
use std::sync::Arc;
use yss_application::database::samples::SampleDataset;
use yss_database_contract::{DatabaseEngineSql, DatabaseImportSource};
use yss_project_identity::ProjectInstanceId;

#[derive(Clone)]
pub(crate) struct ImportScope {
    pub project: ProjectInstanceId,
    pub lifecycle: u64,
}

pub(crate) enum ImportRequest {
    Source {
        source: DatabaseImportSource,
        name: Option<String>,
    },
    Sample {
        id: String,
        version: u32,
    },
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
    fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Parquet => "Parquet",
            Self::Excel => "Excel",
            Self::Sqlite => "SQLite",
            Self::Postgres => "PostgreSQL",
            Self::Mysql => "MySQL",
            Self::Mariadb => "MariaDB",
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

pub(crate) struct ImportDialog {
    services: Arc<NativeServices>,
    owner: WeakEntity<Workbench>,
    scope: ImportScope,
    stage: ImportStage,
    category: usize,
    task: Option<ImportTask>,
    generation: u64,
    name: Entity<InputState>,
    delimiter: Entity<InputState>,
    infer_rows: Entity<InputState>,
    has_header: bool,
    connection: inputs::ConnectionInputs,
    error: Option<String>,
    confirming_close: bool,
}
impl ImportDialog {
    pub(crate) fn new(
        services: Arc<NativeServices>,
        owner: WeakEntity<Workbench>,
        scope: ImportScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            services,
            owner,
            scope,
            stage: ImportStage::Sources,
            category: 0,
            task: None,
            generation: 0,
            name: cx.new(|cx| InputState::new(window, cx).placeholder("留空以使用来源名称")),
            delimiter: cx.new(|cx| InputState::new(window, cx).default_value(",")),
            infer_rows: cx.new(|cx| InputState::new(window, cx).default_value("1000")),
            has_header: true,
            connection: inputs::ConnectionInputs::new(window, cx),
            error: None,
            confirming_close: false,
        }
    }
    pub(crate) fn busy(&self) -> bool {
        self.task.is_some()
    }
    fn choose_kind(&mut self, kind: ImportKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.error = None;
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
        self.error = None;
        cx.notify();
    }
    fn optional_name(&self, cx: &gpui::App) -> Option<String> {
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
                    self.error = Some(error);
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
    fn submit(&mut self, request: ImportRequest, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let dialog = cx.entity().downgrade();
        let task = match &request {
            ImportRequest::Sample { id, .. } => ImportTask::ImportSample(id.clone()),
            ImportRequest::Source { .. } => ImportTask::Import,
        };
        let started = owner.update(cx, |view, cx| {
            view.start_database_import(self.scope.clone(), request, dialog, window, cx)
        });
        if started {
            self.task = Some(task);
            self.error = None;
        } else {
            self.error = Some("项目仍在读取或提交，请稍后重试。".into());
        }
        cx.notify();
    }

    pub(crate) fn finished(
        &mut self,
        outcome: Result<(), &'static str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.task = None;
        match outcome {
            Err(key) => self.error = Some(crate::text::translate(key)),
            Ok(()) => window.close_dialog(cx),
        }
        cx.notify();
    }
    pub(crate) fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
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
        let prompt = window.prompt(
            PromptLevel::Warning,
            "关闭导入窗口？",
            Some("尚未导入的配置与连接输入将被放弃。"),
            &["关闭", "继续编辑"],
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let choice = prompt.await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.confirming_close = false;
                if matches!(choice, Ok(0)) {
                    window.close_dialog(cx);
                }
                cx.notify();
            });
        })
        .detach();
        false
    }
}
