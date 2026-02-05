pub struct ProjectStore {
    item: String
}

impl ProjectStore {
    pub fn new() -> Self {
        Self {
            item: String::new()
        }
    }
}