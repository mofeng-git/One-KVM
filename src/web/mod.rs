mod audio_ws;
mod error;
mod handlers;
mod routes;
pub(crate) mod state;
mod static_files;
#[cfg(unix)]
mod uac_ws;
mod ws;

pub use audio_ws::audio_ws_handler;
pub use error::ErrorResponse;
pub use routes::create_router;
#[cfg(not(debug_assertions))]
pub use static_files::StaticAssets;
#[cfg(unix)]
pub use uac_ws::uac_audio_ws_handler;
pub use ws::ws_handler;
