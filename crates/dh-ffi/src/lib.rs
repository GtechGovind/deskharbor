use dh_core::engine::DeskEngine;
use std::sync::OnceLock;
mod api;
mod frb_generated;
mod streams;
mod surfaces;

static ENGINE: OnceLock<DeskEngine> = OnceLock::new();

pub(crate) fn engine() -> Result<DeskEngine, String> {
    ENGINE
        .get()
        .cloned()
        .ok_or_else(|| "Engine is not initialized".to_owned())
}
