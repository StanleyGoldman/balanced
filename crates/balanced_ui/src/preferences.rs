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

/// The user's Preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Preferences {
    pub(crate) scheme: Scheme,
    pub(crate) font_face: FontFace,
    /// Rows 16 px high instead of 20 px.
    pub(crate) dense_rows: bool,
    /// Amounts in the monospace face, so their digits line up.
    pub(crate) monospace_money: bool,
    /// Negative amounts in the negative colour.
    pub(crate) coloured_negatives: bool,
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
            register_columns: RegisterColumns::default(),
        }
    }
}

/// The version of the file's format this build writes. A rename, or a change in what a
/// key means, bumps it and adds a step to [`migrate`]. Adding or removing a key does
/// not: a missing key takes its default, and an unknown key is ignored.
const VERSION: i64 = 1;

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
fn migrate(table: toml::Table, version: i64) -> Result<toml::Table, LoadError> {
    match version {
        VERSION => Ok(table),
        _ => Err(LoadError::UnknownVersion(version)),
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
            None => VERSION,
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
                RegisterColumn::Amount,
                RegisterColumn::Balance,
                RegisterColumn::Actions,
            ]
        );
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
        std::fs::write(dir.path().join("preferences.toml"), "version = 2\n").unwrap();

        let loaded = file.load();

        assert!(matches!(loaded, Err(LoadError::UnknownVersion(2))));
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
            "version = 1\n\
             scheme = \"light\"\n\
             font_face = \"liberation_sans\"\n\
             dense_rows = false\n\
             monospace_money = false\n\
             coloured_negatives = true\n\
             register_columns = [\"date\", \"num\", \"payee\", \"category\", \"memo\", \"amount\", \"balance\"]\n\
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
