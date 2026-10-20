// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`RecentFiles`]: the data files opened lately on this computer, and
//! [`RecentFilesFile`], which keeps them in `recent-files.toml`.

use std::path::{Path, PathBuf};

use balanced_core::AccountId;
use serde::{Deserialize, Serialize};

use crate::route::{Route, TopLevelTab};

/// The data files opened lately, newest first, each with the view it closed on. Recent
/// files are a record, not a Preference.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RecentFiles(Vec<RecentFile>);

/// A Recent file: where it is, and the view it last closed on, if one was saved.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RecentFile {
    path: PathBuf,
    last_view: Option<Route>,
}

impl RecentFiles {
    /// How many files the list keeps.
    const MAX: usize = 4;

    /// Puts `path` at the top, as the file just opened, and drops the oldest file past
    /// [`RecentFiles::MAX`]. A file already listed keeps its saved view.
    pub(crate) fn note_opened(&mut self, path: PathBuf) {
        let last_view = self.last_view(&path);
        self.0.retain(|listed| listed.path != path);
        self.0.insert(0, RecentFile { path, last_view });
        self.0.truncate(Self::MAX);
    }

    /// Saves `route` as the view the listed file at `path` closed on. A file not
    /// listed saves nothing.
    pub(crate) fn note_closed(&mut self, path: &Path, route: Route) {
        self.set_last_view(path, Some(route));
    }

    /// Forgets the view saved for the listed file at `path`, now that a new file has
    /// replaced it.
    pub(crate) fn note_replaced(&mut self, path: &Path) {
        self.set_last_view(path, None);
    }

    /// Sets the view saved for the listed file at `path`. A file not listed saves
    /// nothing.
    fn set_last_view(&mut self, path: &Path, last_view: Option<Route>) {
        if let Some(listed) = self.0.iter_mut().find(|listed| listed.path == path) {
            listed.last_view = last_view;
        }
    }

    /// The view the listed file at `path` last closed on, if one was saved.
    pub(crate) fn last_view(&self, path: &Path) -> Option<Route> {
        self.0
            .iter()
            .find(|listed| listed.path == path)
            .and_then(|listed| listed.last_view)
    }

    /// The file opened last, if any.
    pub(crate) fn newest(&self) -> Option<&Path> {
        self.0.first().map(|listed| listed.path.as_path())
    }

    /// The files, newest first.
    pub(crate) fn paths(&self) -> impl Iterator<Item = &Path> {
        self.0.iter().map(|listed| listed.path.as_path())
    }
}

/// The version of the file's format this build writes. A change in what a key means
/// bumps it and adds a step to [`migrate`].
const VERSION: i64 = 2;

/// The key holding the file's format version.
const VERSION_KEY: &str = "version";

/// The file's contents: its format version, then the files, newest first.
#[derive(Serialize, Deserialize)]
struct Contents {
    version: i64,
    files: Vec<Entry>,
}

/// A Recent file as the file keeps it.
#[derive(Serialize, Deserialize)]
struct Entry {
    path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_view: Option<LastView>,
}

/// The view a file closed on, as the file keeps it: the top-level tab and, on the
/// Accounts tab, the account whose Register was open.
#[derive(Serialize, Deserialize)]
struct LastView {
    tab: TopLevelTab,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    account: Option<u32>,
}

impl From<Route> for LastView {
    fn from(route: Route) -> Self {
        match route {
            Route::Tab(tab) => Self { tab, account: None },
            Route::Account(account) => Self {
                tab: TopLevelTab::Accounts,
                account: Some(account.into()),
            },
        }
    }
}

impl TryFrom<LastView> for Route {
    type Error = LoadError;

    /// An account on any tab but Accounts is an error, since only the Accounts tab
    /// shows a Register.
    fn try_from(view: LastView) -> Result<Self, LoadError> {
        match (view.tab, view.account) {
            (TopLevelTab::Accounts, Some(account)) => Ok(Self::Account(AccountId::from(account))),
            (tab, None) => Ok(Self::Tab(tab)),
            (tab, Some(_)) => Err(LoadError::AccountOffAccountsTab(tab)),
        }
    }
}

/// Brings `table`, read from a file in format `version`, up to [`VERSION`].
fn migrate(table: toml::Table, version: i64) -> Result<toml::Table, LoadError> {
    match version {
        1 => migrate(migrate_v1_to_v2(table), 2),
        VERSION => Ok(table),
        _ => Err(LoadError::UnknownVersion(version)),
    }
}

