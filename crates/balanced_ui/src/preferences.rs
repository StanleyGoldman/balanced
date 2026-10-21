// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`Preferences`]: the choices that apply to every data file on this computer, and
//! [`PreferencesFile`], which keeps them in `preferences.toml`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::register_columns::RegisterColumns;

/// Light or dark colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Scheme {
    #[default]
    Light,
    Dark,
}

/// The face body text is drawn in. egui's own fonts stay behind it, for the glyphs it
/// lacks. The file holds its id; an id this build doesn't know reads as egui's face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "String", into = "&'static str")]
pub(crate) enum FontFace {
    /// egui's default proportional face.
    #[default]
    Egui,
    NotoSans,
    LiberationSans,
}

impl FontFace {
    pub(crate) const ALL: [Self; 3] = [Self::Egui, Self::NotoSans, Self::LiberationSans];

    /// The face's id in the preferences file.
    fn id(self) -> &'static str {
        match self {
            Self::Egui => "egui",
            Self::NotoSans => "noto_sans",
            Self::LiberationSans => "liberation_sans",
        }
    }
}

impl From<String> for FontFace {
    fn from(id: String) -> Self {
        Self::ALL
            .into_iter()
            .find(|face| face.id() == id)
            .unwrap_or_default()
    }
}

impl From<FontFace> for &'static str {
    fn from(face: FontFace) -> Self {
        face.id()
    }
}

/// Which rules the Register draws between its cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Gridlines {
    #[default]
    Off,
    /// A rule between rows.
    Horizontal,
    /// A rule between columns.
    Vertical,
    Both,
}

impl Gridlines {
    pub(crate) const ALL: [Self; 4] = [Self::Off, Self::Horizontal, Self::Vertical, Self::Both];
}

/// Where Net Worth is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NetWorthPlacement {
    /// In the Account bar's footer.
    #[default]
    AccountBar,
    StatusBar,
}

/// The user's Preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool is one on/off Preference, a checkbox in the Dialog and a TOML boolean in the file"
)]
pub(crate) struct Preferences {
    pub(crate) scheme: Scheme,
    pub(crate) font_face: FontFace,
    /// Rows 16 px high instead of 20 px.
    pub(crate) dense_rows: bool,
    /// Amounts in the monospace face, so their digits line up.
    pub(crate) monospace_money: bool,
    /// Negative amounts in the negative colour.
    pub(crate) coloured_negatives: bool,
    /// Every other Register row filled with the stripe colour.
    pub(crate) alternating_rows: bool,
    /// A line in the Register between today's rows and the rows dated later.
    pub(crate) today_divider: bool,
    pub(crate) gridlines: Gridlines,
    /// Committing a new transaction's payee fills its empty cells from the payee's
    /// latest transaction (Payee recall).
    pub(crate) payee_recall: bool,
    /// Typing in a Payee or Category cell completes the text with the rest of the
    /// first entry that starts with it (Inline completion).
    pub(crate) inline_completion: bool,
    /// Enter, saving a Register row, opens the next row's cell in the same column,
    /// rather than leaving the saved row highlighted.
    pub(crate) enter_moves_to_next_row: bool,
    /// Each Account bar heading shows its section's total.
    pub(crate) group_totals: bool,
    pub(crate) net_worth_placement: NetWorthPlacement,
    /// The columns every Register shows, and their order.
    #[serde(flatten)]
    pub(crate) register_columns: RegisterColumns,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            scheme: Scheme::default(),
            font_face: FontFace::default(),
            dense_rows: false,
            monospace_money: false,
            coloured_negatives: true,
            alternating_rows: true,
            today_divider: true,
            gridlines: Gridlines::default(),
            payee_recall: true,
            inline_completion: true,
            enter_moves_to_next_row: true,
            group_totals: false,
            net_worth_placement: NetWorthPlacement::default(),
            register_columns: RegisterColumns::default(),
        }
    }
}

