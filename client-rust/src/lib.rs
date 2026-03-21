//! Key-Manager Rust Client

mod private_kv;
mod outlayer_adapter;
mod fastkv_adapter;
mod crypto;
mod types;
mod error;

pub use private_kv::{PrivateKV, KeyValueStore};
pub use outlayer_adapter::{OutLayerAdapter, TeeAdapter};
pub use fastkv_adapter::{FastKVAdapter, StorageAdapter};
pub use types::*;
pub use error::{Error, Result};
pub use crypto::{Encryptor, serialize_encrypted, deserialize_encrypted};
