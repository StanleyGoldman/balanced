//! [`Layout`]: how the Main window was laid out, kept in eframe storage between runs.
//! It holds the Account bar's width and fold, its folded sections and the Register's
//! column widths, and nothing else: no financial data and no navigation.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::account_bar::{AccountBar, AccountBarFold};
use crate::accounts_model::Section;
use crate::register::ColumnWidths;

/// The eframe storage key the layout is saved under.
const STORAGE_KEY: &str = "layout";

/// The version of the saved format. A change of meaning bumps it, and an entry of any
/// other version is ignored.
const VERSION: u32 = 1;

/// The Main window's layout as it is saved. A field the entry lacks takes its value
/// from the first-run layout, except the version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Layout {
    /// An entry with no version reads as version 0, which no layout has.
    #[serde(default)]
    version: u32,
    account_bar_width: f32,
    /// Whether the Account bar is folded to a rail.
    account_bar_folded: bool,
    /// The ids of the folded Account bar sections.
    folded_sections: Vec<String>,
    /// Register column widths by column id, shared by every Register.
    column_widths: BTreeMap<String, f32>,
}

impl Default for Layout {
    /// The layout of a first run: only Closed is folded, and everything else has its
    /// default size.
    fn default() -> Self {
        Self::new(
            AccountBar::DEFAULT_WIDTH,
            AccountBarFold::Open,
            &HashSet::from([Section::Closed]),
            &ColumnWidths::default(),
        )
    }
}

impl Layout {
    /// The layout of an Account bar `account_bar_width` wide, folded as `fold`, with
    /// `folded_sections` folded, over Registers with `column_widths`.
    pub(crate) fn new(
        account_bar_width: f32,
        fold: AccountBarFold,
        folded_sections: &HashSet<Section>,
        column_widths: &ColumnWidths,
    ) -> Self {
        Self {
            version: VERSION,
            account_bar_width,
            account_bar_folded: match fold {
                AccountBarFold::Open => false,
                AccountBarFold::Rail => true,
            },
            folded_sections: Section::ALL
                .into_iter()
                .filter(|section| folded_sections.contains(section))
                .map(|section| section.id().to_owned())
                .collect(),
            column_widths: column_widths.to_saved(),
        }
    }

    /// The layout `storage` holds, or the default when it holds none, or one that
    /// can't be read or has another version.
    pub(crate) fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        storage
            .and_then(|storage| eframe::get_value::<Self>(storage, STORAGE_KEY))
            .filter(|layout| layout.version == VERSION)
            .unwrap_or_default()
    }

    /// Saves the layout in `storage`, in place of the one there.
    pub(crate) fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, self);
    }

    /// The Account bar's width, clamped to the widths it allows.
    pub(crate) fn account_bar_width(&self) -> f32 {
        if self.account_bar_width.is_finite() {
            self.account_bar_width
                .clamp(AccountBar::MIN_WIDTH, AccountBar::MAX_WIDTH)
        } else {
            AccountBar::DEFAULT_WIDTH
        }
    }

    /// Whether the Account bar is folded to a rail.
    pub(crate) fn account_bar_fold(&self) -> AccountBarFold {
        if self.account_bar_folded {
            AccountBarFold::Rail
        } else {
            AccountBarFold::Open
        }
    }

    /// The folded Account bar sections. An unknown id is ignored.
    pub(crate) fn folded_sections(&self) -> HashSet<Section> {
        Section::ALL
            .into_iter()
            .filter(|section| self.folded_sections.iter().any(|id| id == section.id()))
            .collect()
    }

    /// The Register column widths. An unknown column id is ignored.
    pub(crate) fn column_widths(&self) -> ColumnWidths {
        ColumnWidths::from_saved(&self.column_widths)
    }
}

/// eframe storage in memory, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct MemoryStorage(std::collections::HashMap<String, String>);

#[cfg(test)]
impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }

    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }

    fn flush(&mut self) {}
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use balanced_core::AccountGroup;

    use super::*;

    /// Storage holding `entry` as the saved layout.
    fn storage_with(entry: &str) -> MemoryStorage {
        MemoryStorage(HashMap::from([(STORAGE_KEY.to_owned(), entry.to_owned())]))
    }

    #[test]
    fn a_saved_layout_loads_back() {
        let widths = ColumnWidths::from_saved(&BTreeMap::from([("memo".to_owned(), 210.0)]));
        let folded = HashSet::from([Section::Group(AccountGroup::Investing)]);
        let layout = Layout::new(260.0, AccountBarFold::Rail, &folded, &widths);
        let mut storage = MemoryStorage::default();

        layout.save(&mut storage);
        let loaded = Layout::load(Some(&storage));

        assert_eq!(loaded.account_bar_width(), 260.0);
        assert_eq!(loaded.account_bar_fold(), AccountBarFold::Rail);
        assert_eq!(loaded.folded_sections(), folded);
        assert_eq!(loaded.column_widths(), widths);
    }

    #[test]
    fn with_nothing_saved_only_closed_is_folded() {
        let layout = Layout::load(Some(&MemoryStorage::default()));

        assert_eq!(layout.folded_sections(), HashSet::from([Section::Closed]));
        assert_eq!(layout.account_bar_fold(), AccountBarFold::Open);
        assert_eq!(layout.account_bar_width(), AccountBar::DEFAULT_WIDTH);
    }

    #[test]
    fn the_account_bar_width_is_clamped_on_restore() {
        let narrow = Layout::load(Some(&storage_with("(version: 1, account_bar_width: 10.0)")));
        let wide = Layout::load(Some(&storage_with(
            "(version: 1, account_bar_width: 9000.0)",
        )));

        assert_eq!(narrow.account_bar_width(), AccountBar::MIN_WIDTH);
        assert_eq!(wide.account_bar_width(), AccountBar::MAX_WIDTH);
    }

    #[test]
    fn unknown_section_ids_are_ignored() {
        let storage =
            storage_with(r#"(version: 1, folded_sections: ["savings_bonds", "banking"])"#);

        let layout = Layout::load(Some(&storage));

        assert_eq!(
            layout.folded_sections(),
            HashSet::from([Section::Group(AccountGroup::Banking)])
        );
    }

    #[test]
    fn unknown_column_ids_are_ignored() {
        let storage = storage_with(r#"(version: 1, column_widths: {"memo": 210.0, "tags": 50.0})"#);

        let layout = Layout::load(Some(&storage));

        let expected = ColumnWidths::from_saved(&BTreeMap::from([("memo".to_owned(), 210.0)]));
        assert_eq!(layout.column_widths(), expected);
    }

    #[test]
    fn an_unreadable_entry_falls_back_to_defaults() {
        let layout = Layout::load(Some(&storage_with("not a layout {")));

        assert_eq!(layout, Layout::default());
    }

    #[test]
    fn an_entry_without_a_version_falls_back_to_defaults() {
        let storage = storage_with(r#"(folded_sections: ["banking"])"#);

        let layout = Layout::load(Some(&storage));

        assert_eq!(layout, Layout::default());
    }

    #[test]
    fn an_entry_of_another_version_falls_back_to_defaults() {
        let storage = storage_with(r#"(version: 2, folded_sections: ["banking"])"#);

        let layout = Layout::load(Some(&storage));

        assert_eq!(layout, Layout::default());
    }
}
