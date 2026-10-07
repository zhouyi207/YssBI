//! Read-only file and SQL discovery runs outside the window event loop.
use super::{ImportDialog, ImportKind, ImportStage, ImportTask, SourceLocation};
use gpui::{Context, PathPromptOptions, Window};

impl ImportDialog {
    pub(super) fn choose_file(
        &mut self,
        kind: ImportKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.task = Some(ImportTask::Picker);
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(format!("选择 {} 文件", kind.label()).into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = prompt.await;
            let failed = !matches!(result, Ok(Ok(_)));
            let path = result
                .ok()
                .and_then(Result::ok)
                .flatten()
                .and_then(|paths| paths.into_iter().next());
            let _ = view.update_in(cx, |view, window, cx| {
                view.task = None;
                if failed {
                    view.error = Some("文件选择器未打开，请重试或检查桌面文件选择服务。".into());
                }
                if let Some(path) = path {
                    if let Some(path) = path.to_str() {
                        view.stage = ImportStage::File(kind, path.to_owned());
                        view.error = None;
                        if matches!(kind, ImportKind::Excel | ImportKind::Sqlite) {
                            view.discover_file(window, cx);
                        }
                    } else {
                        view.error = Some("文件路径无法识别，请使用有效的文字路径。".into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn discover_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ImportStage::File(kind, path) = &self.stage else {
            return;
        };
        let source = match kind {
            ImportKind::Excel => SourceLocation::Excel(path.clone()),
            ImportKind::Sqlite => SourceLocation::Sql {
                kind: *kind,
                connection: path.clone(),
            },
            _ => return,
        };
        self.discover(source, window, cx);
    }
    pub(super) fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ImportStage::Connection(kind) = self.stage else {
            return;
        };
        match self.connection.url(kind, cx) {
            Ok(connection) => self.discover(SourceLocation::Sql { kind, connection }, window, cx),
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }
    fn discover(&mut self, source: SourceLocation, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.task = Some(ImportTask::Discovery);
        self.error = None;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let query = source.clone();
        let job = self.services.run(move |_| {
            use yss_application::database::{
                list_excel_sheets, list_sql_tables, list_sqlite_tables,
            };
            Ok(match query {
                SourceLocation::Excel(path) => list_excel_sheets(&path)?,
                SourceLocation::Sql {
                    kind: ImportKind::Sqlite,
                    connection,
                } => list_sqlite_tables(&connection)?,
                SourceLocation::Sql { kind, connection } => list_sql_tables(
                    if kind == ImportKind::Postgres {
                        "postgres"
                    } else {
                        "mysql"
                    },
                    &connection,
                )?,
            })
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, _, cx| {
                if view.generation != generation {
                    return;
                }
                view.task = None;
                match result {
                    Some(choices) if !choices.is_empty() => {
                        view.stage = ImportStage::Selection { source, choices };
                    }
                    Some(_) => view.error = Some("没有发现可导入的表或工作表。请检查来源。".into()),
                    None => {
                        view.error =
                            Some("来源未读取，输入已保留。请检查文件、连接配置或访问权限。".into())
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn samples(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.task = Some(ImportTask::Discovery);
        self.error = None;
        self.stage = ImportStage::Samples(vec![]);
        let job = self.services.run(|services| Ok(services.samples.list()?));
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, _, cx| {
                view.task = None;
                if let Some(samples) = result {
                    view.stage = ImportStage::Samples(samples);
                } else {
                    view.error = Some("示例数据目录未读取，请检查安装资源后重试。".into());
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