/// Turns version 1's bare paths into entries with no saved view.
fn migrate_v1_to_v2(mut table: toml::Table) -> toml::Table {
    if let Some(toml::Value::Array(files)) = table.get_mut("files") {
        for file in files.iter_mut() {
            let path = std::mem::replace(file, toml::Value::Boolean(false));
            *file = toml::Value::Table(toml::Table::from_iter([("path".to_owned(), path)]));
        }
    }
    table
}

/// The `recent-files.toml` file in a config directory.
#[derive(Debug)]
pub struct RecentFilesFile {
    path: PathBuf,
    /// The temporary directory [`RecentFilesFile::for_tests`] made, removed with it.
    #[cfg(any(test, feature = "test-support"))]
    _temporary_dir: Option<tempfile::TempDir>,
}

impl RecentFilesFile {
    /// The recent files file in `config_dir`, which need not exist yet.
    pub fn in_dir(config_dir: &Path) -> Self {
        Self {
            path: config_dir.join("recent-files.toml"),
            #[cfg(any(test, feature = "test-support"))]
            _temporary_dir: None,
        }
    }

    /// A recent files file in a new, empty temporary directory, removed when the file
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

    /// The Recent files in the file, or none when there is no file. A file in an
    /// older format is read as this build's.
    pub(crate) fn load(&self) -> Result<RecentFiles, LoadError> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(RecentFiles::default());
            }
            Err(error) => return Err(LoadError::Read(error)),
        };
        let table: toml::Table = toml::from_str(&text)?;
        let version = match table.get(VERSION_KEY) {
            Some(toml::Value::Integer(version)) => *version,
            _ => return Err(LoadError::NoVersion),
        };
        let contents: Contents = migrate(table, version)?.try_into()?;
        let mut files = contents
            .files
            .into_iter()
            .map(|entry| {
                Ok(RecentFile {
                    path: entry.path,
                    last_view: entry.last_view.map(Route::try_from).transpose()?,
                })
            })
            .collect::<Result<Vec<_>, LoadError>>()?;
        files.truncate(RecentFiles::MAX);
        Ok(RecentFiles(files))
    }

    /// Writes `recent` to the file, creating its directory if need be. The file is
    /// written whole beside the old one and then renamed over it, so a failed write
    /// leaves the old file as it was.
    pub(crate) fn save(&self, recent: &RecentFiles) -> Result<(), SaveError> {
        let files = recent
            .0
            .iter()
            .map(|listed| Entry {
                path: listed.path.clone(),
                last_view: listed.last_view.map(LastView::from),
            })
            .collect();
        let text = toml::to_string(&Contents {
            version: VERSION,
            files,
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

/// Why the recent files file couldn't be read.
#[derive(Debug, thiserror::Error)]
pub(crate) enum LoadError {
    #[error("reading the recent files file failed")]
    Read(#[source] std::io::Error),
    #[error("the recent files file is not valid")]
    Invalid(#[from] toml::de::Error),
    #[error("the recent files file has no version")]
    NoVersion,
    #[error("the recent files file saves an account on a tab other than Accounts: {0:?}")]
    AccountOffAccountsTab(TopLevelTab),
    #[error("the recent files file has a version this build can't read: {0}")]
    UnknownVersion(i64),
}

/// Why the recent files file couldn't be written.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SaveError {
    /// A path that isn't valid UTF-8 can't be written as TOML.
    #[error("the recent files can't be written as TOML")]
    Serialize(#[from] toml::ser::Error),
    #[error("writing the recent files file failed")]
    Write(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    /// A recent files file in a new, empty directory, which lasts as long as the
    /// returned `TempDir`.
    fn file() -> (TempDir, RecentFilesFile) {
        let dir = tempfile::tempdir().unwrap();
        let file = RecentFilesFile::in_dir(dir.path());
        (dir, file)
    }

    fn paths(recent: &RecentFiles) -> Vec<&Path> {
        recent.paths().collect()
    }

    fn recent(paths: &[&str]) -> RecentFiles {
        let mut recent = RecentFiles::default();
        for path in paths.iter().rev() {
            recent.note_opened(PathBuf::from(path));
        }
        recent
    }

    #[test]
    fn an_opened_file_goes_to_the_top() {
        let mut recent = recent(&["/a.balanced", "/b.balanced"]);

        recent.note_opened(PathBuf::from("/c.balanced"));

        assert_eq!(
            paths(&recent),
            [
                Path::new("/c.balanced"),
                Path::new("/a.balanced"),
                Path::new("/b.balanced"),
            ]
        );
        assert_eq!(recent.newest(), Some(Path::new("/c.balanced")));
    }

    #[test]
    fn a_listed_file_opened_again_moves_to_the_top_once() {
        let mut recent = recent(&["/a.balanced", "/b.balanced", "/c.balanced"]);

        recent.note_opened(PathBuf::from("/b.balanced"));

        assert_eq!(
            paths(&recent),
            [
                Path::new("/b.balanced"),
                Path::new("/a.balanced"),
                Path::new("/c.balanced"),
            ]
        );
    }

    #[test]
    fn a_fifth_file_drops_the_oldest() {
        let mut recent = recent(&["/a.balanced", "/b.balanced", "/c.balanced", "/d.balanced"]);

        recent.note_opened(PathBuf::from("/e.balanced"));

        assert_eq!(
            paths(&recent),
            [
                Path::new("/e.balanced"),
                Path::new("/a.balanced"),
                Path::new("/b.balanced"),
                Path::new("/c.balanced"),
            ]
        );
    }

    #[test]
    fn a_file_opened_again_keeps_the_view_it_closed_on() {
        let mut recent = recent(&["/a.balanced", "/b.balanced"]);
        let account = Route::Account(AccountId::from(3));
        recent.note_closed(Path::new("/b.balanced"), account);

        recent.note_opened(PathBuf::from("/b.balanced"));

        assert_eq!(recent.last_view(Path::new("/b.balanced")), Some(account));
        assert_eq!(recent.last_view(Path::new("/a.balanced")), None);
    }

    #[test]
    fn closing_a_file_not_listed_saves_nothing() {
        let mut recent = recent(&["/a.balanced"]);

        recent.note_closed(Path::new("/b.balanced"), Route::Tab(TopLevelTab::Bills));

        assert_eq!(recent, self::recent(&["/a.balanced"]));
    }

    #[test]
    fn no_file_loads_an_empty_list() {
        let (_dir, file) = file();

        let loaded = file.load().unwrap();

        assert_eq!(loaded.newest(), None);
    }

    #[test]
    fn the_file_holds_its_version_and_each_file_with_the_view_it_closed_on() {
        let (dir, file) = file();
        let mut saved = recent(&["/c.balanced", "/b.balanced", "/a.balanced"]);
        saved.note_closed(Path::new("/c.balanced"), Route::Account(AccountId::from(3)));
        saved.note_closed(
            Path::new("/b.balanced"),
            Route::Tab(TopLevelTab::PropertyDebt),
        );

        file.save(&saved).unwrap();

        let text = std::fs::read_to_string(dir.path().join("recent-files.toml")).unwrap();
        assert_eq!(
            text,
            "version = 2\n\
             \n\
             [[files]]\n\
             path = \"/c.balanced\"\n\
             \n\
             [files.last_view]\n\
             tab = \"accounts\"\n\
             account = 3\n\
             \n\
             [[files]]\n\
             path = \"/b.balanced\"\n\
             \n\
             [files.last_view]\n\
             tab = \"property_debt\"\n\
             \n\
             [[files]]\n\
             path = \"/a.balanced\"\n"
        );
    }

    #[test]
    fn saved_files_load_back() {
        let (_dir, file) = file();
        let mut saved = recent(&["/c.balanced", "/b.balanced", "/a.balanced"]);
        saved.note_closed(Path::new("/c.balanced"), Route::Account(AccountId::from(3)));
        saved.note_closed(Path::new("/b.balanced"), Route::Tab(TopLevelTab::Accounts));

        file.save(&saved).unwrap();
        let loaded = file.load().unwrap();

        assert_eq!(loaded, saved);
    }

    #[test]
    fn a_version_1_file_loads_its_files_with_no_saved_views() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("recent-files.toml"),
            "version = 1\nfiles = [\"/b.balanced\", \"/a.balanced\"]\n",
        )
        .unwrap();

        let loaded = file.load().unwrap();

        assert_eq!(loaded, recent(&["/b.balanced", "/a.balanced"]));
    }

    #[test]
    fn an_account_on_a_tab_other_than_accounts_is_an_error() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("recent-files.toml"),
            "version = 2\n\
             [[files]]\n\
             path = \"/a.balanced\"\n\
             last_view = { tab = \"bills\", account = 3 }\n",
        )
        .unwrap();

        let loaded = file.load();

        assert!(matches!(
            loaded,
            Err(LoadError::AccountOffAccountsTab(TopLevelTab::Bills))
        ));
    }

    #[test]
    fn a_version_from_a_later_build_is_an_error() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("recent-files.toml"),
            "version = 3\nfiles = []\n",
        )
        .unwrap();

        let loaded = file.load();

        assert!(matches!(loaded, Err(LoadError::UnknownVersion(3))));
    }

    #[test]
    fn saving_creates_the_config_directory_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("balanced");
        let file = RecentFilesFile::in_dir(&config_dir);

        file.save(&recent(&["/a.balanced"])).unwrap();

        let mut names: Vec<_> = std::fs::read_dir(&config_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names, ["recent-files.toml"]);
    }
}
