//! # DataStore Module
//!
//! This module provides functionality for interacting with a SQLite database
//! to store and retrieve information about YouTube streams and their closed captions.
//!
//! The module uses sqlx for database operations and provides an abstraction layer
//! for CRUD operations on streams and their associated closed captions.

mod datastore;
mod domain;

// pub use datastore::DataStore;
pub use datastore::postgres::PgDataStore;
pub use datastore::{BulkInsertResult, DataStore};
pub use domain::{Stream, StreamCategory};
