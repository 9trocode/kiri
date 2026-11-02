//! Images on the host: references, registry pulls and the blob store.

#![forbid(unsafe_code)]

pub mod archive;
pub mod b64;
pub mod cancel;
mod connect;
pub mod reference;
pub mod registry;
pub mod store;

pub use cancel::Cancel;
pub use reference::Reference;
pub use registry::{Client, Credentials, Platform, Progress, PushProgress, set_proxy_source};
pub use store::{Image, Layer, RunConfig, Store};
