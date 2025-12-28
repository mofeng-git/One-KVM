mod audio_ws;
mod routes;
mod handlers;
mod static_files;
mod ws;

pub use audio_ws::audio_ws_handler;
pub use routes::create_router;
// StaticAssets is only available in release mode (embedded assets)
#[cfg(not(debug_assertions))]
pub use static_files::StaticAssets;
pub use ws::ws_handler;
