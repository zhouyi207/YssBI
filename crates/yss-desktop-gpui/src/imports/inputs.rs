//! Native form values become the existing typed import request only on explicit confirmation.
use super::{ImportDialog, ImportKind};
use gpui::{App, AppContext, Context, Entity, Window};
use gpui_component::input::InputState;
use yss_database_contract::DatabaseImportSource;

pub(super) struct ConnectionInputs {
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
            host: cx.new(|cx| InputState::new(window, cx).default_value("localhost")),
            port: cx.new(|cx| InputState::new(window, cx).default_value("5432")),
            user: cx.new(|cx| InputState::new(window, cx).placeholder("用户名")),
            password: cx.new(|cx| InputState::new(window, cx).masked(true)),
            database: cx.new(|cx| InputState::new(window, cx).placeholder("数据库名称")),
            raw_url: cx.new(|cx| InputState::new(window, cx).masked(true)),
            raw: false,
        }
    }
    pub fn dirty(&self, cx: &App) -> bool {
        self.host.read(cx).value() != "localhost"
            || !matches!(self.port.read(cx).value().as_ref(), "5432" | "3306")
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
            let url = url::Url::parse(&value).map_err(|_| "请输入有效的连接字符串。")?;
            if !(url.scheme() == scheme
                || kind == ImportKind::Postgres && url.scheme() == "postgresql")
                || url.host_str().is_none()
            {
                return Err("连接字符串与所选数据库类型不匹配。".into());
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
            .ok_or("请输入 1 到 65535 之间的端口。")?;
        let host = self.host.read(cx).value().trim().to_owned();
        let user = self.user.read(cx).value().trim().to_owned();
        let database = self.database.read(cx).value().trim().to_owned();
        if host.is_empty() || user.is_empty() || database.is_empty() {
            return Err("请填写主机、用户名和数据库名称。".into());
        }
        let mut url = url::Url::parse(&format!("{scheme}://localhost/")).expect("SQL URL base");
        url.set_host(Some(&host))
            .map_err(|_| "主机地址无法识别。")?;
        url.set_port(Some(port)).map_err(|_| "端口无法识别。")?;
        url.set_username(&user).map_err(|_| "用户名无法识别。")?;
        let password = self.password.read(cx).value();
        url.set_password((!password.is_empty()).then_some(password.as_ref()))
            .map_err(|_| "连接配置无法识别。")?;
        url.path_segments_mut()
            .map_err(|_| "数据库名称无法识别。")?
            .clear()
            .push(&database);
        Ok(url.to_string())
    }
}
impl ImportDialog {
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
            .ok_or("分隔符应为单个 ASCII 字符。")?;
        let infer_rows = self
            .infer_rows
            .read(cx)
            .value()
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or("类型推断行数应为正整数。")?;
        Ok(DatabaseImportSource::Csv {
            path,
            delimiter,
            has_header: self.has_header,
            infer_schema_length: Some(infer_rows),
        })
    }
}