/// The version of the file's format this build writes. A rename, or a change in what a
/// key means, bumps it and adds a step to [`migrate`]. Adding or removing a key does
/// not: a missing key takes its default, and an unknown key is ignored.
const VERSION: i64 = 2;

/// The key holding the file's format version.
const VERSION_KEY: &str = "version";

/// The file's contents: its format version, then the Preferences.
#[derive(Serialize)]
struct Contents<'a> {
    version: i64,
    #[serde(flatten)]
    preferences: &'a Preferences,
}

/// Brings `table`, read from a file in format `version`, up to [`VERSION`].
fn migrate(mut table: toml::Table, version: i64) -> Result<toml::Table, LoadError> {
    match version {
        1 => {
            split_amount_column(&mut table);
            migrate(table, 2)
        }
        VERSION => Ok(table),
        _ => Err(LoadError::UnknownVersion(version)),
    }
}

/// Version 1 to 2: the Amount column became Payment and Deposit, which take its place
/// in the saved column order. Amount couldn't be hidden, so the hidden list has none.
fn split_amount_column(table: &mut toml::Table) {
    let Some(toml::Value::Array(order)) = table.get_mut("register_columns") else {
        return;
    };
    let ids = std::mem::take(order);
    for id in ids {
        if id.as_str() == Some("amount") {
            order.extend(["payment", "deposit"].map(toml::Value::from));
        } else {
            order.push(id);
        }
    }
}

/// The `preferences.toml` file in a config directory.
#[derive(Debug)]
pub struct PreferencesFile {
    path: PathBuf,
    /// The temporary directory [`PreferencesFile::for_tests`] made, removed with it.
    #[cfg(any(test, feature = "test-support"))]
    _temporary_dir: Option<tempfile::TempDir>,
}

impl PreferencesFile {
    /// The preferences file in `config_dir`, which need not exist yet.
    pub fn in_dir(config_dir: &Path) -> Self {
        Self {
            path: config_dir.join("preferences.toml"),
            #[cfg(any(test, feature = "test-support"))]
            _temporary_dir: None,
        }
    }

    /// A preferences file in a new, empty temporary directory, removed when the file
    /// is dropped.
    ///
    /// # Panics
    ///
    /// If the temporary directory can't be made.
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_tests() -> Self {
        let dir = tempfile::tempdir().expect("the test machine can make a temporary directory");
        let file = Self::in_dir(dir.path());
        Self {
            _temporary_dir: Some(dir),
            ..file
        }
    }

    /// The Preferences in the file, or the defaults when there is no file.
    pub(crate) fn load(&self) -> Result<Preferences, LoadError> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Preferences::default());
            }
            Err(error) => return Err(LoadError::Read(error)),
        };
        let table: toml::Table = toml::from_str(&text)?;
        let version = match table.get(VERSION_KEY) {
            // Version 1 was the first, so a file without a version is in it.
            None => 1,
            Some(toml::Value::Integer(version)) => *version,
            Some(_) => return Err(LoadError::VersionNotANumber),
        };
        Ok(migrate(table, version)?.try_into()?)
    }

    /// Renames the file to `preferences.toml.broken`, replacing any file of that name,
    /// so a file that can't be read is kept rather than overwritten by the next save.
    pub(crate) fn set_aside(&self) -> std::io::Result<()> {
        std::fs::rename(&self.path, self.path.with_extension("toml.broken"))
    }

    /// Writes `preferences` to the file, creating its directory if need be. The file is
    /// written whole beside the old one and then renamed over it, so a failed write
    /// leaves the old file as it was.
    pub(crate) fn save(&self, preferences: &Preferences) -> Result<(), SaveError> {
        let text = toml::to_string(&Contents {
            version: VERSION,
            preferences,
        })?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temporary = self.path.with_extension("toml.tmp");
        std::fs::write(&temporary, text)?;
        std::fs::rename(&temporary, &self.path)?;
        Ok(())
    }
}

