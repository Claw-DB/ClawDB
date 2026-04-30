//! Internal event bus and event type definitions for the ClawDB runtime.

pub mod bus;
pub mod emitter;
pub mod subscriber;
pub mod types;

pub use bus::EventBus;
pub use emitter::EventEmitter;
pub use subscriber::EventSubscriber;
pub use types::ClawEvent;
