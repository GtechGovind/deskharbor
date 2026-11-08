extern crate self as dh_media;
pub mod codecs {
    pub mod vp8 {
        pub struct DecodedFrame {
            pub rgba: Vec<u8>,
            pub width: u32,
            pub height: u32,
        }
    }
}
struct Engine;
impl Engine {
    fn latest_frame(&self, _id: u32) -> Option<std::sync::Arc<codecs::vp8::DecodedFrame>> {
        None
    }
}
fn engine() -> Result<Engine, String> {
    Err("uninitialized".into())
}
#[path = "../../../../crates/dh-ffi/src/surfaces.rs"]
mod surfaces;