/// Why the preferences file couldn't be read.
#[derive(Debug, thiserror::Error)]
pub(crate) enum LoadError {
    #[error("reading the preferences file failed")]
    Read(#[source] std::io::Error),
    #[error("the preferences file is not valid")]
    Invalid(#[from] toml::de::Error),
    #[error("the preferences file's version is not a number")]
    VersionNotANumber,
    #[error("the preferences file has a version this build can't read: {0}")]
    UnknownVersion(i64),
}

/// Why the preferences file couldn't be written.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SaveError {
    #[error("the preferences can't be written as TOML")]
    Serialize(#[from] toml::ser::Error),
    #[error("writing the preferences file failed")]
    Write(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::register_columns::RegisterColumn;

    /// A preferences file in a new, empty directory, which lasts as long as the
    /// returned `TempDir`.
    fn file() -> (TempDir, PreferencesFile) {
        let dir = tempfile::tempdir().unwrap();
        let file = PreferencesFile::in_dir(dir.path());
        (dir, file)
    }

    #[test]
    fn saved_preferences_load_back() {
        let (_dir, file) = file();
        let mut register_columns = RegisterColumns::default();
        register_columns.move_up(RegisterColumn::Payee);
        register_columns.set_shown(RegisterColumn::Memo, false);
        let preferences = Preferences {
            scheme: Scheme::Dark,
            font_face: FontFace::NotoSans,
            dense_rows: true,
            monospace_money: true,
            coloured_negatives: false,
            alternating_rows: false,
            today_divider: false,
            gridlines: Gridlines::Both,
            payee_recall: false,
            inline_completion: false,
            enter_moves_to_next_row: false,
            group_totals: true,
            net_worth_placement: NetWorthPlacement::StatusBar,
            register_columns,
        };

        file.save(&preferences).unwrap();
        let loaded = file.load().unwrap();

        assert_eq!(loaded, preferences);
    }

    #[test]
    fn a_missing_key_takes_its_default() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("preferences.toml"),
            "version = 1\nscheme = \"dark\"\n",
        )
        .unwrap();

        let loaded = file.load().unwrap();

        let expected = Preferences {
            scheme: Scheme::Dark,
            ..Preferences::default()
        };
        assert_eq!(loaded, expected);
    }

    #[test]
    fn an_unknown_key_is_ignored() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("preferences.toml"),
            "version = 1\ndense_rows = true\nretired_preference = 3\n",
        )
        .unwrap();

        let loaded = file.load().unwrap();

