//! [`App`]: the eframe application, which owns the ledger and the Main window.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use balanced_core::{CurrencyCode, Ledger};
use balanced_i18n::Localizer;
use eframe::egui;

use crate::clock::Clock;
use crate::command::{AppCommand, CommandSender, StatusMessage};
use crate::data_file::DataFileRefusal;
use crate::i18n::fl;
use crate::layout::Layout;
use crate::main_window::{MainWindow, WindowFrame};
use crate::preferences::{Preferences, PreferencesFile};
use crate::theme::{Theme, ThemeSettings};

/// Balanced's eframe application.
#[derive(Debug)]
pub struct App {
    i18n: Localizer,
    ledger: Ledger,
    /// Where the open data file is, or `None` while the App runs on data with no file.
    data_file: Option<PathBuf>,
    preferences_file: PreferencesFile,
    /// The Preferences in use.
    preferences: Preferences,
    /// The look, built from the open Preferences Dialog's draft, or else from the
    /// Preferences in use.
    theme: Theme,
    clock: Clock,
    commands: CommandSender,
    command_receiver: Receiver<AppCommand>,
    main_window: MainWindow,
}

impl App {
    /// The App over `ledger`, showing text through `i18n`, reading today's date from
    /// `clock` and keeping the Preferences in `preferences_file`.
    ///
    /// A preferences file that can't be read is renamed to `preferences.toml.broken`,
    /// and the App starts from the default Preferences and says so in the Status bar.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        i18n: Localizer,
        ledger: Ledger,
        clock: Clock,
        preferences_file: PreferencesFile,
    ) -> Self {
        let (preferences, problem) = match preferences_file.load() {
            Ok(preferences) => (preferences, None),
            Err(error) => {
                tracing::warn!(
                    ?error,
                    "the preferences file can't be read; using the defaults"
                );
                let message = match preferences_file.set_aside() {
                    Ok(()) => StatusMessage::PreferencesReset,
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            "setting the unreadable preferences file aside failed"
                        );
                        StatusMessage::PreferencesUnreadable
                    }
                };
                (Preferences::default(), Some(message))
            }
        };
        let theme = Theme::new(ThemeSettings::from(&preferences));
        apply_theme(&cc.egui_ctx, &theme);
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::Title(fl!(
                &i18n,
                "main-window-title"
            )));
        let (commands, command_receiver) = CommandSender::new(cc.egui_ctx.clone());
        let layout = Layout::load(cc.storage);
        let mut main_window = MainWindow::new(&layout);
        if let Some(message) = problem {
            main_window.show_status(message);
        }
        Self {
            i18n,
            ledger,
            data_file: None,
            preferences_file,
            preferences,
            theme,
            clock,
            commands,
            command_receiver,
            main_window,
        }
    }

    /// Builds the look from `preferences`, unless it is already built from them.
    fn show_preferences(&mut self, ctx: &egui::Context, preferences: &Preferences) {
        let settings = ThemeSettings::from(preferences);
        if settings == self.theme.settings() {
            return;
        }
        self.theme = Theme::new(settings);
        apply_theme(ctx, &self.theme);
    }

    /// Puts `preferences` in use and writes them to the preferences file.
    fn set_preferences(&mut self, preferences: Preferences) {
        if let Err(error) = self.preferences_file.save(&preferences) {
            tracing::warn!(?error, "saving the Preferences failed");
            self.main_window
                .show_status(StatusMessage::PreferencesNotSaved);
        }
        self.preferences = preferences;
    }

    /// Whether `path` names the open data file, through whatever links or relative
    /// parts it takes.
    fn is_open_file(&self, path: &Path) -> bool {
        let Some(open) = &self.data_file else {
            return false;
        };
        match (fs::canonicalize(open), fs::canonicalize(path)) {
            (Ok(open), Ok(path)) => open == path,
            // A path that doesn't resolve names no file, so not the open one.
            _ => false,
        }
    }

    /// Puts `ledger`, over the data file at `path`, in place of the open one, which
    /// closes it, and shows the new file from scratch.
    fn switch_data_file(&mut self, ledger: Ledger, path: PathBuf) {
        self.ledger = ledger;
        self.data_file = Some(path);
        self.main_window.data_file_switched();
    }

    /// Creates an empty data file at `path` and switches to it, replacing any other
    /// file there, or shows why it can't.
    fn new_data_file(&mut self, ctx: &egui::Context, path: PathBuf) {
        if self.is_open_file(&path) {
            self.main_window
                .refuse_data_file(ctx, DataFileRefusal::open_file_not_replaced(&path));
            return;
        }
        // The native picker has asked before replacing a file already at `path`.
        match fs::remove_file(&path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                tracing::warn!(
                    ?error,
                    ?path,
                    "removing the file a new data file replaces failed"
                );
                self.main_window
                    .refuse_data_file(ctx, DataFileRefusal::not_created(&path));
                return;
            }
            Ok(()) | Err(_) => {}
        }
        match Ledger::create(&path, usd()) {
            Ok(ledger) => self.switch_data_file(ledger, path),
            Err(error) => {
                tracing::warn!(?error, ?path, "creating the data file failed");
                self.main_window
                    .refuse_data_file(ctx, DataFileRefusal::not_created(&path));
            }
        }
    }

    /// Opens the data file at `path` and switches to it, or shows why it can't. The
    /// open file, picked again, stays as it is.
    fn open_data_file(&mut self, ctx: &egui::Context, path: PathBuf) {
        if self.is_open_file(&path) {
            return;
        }
        match Ledger::open(&path) {
            Ok(ledger) => self.switch_data_file(ledger, path),
            Err(error) => {
                tracing::info!(?error, ?path, "the data file was refused");
                self.main_window
                    .refuse_data_file(ctx, DataFileRefusal::open(&path, &error));
            }
        }
    }

    fn apply(&mut self, ctx: &egui::Context, command: AppCommand) {
        match command {
            AppCommand::Domain(command) => {
                if let Err(error) = self.ledger.apply(command) {
                    self.main_window.show_status(StatusMessage::Rejected(error));
                }
            }
            AppCommand::SubmitForm(command) => {
                let result = self.ledger.apply(command).map(drop);
                self.main_window.form_applied(ctx, result);
            }
            AppCommand::Undo => {
                if let Err(error) = self.ledger.undo() {
                    self.main_window.show_status(StatusMessage::Rejected(error));
                }
            }
            AppCommand::Redo => {
                if let Err(error) = self.ledger.redo() {
                    self.main_window.show_status(StatusMessage::Rejected(error));
                }
            }
            AppCommand::Navigate(route) => self.main_window.navigate(route),
            AppCommand::GoToDraftRow => self
                .main_window
                .go_to_draft_row(ctx, &self.ledger.snapshot()),
            AppCommand::OpenSplitView(_) => {
                self.main_window.show_status(StatusMessage::NotAvailableYet);
            }
            AppCommand::SwitchPane => self.main_window.switch_pane(ctx),
            AppCommand::OpenDialog(kind) => {
                self.main_window.open_dialog(
                    ctx,
                    &self.i18n,
                    &kind,
                    &self.ledger.snapshot(),
                    self.clock.today(),
                    &self.preferences,
                );
            }
            AppCommand::SubmitDialog(command) => {
                let applied = self.ledger.apply(command).map(drop);
                self.main_window.dialog_applied(ctx, applied);
            }
            AppCommand::SetPreferences(preferences) => self.set_preferences(preferences),
            AppCommand::SetRegisterColumns(register_columns) => {
                self.set_preferences(Preferences {
                    register_columns,
                    ..self.preferences.clone()
                });
            }
            AppCommand::ChooseDataFile(picker) => picker.show(&self.i18n, &self.commands),
            AppCommand::NewDataFile(path) => self.new_data_file(ctx, path),
            AppCommand::OpenDataFile(path) => self.open_data_file(ctx, path),
            AppCommand::ShowStatus(message) => self.main_window.show_status(message),
            AppCommand::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }
}

