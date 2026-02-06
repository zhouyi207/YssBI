use crate::database::dataframe_to_preview_rows;
use polars::prelude::*;

use super::DatabaseAccess;
use super::DatabaseDecl;
use super::DatabaseState;
use super::DatabaseView;

pub struct DatabaseInstance {
    pub decl: DatabaseDecl,
    pub state: DatabaseState,
}

impl DatabaseInstance {
    pub fn get_lazy(&self) -> Option<LazyFrame> {
        match &self.state {
            DatabaseState::Lazy { lazy_frame } => Some(lazy_frame.clone()),
            _ => None,
        }
    }

    pub fn get_preview(&self, n: u32) -> PolarsResult<DataFrame> {
        let lazy = self
            .get_lazy()
            .ok_or_else(|| PolarsError::ComputeError("not in lazy state".into()))?;

        lazy.clone().limit(n).collect()
    }

    pub fn ensure_loaded(&mut self) -> PolarsResult<&DataFrame> {
        let need_load = matches!(self.state, DatabaseState::Lazy { .. });

        if need_load {
            // 1️⃣ 先把 LazyFrame clone 出来（不借用 self.state）
            let lazy = match &self.state {
                DatabaseState::Lazy { lazy_frame } => lazy_frame.clone(),
                _ => unreachable!(),
            };

            // 2️⃣ 真正执行 IO
            let df = lazy.collect()?;

            // 3️⃣ 原地状态切换
            self.state = DatabaseState::Loaded {
                dataframe: df.into(),
            };
        }

        // 4️⃣ 现在才借用 self.state（安全）
        match &self.state {
            DatabaseState::Loaded { dataframe } => Ok(dataframe),
            _ => unreachable!(),
        }
    }

    pub fn access<'a>(&'a mut self, access: DatabaseAccess) -> PolarsResult<DatabaseView> {
        match access {
            DatabaseAccess::Preview => self.preview_view(),
            DatabaseAccess::Execution => self.execution_view(),
        }
    }

    fn preview_view(&mut self) -> PolarsResult<DatabaseView> {
        let n = 100;

        let df = match &self.state {
            DatabaseState::Lazy { lazy_frame } => lazy_frame.clone().limit(n).collect()?,
            DatabaseState::Loaded { dataframe } => dataframe.head(Some(n as usize)),
            DatabaseState::Failed { error } => return Err(PolarsError::NoData(error.clone().into())),
        };

        let rows = dataframe_to_preview_rows(&df);

        Ok(DatabaseView::Preview {
            rows,
            row_count: df.height(),
            column_count: df.width(),
        })
    }

    fn execution_view(&mut self) -> PolarsResult<DatabaseView> {
        let df = self.ensure_loaded()?;

        Ok(DatabaseView::Execution { dataframe: df.clone() })
    }
}
