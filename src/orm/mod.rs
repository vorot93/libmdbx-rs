//! Fully typed ORM based on libmdbx.
//!
//! Much simpler in usage but slightly more limited.
//!
//! Unsigned integer keys (`u8`–`u128`) are ordered by their big-endian
//! encoding; signed integers need an order-preserving (e.g. zig-zag)
//! encoding, which is left to users.
//!
//! ```rust,no_run
//! use libmdbx::orm::{DatabaseChart, table, table_info};
//! use std::sync::Arc;
//!
//! // Define the users table
//! table!(
//!     /// Table with users info.
//!     ( Users ) u64 => String
//! );
//!
//! // Assemble database chart
//! let tables: Arc<DatabaseChart> = Arc::new([table_info!(Users)].into_iter().collect());
//!
//! // Create database with the database chart
//! let db: libmdbx::orm::Database = libmdbx::orm::Database::create(None, &tables).unwrap();
//!
//! let users = [(1, "l33tc0der".to_string()), (2, "lameguy".to_string())];
//!
//! let tx = db.begin_readwrite().unwrap();
//!
//! // Insert user info into table
//! for (id, nickname) in &users {
//!     tx.upsert::<Users>(*id, nickname.clone()).unwrap();
//! }
//!
//! // Read a value back
//! assert_eq!(
//!     tx.get::<Users>(1).unwrap(),
//!     Some("l33tc0der".to_string())
//! );
//!
//! // Walk over table and collect its contents
//! assert_eq!(
//!     tx.cursor::<Users>()
//!         .unwrap()
//!         .walk(None)
//!         .collect::<libmdbx::Result<Vec<_>>>()
//!         .unwrap(),
//!     users
//! );
//!
//! tx.commit().unwrap();
//! ```

mod cursor;
mod database;
mod impls;
mod traits;
mod transaction;

pub use self::{cursor::*, database::*, impls::*, traits::*, transaction::*};
pub use crate::{
    DatabaseKind, DatabaseOptions, Mode, NoWriteMap, RO, RW, ReadWriteOptions, SyncMode,
    TransactionKind, WriteMap, dupsort, table, table_info,
};
