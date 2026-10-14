//! The egui user interface: the Main window, its panels, Dialogs and Detached windows.

mod i18n;
#[expect(
    dead_code,
    reason = "the reference shell's widgets are its first callers"
)]
mod theme;
