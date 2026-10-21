// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`App`]: the eframe application, which owns the open data file's ledger, the Main
//! window and the welcome screen.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use balanced_core::{CurrencyCode, DomainCommand, Ledger, OpenError};
use balanced_i18n::Localizer;
use eframe::egui;

use crate::clock::Clock;
use crate::command::{AppCommand, CommandSender, StatusMessage};
use crate::data_file::DataFileRefusal;
use crate::i18n::fl;
use crate::layout::Layout;
use crate::main_window::{MainWindow, WindowFrame};
use crate::preferences::{Preferences, PreferencesFile};
use crate::recent_files::{RecentFiles, RecentFilesFile};
use crate::route::Route;
use crate::theme::{Theme, ThemeSettings};
use crate::welcome::Welcome;

/// Balanced's eframe application.
#[derive(Debug)]
pub struct App {
    i18n: Localizer,
    session: Session,
    recent_files_file: RecentFilesFile,
    recent_files: RecentFiles,
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

/// What the App shows: a data file, or the welcome screen asking for one.
#[derive(Debug)]
enum Session {
    Open {
        ledger: Ledger,
        /// Where the data file is, as the Recent files list it, or `None` while tests
        /// run the App on data with no file.
        path: Option<PathBuf>,
    },
    Welcome(Welcome),
}

impl App {
    /// The App on the data file opened last, as `recent_files_file` lists it, showing
    /// text through `i18n`, reading today's date from `clock` and keeping the
    /// Preferences in `preferences_file`. With no Recent files, or when the newest is
    /// refused, it starts on the welcome screen, after the refusal.
    ///
    /// A preferences file that can't be read is renamed to `preferences.toml.broken`,
    /// and the App starts from the default Preferences and says so in the Status bar.
    /// A recent files file that can't be read counts as no Recent files.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        i18n: Localizer,
        clock: Clock,
        preferences_file: PreferencesFile,
        recent_files_file: RecentFilesFile,
    ) -> Self {
        let recent_files = recent_files_file.load().unwrap_or_else(|error| {
            tracing::warn!(
                ?error,
                "the recent files file can't be read; starting with none"
            );
            RecentFiles::default()
        });
        let ctx = &cc.egui_ctx;
        let session = match recent_files.newest() {
            None => Session::Welcome(Welcome::new(ctx)),
            Some(path) => match Ledger::open(path) {
                Ok(ledger) => Session::Open {
                    ledger,
                    path: Some(path.to_owned()),
                },
                Err(error) => {
                    tracing::info!(?error, ?path, "the data file opened last was refused");
                    let mut welcome = Welcome::new(ctx);
                    welcome.refuse(ctx, DataFileRefusal::open(path, &error));
                    Session::Welcome(welcome)
                }
            },
        };
        let mut app = Self::start(
            cc,
            i18n,
            session,
            clock,
            preferences_file,
            recent_files_file,
            recent_files,
        );
        app.reopen_last_view();
        app
    }

    /// The App started on `session`, with the rest as [`App::new`] takes it.
    fn start(
        cc: &eframe::CreationContext<'_>,
        i18n: Localizer,
        session: Session,
        clock: Clock,
        preferences_file: PreferencesFile,
        recent_files_file: RecentFilesFile,
        recent_files: RecentFiles,
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
            session,
            recent_files_file,
            recent_files,
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
        let Session::Open {
            path: Some(open), ..
        } = &self.session
        else {
            return false;
        };
        match (fs::canonicalize(open), fs::canonicalize(path)) {
            (Ok(open), Ok(path)) => open == path,
            // A path that doesn't resolve names no file, so not the open one.
            _ => false,
        }
    }

    /// Puts `ledger`, over the data file at `path`, in place of the open one, which
    /// closes it, or of the welcome screen. The closed file's Recent files entry keeps
    /// the view it closed on, and the new file goes to the top of the Recent files and
    /// opens on the view it last closed on, or else from scratch on the tab shown.
    fn switch_data_file(&mut self, ledger: Ledger, path: PathBuf) {
        self.save_editing_row();
        self.note_closed();
        let listed = listed(path);
        self.recent_files.note_opened(listed.clone());
        self.save_recent_files();
        self.session = Session::Open {
            ledger,
            path: Some(listed),
        };
        self.main_window.data_file_switched();
        self.reopen_last_view();
    }

