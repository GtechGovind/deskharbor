use std::sync::OnceLock;
use dh_core::engine::DeskEngine;
mod api;
mod streams;
mod surfaces;
mod frb_generated;

static ENGINE: OnceLock<DeskEngine> = OnceLock::new();

pub(crate) fn engine() -> Result<DeskEngine, String> {
    ENGINE.get().cloned().ok_or_else(|| "Engine is not initialized".to_owned())
}
