#![allow(async_fn_in_trait)]

pub mod async_task;
pub mod configs;
pub mod db;
pub mod entity;
pub mod http;
pub mod json;
pub mod provider;
pub mod repository;
pub mod result;
pub mod usecase;
pub mod utils;

pub use futures;
pub use macros::*;

#[cfg(feature = "flake-id")]
pub use derive_more;
// pub extern crate derive_more;
#[cfg(feature = "flake-id")]
pub extern crate flaken;
