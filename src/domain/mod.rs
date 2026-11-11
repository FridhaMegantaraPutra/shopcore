//! Domain layer: entities, value objects and business rules.
//! Pure Rust — no database, HTTP or async code lives here.

pub mod error;
pub mod money;
pub mod order;
pub mod report;
pub mod settlement;
pub mod wallet;

pub use error::DomainError;
