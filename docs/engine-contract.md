# Concurrent engine, UI and native surface contract

This is the interface handoff for application-plan Tasks7–9. The approved product design remains binding. Changes here require a message to the controller and affected implementer before editing consumers. Files are divided by owner; root Cargo.toml/Cargo.lock and workflows are controller-owned.

## Core engine (Task7 owner)

Create `dh_core::engine::DeskEngine` (Clone, Send+Sync; internal Arc ownership).
Use `EngineResult<T> = Result<T, EngineError>` with error fields `code:String,message:String`; Display must never include passwords/clipboard/file contents. DTOs are Clone and have the exact public fields below:

```rust
DisplayInfo { id:String, name:String, width:u32, height:u32, x:i32, y:i32 }
SessionInfo { id:u32, endpoint:String, state:String, transport:String, host_identity:String,
              epoch:u64, layout:u64, displays:Vec<DisplayInfo>, selected_display:String }
HostInfo { state:String, running:bool, password_configured:bool, unattended_available:bool,
           addresses:Vec<String>, detail:String }
HostSettings { enabled:bool, bind_address:String, allow_input:bool, clipboard:bool,
               files:bool, transfer_directory:Option<String> }
SavedDevice { name:String, address:String, identity:Option<String> }
TransferInfo { id:u32, session_id:u32, filename:String, total:u64,
               transferred:u64, state:String, checksum:String }
InputRequest { epoch:u64, layout:u64, display_id:String, kind:InputKind }
InputKind = Pointer { x:f64,y:f64,buttons:u32,wheel_x:i32,wheel_y:i32 }
          | Key { usage:u16,pressed:bool } | Text(String) | ReleaseAll
EngineEvent = SessionChanged(SessionInfo) | HostChanged(HostInfo)
            | TransferChanged(TransferInfo)
            | FrameReady { session_id:u32, sequence:u64,width:u32,height:u32 }
            | Error { session_id:Option<u32>,code:String,message:String }
```

Exact engine API (all methods `&self` unless constructor):

```rust
fn new(data_dir:std::path::PathBuf) -> EngineResult<Self>;
fn subscribe(&self) -> tokio::sync::broadcast::Receiver<EngineEvent>;
fn latest_frame(&self, session_id:u32) -> Option<std::sync::Arc<dh_media::codecs::vp8::DecodedFrame>>;
async fn connect(&self, endpoint:String, password:zeroize::Zeroizing<Vec<u8>>) -> EngineResult<SessionInfo>;
async fn disconnect(&self, session_id:u32) -> EngineResult<()>;
async fn send_input(&self, session_id:u32, event:InputRequest) -> EngineResult<()>;
async fn select_monitor(&self, session_id:u32, display_id:String) -> EngineResult<()>;
async fn clipboard_send_text(&self, session_id:u32, text:String) -> EngineResult<()>;
async fn clipboard_request_text(&self, session_id:u32) -> EngineResult<String>;
async fn start_transfer(&self, session_id:u32, local_path:String) -> EngineResult<TransferInfo>;
async fn cancel_transfer(&self, transfer_id:u32) -> EngineResult<()>;
async fn host_status(&self) -> EngineResult<HostInfo>;
async fn configure_host(&self, settings:HostSettings) -> EngineResult<HostInfo>;
async fn set_host_password(&self, password:zeroize::Zeroizing<Vec<u8>>) -> EngineResult<()>;
fn devices(&self) -> EngineResult<Vec<SavedDevice>>;
fn save_device(&self, device:SavedDevice) -> EngineResult<()>;
fn forget_device(&self, address:String) -> EngineResult<()>;
fn transfers(&self) -> Vec<TransferInfo>;
```

State values are explicit strings: `connecting`, `authenticating`, `connected`, `reconnecting`, `disconnected`, `ready`, `sign_in`, `permission_needed`, `no_display`, `host_busy`, `stopped`. Unknown/failure states must show a useful error, never imply success.

The controller owns `dh-media` VP8 implementation. Its feature `vp8` provides `Encoder::new(width,height,kbps)`, `encode(&mut self,rgba:&[u8],force:bool)->Result<EncodedFrame,CodecError>`, `Decoder::new()`, `decode(&mut self,packet:&[u8])->Result<DecodedFrame,CodecError>`, `mark_lost()`. DecodedFrame has `rgba:Vec<u8>,width:u32,height:u32`; EncodedFrame adds `data:Vec<u8>,keyframe:bool,width,height`. Do not write a placeholder encoder or send pixels through Dart.

