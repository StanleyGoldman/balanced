//! The [`Ledger`]: the financial data, and the snapshots windows read it through.

use std::sync::Arc;

/// Counts the changes applied to a [`Ledger`]. A window's view model rebuilds when
/// the generation of the snapshot it reads changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Generation(u64);

impl Generation {
    /// The generation of a ledger no change has been applied to.
    pub const FIRST: Self = Self(0);
}

/// An immutable snapshot of the ledger, shared by every window that reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerView {
    generation: Generation,
}

impl LedgerView {
    /// Which change to the ledger this snapshot follows.
    pub fn generation(&self) -> Generation {
        self.generation
    }
}

/// The financial data. It changes only by applying a `DomainCommand`, and windows
/// read it through [`Ledger::snapshot`].
#[derive(Debug)]
pub struct Ledger {
    view: Arc<LedgerView>,
}

impl Ledger {
    /// The in-memory data Balanced runs on until it reads data files.
    pub fn stub() -> Self {
        Self {
            view: Arc::new(LedgerView {
                generation: Generation::FIRST,
            }),
        }
    }

    /// The current snapshot. It stays valid, unchanged, after the ledger changes.
    pub fn snapshot(&self) -> Arc<LedgerView> {
        Arc::clone(&self.view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stub_ledger_starts_at_the_first_generation() {
        let ledger = Ledger::stub();

        let view = ledger.snapshot();

        assert_eq!(view.generation(), Generation::FIRST);
    }
}
