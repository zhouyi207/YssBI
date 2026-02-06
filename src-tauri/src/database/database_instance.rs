use super::DatabaseEngine;
use super::DatabaseConnection;

pub struct DatabaseInstance {
    pub engine: DatabaseEngine,
    pub connection: Box<dyn DatabaseConnection>,
}
