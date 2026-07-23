mod audio_ws;
mod error;
mod handlers;
mod routes;
mod static_files;
mod uac_ws;
mod ws;

pub use audio_ws::audio_ws_handler;
pub use error::ErrorResponse;
pub use routes::create_router;
#[cfg(not(debug_assertions))]
pub use static_files::StaticAssets;
pub use uac_ws::uac_audio_ws_handler;
pub use ws::ws_handler;
