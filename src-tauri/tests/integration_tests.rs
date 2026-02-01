//! 集成测试模块
//! 
//! 包含所有集成测试的入口点

mod integration {
    pub mod project_tests;
    pub mod schema_pin_types_tests;
    pub mod schema_variables_tests;
    pub mod state_project_state_tests;
    pub mod state_subgraph_crud_tests;
    pub mod type_inference_api_tests;
}