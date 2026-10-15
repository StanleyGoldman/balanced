//! Categories: what money was spent on or came from, in a tree.

/// Identifies a category for as long as it exists. Ids are never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CategoryId(u32);

impl CategoryId {
    pub(crate) fn new(id: u32) -> Self {
        Self(id)
    }
}

/// A category: its name, and the category it sits under, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    id: CategoryId,
    name: String,
    parent: Option<CategoryId>,
}

impl Category {
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
}

/// A category to add to a new [`Ledger`](crate::Ledger). Its id is its position in the
/// list of new categories, counting from 0, and its parent comes earlier in that list.
#[derive(Debug, Clone)]
pub(crate) struct NewCategory {
    name: String,
    parent: Option<CategoryId>,
}

impl NewCategory {
    /// A category at the top of the tree.
    pub(crate) fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            parent: None,
        }
    }

    /// The same category, under `parent`.
    pub(crate) fn under(self, parent: CategoryId) -> Self {
        Self {
            parent: Some(parent),
            ..self
        }
    }

    pub(crate) fn into_category(self, id: CategoryId) -> Category {
        Category {
            id,
            name: self.name,
            parent: self.parent,
        }
    }
}