#[cfg(feature = "test-support")]
impl App {
    /// Sends `command` to `App::logic`, as a widget or a worker would.
    pub(crate) fn send(&self, command: AppCommand) {
        self.commands.send(command);
    }
}

impl eframe::App for App {
    /// Applies every Command sent since the last frame, then shows the open
    /// Preferences Dialog's draft, or else the Preferences in use.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(command) = self.command_receiver.try_recv() {
            self.apply(ctx, command);
        }
        let preferences = self
            .main_window
            .preferences_draft()
            .unwrap_or(&self.preferences)
            .clone();
        self.show_preferences(ctx, &preferences);
    }

    /// Takes the arrow keys away from egui's focus navigation while a menu is open, and
    /// Tab and Shift+Tab while the Register owns the keyboard, and hands them to the
    /// Menu bar and the Register.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.main_window.take_menu_arrows(ctx, raw_input);
        if self.main_window.register_takes_tab(ctx) {
            raw_input.events.retain(|event| {
                let egui::Event::Key {
                    key: egui::Key::Tab,
                    pressed,
                    modifiers,
                    ..
                } = event
                else {
                    return true;
                };
                // Tab with Ctrl, Alt or Command held stays with the rest of the app.
                if !(modifiers.is_none() || modifiers.shift_only()) {
                    return true;
                }
                if *pressed {
                    self.main_window.press_tab(*modifiers);
                }
                false
            });
        }
    }

    /// Saves the Main window's layout. eframe saves the window's size and position
    /// itself.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.main_window.layout().save(storage);
    }

    /// egui's memory (scroll offsets, focus, panel sizes) stays out of storage: the
    /// [`Layout`] holds everything that outlives the run.
    fn persist_egui_memory(&self) -> bool {
        false
    }

    /// Draws the Main window with the open Preferences Dialog's draft, or else the
    /// Preferences in use.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let view = self.ledger.snapshot();
        let today = self.clock.today();
        let preferences = self
            .main_window
            .preferences_draft()
            .unwrap_or(&self.preferences)
            .clone();
        let frame = WindowFrame {
            i18n: &self.i18n,
            theme: &self.theme,
            preferences: &preferences,
            commands: &self.commands,
            view: &view,
            today,
        };
        self.main_window.show(ui, &frame);
    }
}

