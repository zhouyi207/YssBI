pub trait DatabaseConnection: Send + Sync {
    fn execute(&self, sql: &str) -> Result<(), String>;
    fn close(&self);
}