        let expected = Preferences {
            dense_rows: true,
            ..Preferences::default()
        };
        assert_eq!(loaded, expected);
    }

    #[test]
    fn a_file_holding_retired_keys_keeps_every_other_preference() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("preferences.toml"),
            "version = 1\n\
             scheme = \"dark\"\n\
             font_face = \"noto_sans\"\n\
             dense_rows = true\n\
             monospace_money = true\n\
             coloured_negatives = false\n\
             alternating_rows = false\n\
             gridlines = \"both\"\n\
             group_totals = true\n\
             net_worth_placement = \"status_bar\"\n\
             auto_fold = false\n\
             open_account_in = \"new_tab\"\n\
             register_columns = [\"payee\", \"date\", \"num\", \"category\", \"memo\", \"amount\", \"balance\"]\n\
             hidden_register_columns = [\"memo\"]\n",
        )
        .unwrap();

        let loaded = file.load().unwrap();

        let mut register_columns = RegisterColumns::default();
        register_columns.move_up(RegisterColumn::Payee);
        register_columns.move_up(RegisterColumn::Payee);
        register_columns.set_shown(RegisterColumn::Memo, false);
        let expected = Preferences {
            scheme: Scheme::Dark,
            font_face: FontFace::NotoSans,
            dense_rows: true,
            monospace_money: true,
            coloured_negatives: false,
            alternating_rows: false,
            today_divider: true,
            gridlines: Gridlines::Both,
            payee_recall: true,
            inline_completion: true,
            enter_moves_to_next_row: true,
            group_totals: true,
            net_worth_placement: NetWorthPlacement::StatusBar,
            register_columns,
        };
        assert_eq!(loaded, expected);
    }

    #[test]
    fn a_version_1_file_puts_payment_and_deposit_where_amount_was() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("preferences.toml"),
            "version = 1\n\
             register_columns = [\"amount\", \"date\", \"num\", \"payee\", \"category\", \"memo\", \"balance\"]\n",
        )
        .unwrap();

        let loaded = file.load().unwrap();

        assert_eq!(
            loaded.register_columns.shown(),
            [
                RegisterColumn::Payment,
                RegisterColumn::Deposit,
                RegisterColumn::Date,
                RegisterColumn::Number,
                RegisterColumn::Payee,
                RegisterColumn::Category,
                RegisterColumn::Memo,
                RegisterColumn::Balance,
                RegisterColumn::Clr,
                RegisterColumn::Flag,
                RegisterColumn::Actions,
            ]
        );
    }

    #[test]
    fn the_enumerated_preferences_read_their_ids() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("preferences.toml"),
            "version = 1\n\
             gridlines = \"vertical\"\n\
             net_worth_placement = \"status_bar\"\n",
        )
        .unwrap();

        let loaded = file.load().unwrap();

        let expected = Preferences {
            gridlines: Gridlines::Vertical,
            net_worth_placement: NetWorthPlacement::StatusBar,
            ..Preferences::default()
        };
        assert_eq!(loaded, expected);
    }

    #[test]
    fn an_unknown_font_face_reads_as_eguis_face() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("preferences.toml"),
            "version = 1\nfont_face = \"comic_sans\"\n",
        )
        .unwrap();

        let loaded = file.load().unwrap();

        assert_eq!(loaded.font_face, FontFace::Egui);
    }

    #[test]
    fn unknown_column_ids_are_dropped_and_missing_columns_appended() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("preferences.toml"),
            "version = 1\n\
             register_columns = [\"payee\", \"tags\", \"actions\", \"date\", \"payee\"]\n\
             hidden_register_columns = [\"memo\", \"tags\", \"amount\"]\n",
        )
        .unwrap();

        let loaded = file.load().unwrap();

        assert_eq!(
            loaded.register_columns.shown(),
            [
                RegisterColumn::Payee,
                RegisterColumn::Date,
                RegisterColumn::Number,
                RegisterColumn::Category,
                RegisterColumn::Payment,
                RegisterColumn::Deposit,
                RegisterColumn::Balance,
                RegisterColumn::Clr,
                RegisterColumn::Flag,
                RegisterColumn::Actions,
            ]
        );
    }

    #[test]
    fn a_new_file_shows_clr_and_flag_between_balance_and_actions() {
        let (_dir, file) = file();

        let loaded = file.load().unwrap();

        assert_eq!(
            loaded.register_columns.shown(),
            [
                RegisterColumn::Date,
                RegisterColumn::Number,
                RegisterColumn::Payee,
                RegisterColumn::Category,
                RegisterColumn::Memo,
                RegisterColumn::Payment,
                RegisterColumn::Deposit,
                RegisterColumn::Balance,
                RegisterColumn::Clr,
                RegisterColumn::Flag,
                RegisterColumn::Actions,
            ]
        );
    }

    #[test]
    fn a_file_saved_before_the_flag_column_shows_it_after_the_others() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("preferences.toml"),
            "version = 2\n\
             register_columns = [\"payee\", \"date\", \"num\", \"category\", \"memo\", \"payment\", \"deposit\", \"balance\"]\n\
             hidden_register_columns = [\"memo\"]\n",
        )
        .unwrap();

        let loaded = file.load().unwrap();

        assert_eq!(
            loaded.register_columns.shown(),
            [
                RegisterColumn::Payee,
                RegisterColumn::Date,
                RegisterColumn::Number,
                RegisterColumn::Category,
                RegisterColumn::Payment,
                RegisterColumn::Deposit,
                RegisterColumn::Balance,
                RegisterColumn::Clr,
                RegisterColumn::Flag,
                RegisterColumn::Actions,
            ]
        );
    }

    #[test]
    fn the_today_divider_is_on_in_a_fresh_file() {
        let (_dir, file) = file();

        let loaded = file.load().unwrap();

        assert!(loaded.today_divider);
    }

    #[test]
    fn payee_recall_is_on_in_a_file_from_before_it() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("preferences.toml"),
            "version = 2\ndense_rows = true\n",
        )
        .unwrap();

        let loaded = file.load().unwrap();

        assert!(loaded.payee_recall);
    }

    #[test]
    fn inline_completion_is_on_in_a_file_from_before_it() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("preferences.toml"),
            "version = 2\ndense_rows = true\n",
        )
        .unwrap();

        let loaded = file.load().unwrap();

        assert!(loaded.inline_completion);
    }

    #[test]
    fn enter_moves_to_the_next_row_in_a_file_from_before_it() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("preferences.toml"),
            "version = 2\ndense_rows = true\n",
        )
        .unwrap();

        let loaded = file.load().unwrap();

        assert!(loaded.enter_moves_to_next_row);
    }

    #[test]
    fn no_file_loads_the_defaults() {
        let (_dir, file) = file();

        let loaded = file.load().unwrap();

        assert_eq!(loaded, Preferences::default());
    }

    #[test]
    fn a_file_without_a_version_reads_as_version_1() {
        let (dir, file) = file();
        std::fs::write(dir.path().join("preferences.toml"), "dense_rows = true\n").unwrap();

        let loaded = file.load().unwrap();

        let expected = Preferences {
            dense_rows: true,
            ..Preferences::default()
        };
        assert_eq!(loaded, expected);
    }

    #[test]
    fn a_version_from_a_later_build_is_an_error() {
        let (dir, file) = file();
        std::fs::write(dir.path().join("preferences.toml"), "version = 3\n").unwrap();

        let loaded = file.load();

        assert!(matches!(loaded, Err(LoadError::UnknownVersion(3))));
    }

    #[test]
    fn the_file_holds_its_version_and_every_preference() {
        let (dir, file) = file();
        let preferences = Preferences {
            font_face: FontFace::LiberationSans,
            ..Preferences::default()
        };

        file.save(&preferences).unwrap();

        let text = std::fs::read_to_string(dir.path().join("preferences.toml")).unwrap();
        assert_eq!(
            text,
            "version = 2\n\
             scheme = \"light\"\n\
             font_face = \"liberation_sans\"\n\
             dense_rows = false\n\
             monospace_money = false\n\
             coloured_negatives = true\n\
             alternating_rows = true\n\
             today_divider = true\n\
             gridlines = \"off\"\n\
             payee_recall = true\n\
             inline_completion = true\n\
             enter_moves_to_next_row = true\n\
             group_totals = false\n\
             net_worth_placement = \"account_bar\"\n\
             register_columns = [\"date\", \"num\", \"payee\", \"category\", \"memo\", \"payment\", \"deposit\", \"balance\", \"clr\", \"flag\"]\n\
             hidden_register_columns = []\n"
        );
    }

    #[test]
    fn a_file_set_aside_is_kept_as_broken() {
        let (dir, file) = file();
        std::fs::write(dir.path().join("preferences.toml"), "not toml [").unwrap();

        file.set_aside().unwrap();

        let broken = std::fs::read_to_string(dir.path().join("preferences.toml.broken"));
        assert_eq!(broken.unwrap(), "not toml [");
        assert!(!dir.path().join("preferences.toml").exists());
    }

    #[test]
    fn saving_creates_the_config_directory_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("balanced");
        let file = PreferencesFile::in_dir(&config_dir);

        file.save(&Preferences::default()).unwrap();

        let mut names: Vec<_> = std::fs::read_dir(&config_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names, ["preferences.toml"]);
    }
}