/// The currency of a new data file. Balanced shows amounts in US dollars only, so far.
fn usd() -> CurrencyCode {
    CurrencyCode::new("USD").expect("USD is an ISO 4217 currency")
}

/// Gives every window `theme`'s style and fonts.
fn apply_theme(ctx: &egui::Context, theme: &Theme) {
    // Balanced's Scheme, not the OS, picks light or dark, so egui's light and dark
    // styles are both the theme's style.
    let style = Arc::new(theme.style());
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_fonts(theme.fonts());
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use balanced_core::AccountGroup;
    use eframe::App as _;
    use eframe::egui::accesskit::Role;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    use icu_calendar::Date;

    use super::*;
    use crate::accounts_model::Section;
    use crate::layout::MemoryStorage;

    #[test]
    fn folding_a_group_saves_it_in_the_layout() {
        let mut harness = Harness::new_eframe(|cc| {
            App::new(
                cc,
                Localizer::for_tests(),
                Ledger::stub(),
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
                PreferencesFile::for_tests(),
            )
        });
        let mut storage = MemoryStorage::default();
        harness.run();

        harness
            .get_by_role_and_label(Role::Button, "Banking")
            .click();
        harness.run();
        harness.state_mut().save(&mut storage);

        let layout = Layout::load(Some(&storage));
        assert_eq!(
            layout.folded_sections(),
            HashSet::from([Section::Group(AccountGroup::Banking), Section::Closed])
        );
    }
}
