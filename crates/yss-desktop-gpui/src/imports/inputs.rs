//! Native form values become the existing typed import request only on explicit confirmation.
use super::{ImportDialog, ImportKind, ImportStage};
use gpui_kit::component::input::{Enter, InputState};
use gpui_kit::{App, AppContext, Context, Entity, EntityInputHandler, Focusable, Window};
use yss_database_contract::DatabaseImportSource;

pub(super) struct ConnectionInputs {
    kind: ImportKind,
    pub host: Entity<InputState>,
    pub port: Entity<InputState>,
    pub user: Entity<InputState>,
    pub password: Entity<InputState>,
    pub database: Entity<InputState>,
    pub raw_url: Entity<InputState>,
    pub raw: bool,
}
impl ConnectionInputs {
    pub fn new(window: &mut Window, cx: &mut Context<ImportDialog>) -> Self {
        Self {
            kind: ImportKind::Postgres,
            host: cx.new(|cx| InputState::new(window, cx).default_value("localhost")),
            port: cx.new(|cx| InputState::new(window, cx).default_value("5432")),
            user: cx.new(|cx| {
                InputState::new(window, cx).placeholder(crate::text::t("importModal.username"))
            }),
            password: cx.new(|cx| InputState::new(window, cx).masked(true)),
            database: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(crate::text::t("native.imports.databaseName"))
            }),
            raw_url: cx.new(|cx| InputState::new(window, cx).masked(true)),
            raw: false,
        }
    }
    pub fn select_kind(
        &mut self,
        kind: ImportKind,
        window: &mut Window,
        cx: &mut Context<ImportDialog>,
    ) {
        if self.kind == kind {
            return;
        }
        // Only a previous default follows the engine; keep the user's custom port.
        if self.port.read(cx).value() == default_port(self.kind) {
            self.port.update(cx, |input, cx| {
                input.set_value(default_port(kind), window, cx)
            });
        }
        self.kind = kind;
    }
    pub fn dirty(&self, cx: &App) -> bool {
        self.host.read(cx).value() != "localhost"
            || self.port.read(cx).value() != default_port(self.kind)
            || [&self.user, &self.password, &self.database, &self.raw_url]
                .into_iter()
                .any(|input| !input.read(cx).value().is_empty())
    }
    pub fn url(&self, kind: ImportKind, cx: &App) -> Result<String, String> {
        let scheme = if kind == ImportKind::Postgres {
            "postgres"
        } else {
            "mysql"
        };
        if self.raw {
            let value = self.raw_url.read(cx).value().trim().to_owned();
            let url = url::Url::parse(&value)
                .map_err(|_| crate::text::t("native.imports.invalidConnectionString"))?;
            if !(url.scheme() == scheme
                || kind == ImportKind::Postgres && url.scheme() == "postgresql")
                || url.host_str().is_none()
            {
                return Err(crate::text::t("native.imports.connectionTypeMismatch").into());
            }
            return Ok(value);
        }
        let port = self
            .port
            .read(cx)
            .value()
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|port| *port > 0)
            .ok_or(crate::text::t("native.imports.invalidPort"))?;
        let host = self.host.read(cx).value().trim().to_owned();
        let user = self.user.read(cx).value().trim().to_owned();
        let database = self.database.read(cx).value().trim().to_owned();
        if host.is_empty() || user.is_empty() || database.is_empty() {
            return Err(crate::text::t("native.imports.connectionRequired").into());
        }
        let mut url = url::Url::parse(&format!("{scheme}://localhost/")).expect("SQL URL base");
        url.set_host(Some(&host))
            .map_err(|_| crate::text::t("native.imports.invalidHost"))?;
        url.set_port(Some(port))
            .map_err(|_| crate::text::t("native.imports.unrecognizedPort"))?;
        url.set_username(&user)
            .map_err(|_| crate::text::t("native.imports.invalidUsername"))?;
        let password = self.password.read(cx).value();
        url.set_password((!password.is_empty()).then_some(password.as_ref()))
            .map_err(|_| crate::text::t("native.imports.invalidConnection"))?;
        url.path_segments_mut()
            .map_err(|_| crate::text::t("native.imports.invalidDatabaseName"))?
            .clear()
            .push(&database);
        Ok(url.to_string())
    }
}

fn default_port(kind: ImportKind) -> &'static str {
    if kind == ImportKind::Postgres {
        "5432"
    } else {
        "3306"
    }
}
impl ImportDialog {
    pub(super) fn submit_input(
        &mut self,
        action: &Enter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if action.secondary || action.shift {
            cx.propagate();
            return;
        }
        if self.busy() {
            return;
        }
        let fields: &[&Entity<InputState>] = match self.stage {
            ImportStage::Connection(_) => &[
                &self.name,
                &self.connection.host,
                &self.connection.port,
                &self.connection.user,
                &self.connection.password,
                &self.connection.database,
                &self.connection.raw_url,
            ],
            ImportStage::File(..) => &[&self.name, &self.delimiter, &self.infer_rows],
            _ => {
                cx.propagate();
                return;
            }
        };
        let Some(input) = fields
            .iter()
            .find(|input| input.focus_handle(cx).is_focused(window))
        else {
            cx.propagate();
            return;
        };
        if input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return;
        }
        match self.stage {
            ImportStage::Connection(_) => self.connect(window, cx),
            ImportStage::File(..) => self.submit_file(cx, window),
            _ => unreachable!("only active input forms handle Enter"),
        }
    }

    pub(super) fn csv_source(
        &self,
        path: String,
        cx: &App,
    ) -> Result<DatabaseImportSource, String> {
        let delimiter = self.delimiter.read(cx).value();
        let mut chars = delimiter.chars();
        let delimiter = chars
            .next()
            .filter(|ch| ch.is_ascii() && !matches!(ch, '\n' | '\r'))
            .filter(|_| chars.next().is_none())
            .ok_or(crate::text::t("native.imports.invalidDelimiter"))?;
        let infer_rows = self
            .infer_rows
            .read(cx)
            .value()
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or(crate::text::t("native.imports.invalidInferenceRows"))?;
        Ok(DatabaseImportSource::Csv {
            path,
            delimiter,
            has_header: self.has_header,
            infer_schema_length: Some(infer_rows),
        })
    }
}
