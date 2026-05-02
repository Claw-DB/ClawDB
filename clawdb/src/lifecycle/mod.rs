//! Small lifecycle helpers exposed by the wrapper crate.

mod shutdown;

pub use shutdown::GracefulShutdown;
