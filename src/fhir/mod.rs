pub mod capability;
pub mod client;

pub use capability::{fetch_capability_statement, fetch_resources_parallel};
pub use client::{
    delete_from_fhir_server, execute_operation, fetch_resource_history, put_to_fhir_server,
    search_fhir_resources,
};

#[allow(unused_imports)]
pub use client::get_from_fhir_server;
