use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DatabaseEngine {
    /// SQLite（本地文件）
    Sqlite {
        /// 是否允许自动创建
        auto_create: bool,
    },

    /// PostgreSQL
    Postgres {
        /// 是否要求 SSL
        ssl: bool,
    },

    /// MySQL / MariaDB
    Mysql {
        /// 字符集
        charset: String,
    },

    /// 内存数据库（测试 / demo）
    InMemory,
}
