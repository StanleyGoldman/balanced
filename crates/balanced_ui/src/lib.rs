//! The egui user interface: the Main window, its panels, Dialogs and Detached windows.

mod account_bar;
mod accounts_model;
mod app;
mod command;
mod formatted_amount;
mod i18n;
mod keymap;
mod main_window;
mod register;
mod register_model;
mod route;
#[cfg(feature = "test-support")]
pub mod testing;
mod theme;
mod user_action;

pub use app::App;
