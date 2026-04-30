//! gRPC service implementation for ClawDB.
//!
//! When protoc is available at build time this module exposes a full tonic
//! service implementation. Without protoc the module compiles as an empty stub.

#[cfg(proto_compiled)]
pub mod proto {
    tonic::include_proto!("clawdb.v1");
}

/// Placeholder service struct; the full implementation requires proto-generated types.
pub struct ClawDBGrpcService;

impl ClawDBGrpcService {
    /// Creates a new service instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClawDBGrpcService {
    fn default() -> Self {
        Self::new()
    }
}
