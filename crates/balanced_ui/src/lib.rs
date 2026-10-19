//! The egui user interface: the Main window, its panels, Dialogs and Detached windows.

mod account_bar;
mod accounts_model;
mod amount_field;
mod app;
mod clock;
mod combobox;
mod command;
mod data_file;
mod date_field;
mod dialog;
mod draft_row;
mod formatted_amount;
mod i18n;
mod keymap;
mod layout;
mod main_window;
mod manage_accounts;
mod menu_nav;
mod new_account;
mod new_category;
mod preferences;
mod preferences_dialog;
mod recent_files;
mod register;
mod register_columns;
mod register_model;
mod route;
#[cfg(feature = "test-support")]
pub mod testing;
mod theme;
mod triangle_button;
mod user_action;
mod welcome;

pub use app::App;
pub use clock::Clock;
pub use preferences::PreferencesFile;
pub use recent_files::RecentFilesFile;
