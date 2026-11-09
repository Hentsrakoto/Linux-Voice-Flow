pub mod app_state;
pub mod commands;
pub mod hotkey;
#[cfg(any(feature = "desktop", test))]
mod hotkey_relay;
pub mod protocol;

#[cfg(feature = "desktop")]
mod desktop;

#[cfg(feature = "desktop")]
pub use desktop::run;

pub mod audio;
pub mod engine;
pub mod temp_audio;

pub mod dictation;

pub mod models;
pub mod focus;
pub mod insertion;
pub mod settings;
