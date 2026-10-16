//! The egui user interface: the Main window, its panels, Dialogs and Detached windows.

mod account_bar;
mod accounts_model;
mod amount_field;
mod app;
mod clock;
mod combobox;
mod command;
mod date_field;
mod dialog;
mod formatted_amount;
mod i18n;
mod keymap;
mod layout;
mod main_window;
mod menu_nav;
mod register;
mod register_model;
mod register_tabs;
mod route;
#[cfg(feature = "test-support")]
pub mod testing;
mod theme;
mod user_action;

pub use app::App;
pub use clock::Clock;
