mod event_project;
mod event_resource;
pub use event_project::EventProject;
pub use event_resource::EventResource;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Event {
    // 项目级事件
    Project(EventProject),

    // 统一资源事件
    Resource(EventResource),
}
