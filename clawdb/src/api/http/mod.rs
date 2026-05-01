//! HTTP REST server: axum-based API.

pub mod routes;
pub mod server;

pub use server::serve;
