//! API layer: gRPC service, HTTP REST server, and reflect HTTP client.

pub mod grpc;
pub mod grpc_service;
pub mod http;
pub mod reflect_client;

pub use reflect_client::ReflectClient;
