//! Picking a data file to create or open, and why one was refused, as the refusal
//! Dialog says it.

use std::path::{Path, PathBuf};
use std::thread;

use balanced_core::OpenError;
use balanced_i18n::Localizer;
use rfd::{AsyncFileDialog, FileHandle};

use crate::command::{AppCommand, CommandSender};
use crate::i18n::fl;

/// The native file picker File → New Data File… or File → Open Data File… shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DataFilePicker {
    /// The save picker, which asks before replacing a file already there.
    New,
    /// The open picker.
    Open,
}

impl DataFilePicker {
    /// Shows the picker, listing `.balanced` files, and leaves the Main window free
    /// while it is open. A picked path comes back as `AppCommand::NewDataFile` or
    /// `AppCommand::OpenDataFile` through `commands`; cancelling sends nothing.
    pub(crate) fn show(self, i18n: &Localizer, commands: &CommandSender) {
        // The picker is built here, on the UI thread, as macOS requires, and waited
        // for on a worker thread.
        let picker =
            AsyncFileDialog::new().add_filter(fl!(i18n, "dialog-data-file-filter"), &["balanced"]);
        match self {
            Self::New => wait_for(
                picker
                    .set_file_name(fl!(i18n, "dialog-data-file-suggested-name"))
                    .save_file(),
                AppCommand::NewDataFile,
                commands,
            ),
            Self::Open => wait_for(picker.pick_file(), AppCommand::OpenDataFile, commands),
        }
    }
}

/// Waits for `picked`, a picker already showing, on a worker thread, and sends `command` with the path it
/// returns, if any, through `commands`.
fn wait_for(
    picked: impl Future<Output = Option<FileHandle>> + Send + 'static,
    command: fn(PathBuf) -> AppCommand,
    commands: &CommandSender,
) {
    let commands = commands.clone();
    let spawned = thread::Builder::new()
        .name("data-file-picker".to_owned())
        .spawn(move || {
            if let Some(file) = pollster::block_on(picked) {
                commands.send(command(file.path().to_owned()));
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(
            ?error,
            "starting the thread that waits for the file picker failed"
        );
    }
}

/// A data file the App refused to create or open, and why. The open file stays open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataFileRefusal {
    /// The refused file's name, without its folder.
    file: String,
    reason: RefusalReason,
}

/// Why a data file was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RefusalReason {
    /// No file is at the path.
    Missing,
    /// The file isn't SQLite, or another program's SQLite file.
    NotBalanced,
    /// The file is damaged, or holds something no Balanced could have written.
    Damaged,
    /// The file is from a newer Balanced.
    Newer,
    /// Another program, or another Balanced, has the file open.
    Locked,
    /// The file is from an older Balanced, and the backup taken before upgrading it
    /// couldn't be written.
    BackupFailed,
    /// Upgrading the file from an older Balanced failed. `backup` is the name of the
    /// backup taken first, without its folder.
    UpgradeFailed { backup: String },
    /// Reading the file failed.
    Unreadable,
    /// Creating a new file failed.
    NotCreated,
    /// A new file would have replaced the open one.
    OpenFileNotReplaced,
}

impl DataFileRefusal {
    /// The refusal of the file at `path`, which opening refused with `error`.
    pub(crate) fn open(path: &Path, error: &OpenError) -> Self {
        let reason = match error {
            OpenError::Missing(_) => RefusalReason::Missing,
            OpenError::NotBalanced => RefusalReason::NotBalanced,
            // The schema admits a currency, kind or date Balanced can't read only in a
            // file something other than Balanced changed.
            OpenError::Damaged(_)
            | OpenError::Currency(_)
            | OpenError::Unknown { .. }
            | OpenError::Date { .. } => RefusalReason::Damaged,
            OpenError::Newer { .. } => RefusalReason::Newer,
            OpenError::Locked => RefusalReason::Locked,
            OpenError::BackupFailed(_) => RefusalReason::BackupFailed,
            OpenError::UpgradeFailed { backup, .. } => RefusalReason::UpgradeFailed {
                backup: Self::file_name(backup),
            },
            OpenError::Store(_) => RefusalReason::Unreadable,
        };
        Self::new(path, reason)
    }

    /// The refusal of a new data file at `path`, which couldn't be created there.
    pub(crate) fn not_created(path: &Path) -> Self {
        Self::new(path, RefusalReason::NotCreated)
    }

    /// The refusal of a new data file at `path`, where the open data file is.
    pub(crate) fn open_file_not_replaced(path: &Path) -> Self {
        Self::new(path, RefusalReason::OpenFileNotReplaced)
    }

    fn new(path: &Path, reason: RefusalReason) -> Self {
        Self {
            file: Self::file_name(path),
            reason,
        }
    }

    /// The name of the file at `path`, without its folder.
    fn file_name(path: &Path) -> String {
        path.file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned()
    }

    /// What the refusal Dialog says.
    pub(crate) fn message(&self, i18n: &Localizer) -> String {
        let file = self.file.clone();
        match &self.reason {
            RefusalReason::Missing => fl!(i18n, "dialog-data-file-missing", file = file),
            RefusalReason::NotBalanced => {
                fl!(i18n, "dialog-data-file-not-balanced", file = file)
            }
            RefusalReason::Damaged => fl!(i18n, "dialog-data-file-damaged", file = file),
            RefusalReason::Newer => fl!(i18n, "dialog-data-file-newer", file = file),
            RefusalReason::Locked => fl!(i18n, "dialog-data-file-locked", file = file),
            RefusalReason::BackupFailed => {
                fl!(i18n, "dialog-data-file-backup-failed", file = file)
            }
            RefusalReason::UpgradeFailed { backup } => fl!(
                i18n,
                "dialog-data-file-upgrade-failed",
                file = file,
                backup = backup.clone()
            ),
            RefusalReason::Unreadable => fl!(i18n, "dialog-data-file-unreadable", file = file),
            RefusalReason::NotCreated => fl!(i18n, "dialog-data-file-not-created", file = file),
            RefusalReason::OpenFileNotReplaced => {
                fl!(i18n, "dialog-data-file-open-not-replaced", file = file)
            }
        }
    }
}
