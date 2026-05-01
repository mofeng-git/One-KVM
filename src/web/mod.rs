mod audio_ws;
mod error;
mod handlers;
mod routes;
mod static_files;
mod ws;

pub use audio_ws::audio_ws_handler;
pub use error::ErrorResponse;
pub use routes::create_router;
#[cfg(not(debug_assertions))]
pub use static_files::StaticAssets;
pub use ws::ws_handler;
