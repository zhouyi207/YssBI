use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectProgressDto {
    Scanning,
    Discovered { count: usize },
    Registering { current: usize, total: usize },
    Checking { current: usize, total: usize },
    Removing { removed: usize, total: usize },
}
