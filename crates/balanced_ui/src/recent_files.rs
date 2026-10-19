//! [`RecentFiles`]: the data files opened lately on this computer, and
//! [`RecentFilesFile`], which keeps them in `recent-files.toml`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The data files opened lately, newest first. Recent files are a record, not a
/// Preference.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RecentFiles(Vec<PathBuf>);

impl RecentFiles {
    /// How many files the list keeps.
    const MAX: usize = 4;

    /// Puts `path` at the top, as the file just opened or created, and drops the
    /// oldest file past [`RecentFiles::MAX`].
    pub(crate) fn note_opened(&mut self, path: PathBuf) {
        self.0.retain(|listed| *listed != path);
        self.0.insert(0, path);
        self.0.truncate(Self::MAX);
    }

    /// The file opened last, if any.
    pub(crate) fn newest(&self) -> Option<&Path> {
        self.0.first().map(PathBuf::as_path)
    }

    /// The files, newest first.
    pub(crate) fn files(&self) -> &[PathBuf] {
        &self.0
    }
}

/// The version of the file's format this build writes.
const VERSION: i64 = 1;

/// The file's contents: its format version, then the files, newest first.
#[derive(Serialize, Deserialize)]
struct Contents {
    version: i64,
    files: Vec<PathBuf>,
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

    /// The Recent files in the file, or none when there is no file.
    pub(crate) fn load(&self) -> Result<RecentFiles, LoadError> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(RecentFiles::default());
            }
            Err(error) => return Err(LoadError::Read(error)),
        };
        let contents: Contents = toml::from_str(&text)?;
        if contents.version != VERSION {
            return Err(LoadError::UnknownVersion(contents.version));
        }
        let mut files = contents.files;
        files.truncate(RecentFiles::MAX);
        Ok(RecentFiles(files))
    }

    /// Writes `recent` to the file, creating its directory if need be. The file is
    /// written whole beside the old one and then renamed over it, so a failed write
    /// leaves the old file as it was.
    pub(crate) fn save(&self, recent: &RecentFiles) -> Result<(), SaveError> {
        let text = toml::to_string(&Contents {
            version: VERSION,
            files: recent.0.clone(),
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
            recent.files(),
            [
                PathBuf::from("/c.balanced"),
                PathBuf::from("/a.balanced"),
                PathBuf::from("/b.balanced"),
            ]
        );
        assert_eq!(recent.newest(), Some(Path::new("/c.balanced")));
    }

    #[test]
    fn a_listed_file_opened_again_moves_to_the_top_once() {
        let mut recent = recent(&["/a.balanced", "/b.balanced", "/c.balanced"]);

        recent.note_opened(PathBuf::from("/b.balanced"));

        assert_eq!(
            recent.files(),
            [
                PathBuf::from("/b.balanced"),
                PathBuf::from("/a.balanced"),
                PathBuf::from("/c.balanced"),
            ]
        );
    }

    #[test]
    fn a_fifth_file_drops_the_oldest() {
        let mut recent = recent(&["/a.balanced", "/b.balanced", "/c.balanced", "/d.balanced"]);

        recent.note_opened(PathBuf::from("/e.balanced"));

        assert_eq!(
            recent.files(),
            [
                PathBuf::from("/e.balanced"),
                PathBuf::from("/a.balanced"),
                PathBuf::from("/b.balanced"),
                PathBuf::from("/c.balanced"),
            ]
        );
    }

    #[test]
    fn no_file_loads_an_empty_list() {
        let (_dir, file) = file();

        let loaded = file.load().unwrap();

        assert_eq!(loaded.newest(), None);
    }

    #[test]
    fn the_file_holds_its_version_and_the_files_newest_first() {
        let (dir, file) = file();

        file.save(&recent(&["/b.balanced", "/a.balanced"])).unwrap();

        let text = std::fs::read_to_string(dir.path().join("recent-files.toml")).unwrap();
        assert_eq!(
            text,
            "version = 1\nfiles = [\"/b.balanced\", \"/a.balanced\"]\n"
        );
    }

    #[test]
    fn saved_files_load_back() {
        let (_dir, file) = file();
        let saved = recent(&["/b.balanced", "/a.balanced"]);

        file.save(&saved).unwrap();
        let loaded = file.load().unwrap();

        assert_eq!(loaded, saved);
    }

    #[test]
    fn a_version_from_a_later_build_is_an_error() {
        let (dir, file) = file();
        std::fs::write(
            dir.path().join("recent-files.toml"),
            "version = 2\nfiles = []\n",
        )
        .unwrap();

        let loaded = file.load();

        assert!(matches!(loaded, Err(LoadError::UnknownVersion(2))));
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
