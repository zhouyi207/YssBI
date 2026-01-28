use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericOutDataPin, GenericInDataPin};

pub fn register(registry: &NodeRegistry) {
    // 1. Get DataFrame
    let get_df = GenericNode::new_prototype("get_dataframe", "Get DataFrame");
    get_df.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "DataFrame", "dataframe"));
    
    let mut get_df = get_df;
    get_df.set_metadata(vec!["Data".into()], "default".into(), Some("Get a loaded DataFrame".into()));
    registry.register("get_dataframe".into(), Arc::new(get_df));

    // 2. Get Column
    let get_col = GenericNode::new_prototype("get_column", "Get Column");
    get_col.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "DataFrame", "dataframe"));
    get_col.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Column", "object"));
    
    let mut get_col = get_col;
    get_col.set_metadata(vec!["Data".into()], "default".into(), Some("Get a column from a DataFrame".into()));
    registry.register("get_column".into(), Arc::new(get_col));
}