    /// Saves the changes to the Register's editing row into the open ledger at once,
    /// with no confirm, as the Register goes: for another account's, another data
    /// file's, or the window closing. A refused save shows in the Status bar.
    // TODO(#423): ask Save, Continue editing or Discard first.
    fn save_editing_row(&mut self) {
        let (commands, saves) = self.commands.with_own_channel();
        self.main_window
            .save_editing_row(&self.i18n, &self.preferences, &commands);
        for command in saves.try_iter() {
            match command {
                AppCommand::Domain(command) => self.apply_domain(command),
                AppCommand::ShowStatus(message) => self.main_window.show_status(message),
                // The confirm a Reconciled amount change asks for can't open over a
                // Register that is going, so that row's changes are not saved.
                AppCommand::OpenDialog(kind) => {
                    tracing::error!(
                        ?kind,
                        "the editing row's save needs a confirm, so its changes were lost"
                    );
                    self.main_window
                        .show_status(StatusMessage::ReconciledAmountNotSaved);
                }
                // Saving the editing row sends none of these.
                AppCommand::SubmitForm(_)
                | AppCommand::Undo
                | AppCommand::Redo
                | AppCommand::Navigate(_)
                | AppCommand::OpenSplitView(_)
                | AppCommand::GoToTransaction { .. }
                | AppCommand::GoToDraftRow
                | AppCommand::GoToDate(_)
                | AppCommand::SwitchPane
                | AppCommand::OpenCalculator
                | AppCommand::SubmitDialog(_)
                | AppCommand::SetPreferences(_)
                | AppCommand::SetRegisterColumns(_)
                | AppCommand::ChooseDataFile(_)
                | AppCommand::NewDataFile(_)
                | AppCommand::OpenDataFile(_)
                | AppCommand::Quit => {}
            }
        }
    }

    /// Applies `command` to the open ledger. A refused change shows in the Status bar.
    fn apply_domain(&mut self, command: DomainCommand) {
        let Session::Open { ledger, .. } = &mut self.session else {
            return;
        };
        if let Err(error) = ledger.apply(command) {
            let message = StatusMessage::refusal(error, &ledger.snapshot());
            self.main_window.show_status(message);
        }
    }

    /// Saves the view the Main window shows as the one the open data file closed on.
    fn note_closed(&mut self) {
        if let Session::Open {
            path: Some(path), ..
        } = &self.session
        {
            self.recent_files
                .note_closed(path, self.main_window.route());
        }
    }

    /// Shows the view the open data file last closed on, if the Recent files saved one.
    fn reopen_last_view(&mut self) {
        let Session::Open {
            ledger,
            path: Some(path),
        } = &self.session
        else {
            return;
        };
        if let Some(route) = self.recent_files.last_view(path) {
            self.main_window.reopen(route, &ledger.snapshot());
        }
    }

    /// Writes the Recent files to their file.
    fn save_recent_files(&self) {
        if let Err(error) = self.recent_files_file.save(&self.recent_files) {
            tracing::warn!(?error, "saving the Recent files failed");
        }
    }

    /// Shows why a data file was refused, over the Main window, or before the welcome
    /// Dialog when no file is open.
    fn refuse_data_file(&mut self, ctx: &egui::Context, refusal: DataFileRefusal) {
        match &mut self.session {
            Session::Open { .. } => self.main_window.refuse_data_file(ctx, refusal),
            Session::Welcome(welcome) => welcome.refuse(ctx, refusal),
        }
    }

    /// Creates an empty data file at `path` and switches to it, replacing any other
    /// file there, or shows why it can't.
    fn new_data_file(&mut self, ctx: &egui::Context, path: PathBuf) {
        if self.is_open_file(&path) {
            self.refuse_data_file(ctx, DataFileRefusal::open_file_not_replaced(&path));
            return;
        }
        match create_replacing(&path) {
            Ok(ledger) => {
                self.recent_files.note_replaced(&listed(path.clone()));
                self.switch_data_file(ledger, path);
            }
            Err(error) => {
                tracing::warn!(?error, ?path, "creating the data file failed");
                self.refuse_data_file(ctx, DataFileRefusal::not_created(&path));
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
                self.refuse_data_file(ctx, DataFileRefusal::open(&path, &error));
            }
        }
    }

