//! ShopCore, structured in DDD layers. Dependencies point inward only:
//!
//! ```text
//! interfaces (HTTP, CLI) ──▶ application (use cases) ──▶ domain (rules)
//!                                   │
//!                                   ▼
//!                       infrastructure (Postgres, HMAC)
//! ```

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