Task7 owns all `dh-core` implementation, additions to `dh-net`/`dh-protocol`, the three user-data crates as needed, new `bins/dh-hostd`, `bins/dh-cli`, and its core/API/daemon tests. Task7 may add ordinary-session platform adapters in new files under `dh-platform/src/runtime/` and a module declaration, without changing existing probe/harness security. Privileged installed worker activation still requires the native proof gates. Do not inject input, sign out, install a service, or capture the user's real screen in automated tests. Use a deliberately labelled synthetic backend for executable tests.

## Flutter/bridge (Task8 owner)

Task8 owns `apps/client`, `packages/engine_bridge`, `crates/dh-ffi/{Cargo.toml,src/{lib.rs,api.rs,streams.rs,frb_generated.rs}}` and generated bridge files. It consumes exactly the API above; coordinate missing implementation with Task7 rather than inventing production success returns. Root Cargo changes go through the controller. `dh-ffi` is a cdylib+staticlib+rlib and depends on dh-core (feature `vp8`), dh-media (feature `vp8`), flutter_rust_bridge2.13.0, Tokio and zeroize.

It must expose `pub(crate) fn engine() -> Result<DeskEngine,String>` in dh-ffi (DeskEngine clone), and include `mod surfaces;` for Task9's separate file. Engine initialization happens once with the application support directory supplied through path_provider. Async FRB methods wrap the exact core API and forward errors. Events carry small metadata, never pixel buffers.

The required UI is Material3 navy/teal, system light/dark, desktop sidebar/Android bottom navigation, all Connect/Host/Devices/Transfers/Settings/Permissions/Session screens. Input forwards only with remote canvas focus and current epoch/layout; Android has direct-touch/trackpad, keyboard and modifiers. Display readiness and missing native/installed capabilities candidly. Clear password UI/controller state; no saved passwords. Use a texture surface from Task9.

Pinned SDK at `C:/Users/govin/Documents/Codex/2026-11-08/de/work/tools/flutter`, FRB generator at `.../work/tools/frb-2.13.0/flutter_rust_bridge_codegen.exe`. Flutter3.47.9/Dart3.13.7 verified. Riverpod3.5.0/go_router18.1.3/FRB2.13.0 verified. Local Windows desktop CMake integration is absent, so builds may need hosted runners. Do not modify the user's global Flutter/VS configuration.

## Native surfaces (Task9 owner)

Task9 owns `packages/native_surfaces` and `crates/dh-ffi/src/surfaces.rs` only. No edits to FFI lib/api or Flutter app. Dart contract:

```dart
class NativeSurface {
  final int textureId;
  static Future<NativeSurface> create(int sessionId);
  Future<void> present();
  Future<void> dispose();
}
```

Method channel `deskharbor/native_surfaces`: `createTexture` args `{sessionId:int}` returns textureId; `present`/`disposeTexture` args `{textureId:int}`. Rust bridge's global engine accessor above supplies latest_frame. Implement native Flutter textures on all four platforms; no base64/images through Dart. Use dynamic loading of the same dh_ffi library, never a second copy of engine state. Missing library/frame gives explicit plugin error/blank clear, not a fake video frame.

C ABI implemented in surfaces.rs and declared in a shared native header:

```c
typedef struct DhFrame {
  const unsigned char *data;
  size_t length;
  unsigned int width, height, stride;
  void *lease;
} DhFrame;
bool dh_frame_acquire(unsigned int session_id, DhFrame *out);
void dh_frame_release(void *lease);
```

Frames are tightly packed RGBA8. Acquire creates a Box holding Arc<DecodedFrame> so bytes remain alive until exactly one release; no mutable alias or copied pixel payload in a Dart message. Validate dimensions/length/stride before exposing pointers. Exported unsafe FFI methods document caller obligations and handle null pointers. Native plugin callbacks must release leases at the renderer's completion boundary, including shutdown/error paths. Windows uses PixelBufferTexture; Linux FlPixelBufferTexture; macOS CVPixelBuffer/FlutterTexture; Android SurfaceTexture/SurfaceProducer with JNI/ANativeWindow. Preserve RGBA/BGRA channel semantics and orientation.

Package plugins must build with the standard generated Flutter runners. Rust libraries will be packaged beside Windows executables, under Linux bundle/lib, macOS Contents/Frameworks, and Android jniLibs. Report any architecture-specific constraints explicitly.