    /// Applies `command`. A Command that acts on the open data file does nothing on the
    /// welcome screen, which sends none.
    fn apply(&mut self, ctx: &egui::Context, command: AppCommand) {
        let ledger = match &mut self.session {
            Session::Open { ledger, .. } => Some(ledger),
            Session::Welcome(_) => None,
        };
        match command {
            AppCommand::Domain(command) => self.apply_domain(command),
            AppCommand::SubmitForm(command) => {
                if let Some(ledger) = ledger {
                    let result = ledger.apply(command).map(drop);
                    self.main_window.form_applied(ctx, result);
                }
            }
            AppCommand::Undo => {
                if let Some(ledger) = ledger
                    && let Err(error) = ledger.undo()
                {
                    let message = StatusMessage::refusal(error, &ledger.snapshot());
                    self.main_window.show_status(message);
                }
            }
            AppCommand::Redo => {
                if let Some(ledger) = ledger
                    && let Err(error) = ledger.redo()
                {
                    let message = StatusMessage::refusal(error, &ledger.snapshot());
                    self.main_window.show_status(message);
                }
            }
            AppCommand::Navigate(route) => {
                if self.main_window.replaces_register(route) {
                    self.save_editing_row();
                }
                self.main_window.navigate(route);
            }
            AppCommand::GoToTransaction {
                account,
                transaction,
            } => {
                if self.main_window.replaces_register(Route::Account(account)) {
                    self.save_editing_row();
                }
                self.main_window.go_to_transaction(account, transaction);
            }
            AppCommand::GoToDate(date) => self.main_window.go_to_date(date),
            AppCommand::GoToDraftRow => {
                if let Some(ledger) = ledger {
                    self.main_window.go_to_draft_row(ctx, &ledger.snapshot());
                }
            }
            AppCommand::OpenSplitView(_) => {
                self.main_window.show_status(StatusMessage::NotAvailableYet);
            }
            AppCommand::SwitchPane => self.main_window.switch_pane(ctx),
            AppCommand::OpenCalculator => {
                // The Calculator works in the ledger's currency, as New Account does.
                if let Some(ledger) = ledger {
                    let currency = ledger.snapshot().net_worth().currency();
                    self.main_window.open_calculator(&self.i18n, currency);
                }
            }
            AppCommand::OpenDialog(kind) => {
                if let Some(ledger) = ledger {
                    self.main_window.open_dialog(
                        ctx,
                        &self.i18n,
                        &kind,
                        &ledger.snapshot(),
                        self.clock.today(),
                        &self.preferences,
                    );
                }
            }
            AppCommand::SubmitDialog(command) => {
                if let Some(ledger) = ledger {
                    let applied = ledger.apply(command).map(drop);
                    self.main_window.dialog_applied(ctx, applied);
                }
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
    /// The App over `ledger`, a ledger with no data file such as `Ledger::stub()`, with
    /// no Recent files, and the rest as [`App::new`] takes it.
    pub fn on_ledger(
        cc: &eframe::CreationContext<'_>,
        i18n: Localizer,
        ledger: Ledger,
        clock: Clock,
        preferences_file: PreferencesFile,
    ) -> Self {
        Self::start(
            cc,
            i18n,
            Session::Open { ledger, path: None },
            clock,
            preferences_file,
            RecentFilesFile::for_tests(),
            RecentFiles::default(),
        )
    }

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

    /// Saves the changes to the Register's editing row, with no confirm, and the view
    /// the Main window shows as the one the open data file closed on.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_editing_row();
        self.note_closed();
        self.save_recent_files();
    }

    /// egui's memory (scroll offsets, focus, panel sizes) stays out of storage: the
    /// [`Layout`] holds everything that outlives the run.
    fn persist_egui_memory(&self) -> bool {
        false
    }

    /// Draws the Main window with the open Preferences Dialog's draft, or else the
    /// Preferences in use.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ledger = match &mut self.session {
            Session::Open { ledger, .. } => ledger,
            Session::Welcome(welcome) => {
                welcome.show(ui, &self.i18n, &self.theme, &self.commands);
                return;
            }
        };
        let view = ledger.snapshot();
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
            recent_files: &self.recent_files,
        };
        self.main_window.show(ui, &frame);
    }
}

/// `path` as the Recent files list it: resolved through any links or relative parts.
/// A data file just opened resolves, unless it has just been moved.
fn listed(path: PathBuf) -> PathBuf {
    fs::canonicalize(&path).unwrap_or(path)
}

/// Creates an empty data file at `path`, replacing any file there, since the native
/// picker has asked before replacing it. The new file is made beside it and then moved
/// into place, so a failure leaves the old file as it was.
fn create_replacing(path: &Path) -> Result<Ledger, CreateError> {
    let staging = beside(path, ".new");
    remove_if_present(&staging)?;
    // The ledger holds the file it creates locked, so it closes before the move.
    drop(Ledger::create(&staging, usd())?);
    if let Err(error) = fs::rename(&staging, path) {
        // Nothing uses the staging file, and the next New Data File… there replaces
        // it, so failing to remove it loses nothing.
        if let Err(error) = fs::remove_file(&staging) {
            tracing::warn!(?error, ?staging, "removing an unused new data file failed");
        }
        return Err(error.into());
    }
    // A journal left beside the replaced file would roll its changes into the new one.
    for suffix in ["-journal", "-wal", "-shm"] {
        remove_if_present(&beside(path, suffix))?;
    }
    Ok(Ledger::open(path)?)
}

/// The path of the file beside `path` whose name is `path`'s with `suffix` added.
fn beside(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Removes the file at `path`, if there is one.
fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        Ok(()) | Err(_) => Ok(()),
    }
}

/// Why a new data file couldn't be created.
#[derive(Debug, thiserror::Error)]
enum CreateError {
    #[error("moving or removing a file beside the new data file failed")]
    Io(#[from] io::Error),
    #[error("the new data file couldn't be created or opened")]
    Ledger(#[from] OpenError),
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
            App::on_ledger(
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
