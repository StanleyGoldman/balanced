// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Categories: what money was spent on or came from, in a tree.

/// Identifies a category for as long as it exists. Ids are never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CategoryId(u32);

impl CategoryId {
    pub(crate) const fn new(id: u32) -> Self {
        Self(id)
    }

    /// The id after this one.
    pub(crate) fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// The id as the data file stores it.
    pub(crate) fn raw(self) -> u32 {
        self.0
    }
}

/// Whether a category is money coming in or going out. A subcategory has its top-level
/// category's type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CategoryType {
    Income,
    Expense,
}

impl CategoryType {
    pub const ALL: [Self; 2] = [Self::Income, Self::Expense];
}

/// A category: its name, its type, and the category it sits under, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    id: CategoryId,
    name: String,
    parent: Option<CategoryId>,
    #[expect(
        clippy::struct_field_names,
        reason = "the glossary's term is category type, and `type` is a keyword"
    )]
    category_type: CategoryType,
}

impl Category {
    pub(crate) fn new(
        id: CategoryId,
        name: String,
        parent: Option<CategoryId>,
        category_type: CategoryType,
    ) -> Self {
        Self {
            id,
            name,
            parent,
            category_type,
        }
    }

    pub fn id(&self) -> CategoryId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The category this one sits under, or `None` at the top of the tree.
    pub fn parent(&self) -> Option<CategoryId> {
        self.parent
    }

    /// Whether it is Income or Expense: the same as its top-level category's.
    pub fn category_type(&self) -> CategoryType {
        self.category_type
    }
}

/// A category to add to a new [`Ledger`](crate::Ledger). Its id is its position in the
/// list of new categories, counting from 0, and its parent comes earlier in that list.
#[derive(Debug, Clone)]
pub(crate) struct NewCategory {
    name: String,
    parent: Option<CategoryId>,
    category_type: CategoryType,
}

impl NewCategory {
    /// An Expense category at the top of the tree.
    pub(crate) fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            parent: None,
            category_type: CategoryType::Expense,
        }
    }

    /// The same category, an Income one.
    pub(crate) fn income(self) -> Self {
        Self {
            category_type: CategoryType::Income,
            ..self
        }
    }

    /// The same category, under `parent`.
    pub(crate) fn under(self, parent: CategoryId) -> Self {
        Self {
            parent: Some(parent),
            ..self
        }
    }

    /// The category it sits under, or `None` at the top of the tree.
    pub(crate) fn parent(&self) -> Option<CategoryId> {
        self.parent
    }

    /// The category, with id `id`. A subcategory takes `parent_type`, its parent's
    /// type, in place of its own.
    pub(crate) fn into_category(
        self,
        id: CategoryId,
        parent_type: Option<CategoryType>,
    ) -> Category {
        Category {
            id,
            name: self.name,
            parent: self.parent,
            category_type: parent_type.unwrap_or(self.category_type),
        }
    }
}
