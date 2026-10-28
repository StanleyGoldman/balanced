// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Hiding a category, with every category under it, and showing it again.

use Visibility::{Hidden, Shown};
use balanced_core::{
    CategoryId, CategoryType, CurrencyCode, DomainCommand, DomainError, Ledger, Visibility,
};
use tempfile::TempDir;

/// A household's categories in a new data file:
///
/// ```text
/// Food (Expense)          Income (Income)
///   Groceries               Salary
///   Restaurants
///     Coffee
/// ```
struct Household {
    /// Holds the data file, which a test can reopen.
    dir: TempDir,
    ledger: Ledger,
    food: CategoryId,
    groceries: CategoryId,
    restaurants: CategoryId,
    coffee: CategoryId,
    income: CategoryId,
    salary: CategoryId,
}

impl Household {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("the temp dir is writable");
        let currency = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
        let mut ledger = Ledger::create(&dir.path().join("Household.balanced"), currency)
            .expect("a new path takes a data file");
        let mut create = |name: &str, parent, category_type| {
            ledger
                .apply(DomainCommand::CreateCategory {
                    name: name.to_owned(),
                    parent,
                    category_type,
                })
                .expect("the ledger creates the category");
            id(&ledger, name)
        };
        let food = create("Food", None, CategoryType::Expense);
        let groceries = create("Groceries", Some(food), CategoryType::Expense);
        let restaurants = create("Restaurants", Some(food), CategoryType::Expense);
        let coffee = create("Coffee", Some(restaurants), CategoryType::Expense);
        let income = create("Income", None, CategoryType::Income);
        let salary = create("Salary", Some(income), CategoryType::Income);
        Self {
            dir,
            ledger,
            food,
            groceries,
            restaurants,
            coffee,
            income,
            salary,
        }
    }

    fn apply(&mut self, command: DomainCommand) -> Result<(), DomainError> {
        self.ledger.apply(command).map(drop)
    }

    fn hide(&mut self, category: CategoryId) {
        self.apply(set_visibility(category, Visibility::Hidden))
            .expect("the ledger hides the category");
    }

    /// The visibility of each category, Food's branch first, then Income's.
    fn visibilities(&self) -> [Visibility; 6] {
        visibilities(
            &self.ledger,
            [
                self.food,
                self.groceries,
                self.restaurants,
                self.coffee,
                self.income,
                self.salary,
            ],
        )
    }
}

/// The id of the category named `name`, which only one category is.
fn id(ledger: &Ledger, name: &str) -> CategoryId {
    ledger
        .snapshot()
        .categories()
        .iter()
        .find(|category| category.name() == name)
        .expect("the test created the category")
        .id()
}

fn visibilities<const N: usize>(ledger: &Ledger, ids: [CategoryId; N]) -> [Visibility; N] {
    let view = ledger.snapshot();
    ids.map(|id| {
        view.category(id)
            .expect("the category is in the ledger")
            .visibility()
    })
}

fn set_visibility(category: CategoryId, visibility: Visibility) -> DomainCommand {
    DomainCommand::SetCategoryVisibility {
        category,
        visibility,
    }
}

#[test]
fn a_new_category_is_shown() {
    let household = Household::new();

    assert_eq!(household.visibilities(), [Shown; 6]);
}

#[test]
fn hiding_a_category_hides_every_category_under_it() {
    let mut household = Household::new();

    household.hide(household.food);

    assert_eq!(
        household.visibilities(),
        [Hidden, Hidden, Hidden, Hidden, Shown, Shown]
    );
}

#[test]
fn showing_a_subcategory_shows_the_categories_above_it_but_not_their_others() {
    let mut household = Household::new();
    household.hide(household.food);

    household
        .apply(set_visibility(household.coffee, Shown))
        .unwrap();

    assert_eq!(
        household.visibilities(),
        [Shown, Hidden, Shown, Shown, Shown, Shown]
    );
}

#[test]
fn one_undo_hides_again_a_subcategory_and_the_categories_shown_above_it() {
    let mut household = Household::new();
    household.hide(household.food);
    household
        .apply(set_visibility(household.coffee, Shown))
        .unwrap();

    household.ledger.undo().unwrap();

    assert_eq!(
        household.visibilities(),
        [Hidden, Hidden, Hidden, Hidden, Shown, Shown]
    );
}

#[test]
fn redo_shows_a_subcategory_and_the_categories_above_it_again() {
    let mut household = Household::new();
    household.hide(household.food);
    household
        .apply(set_visibility(household.coffee, Shown))
        .unwrap();
    household.ledger.undo().unwrap();

    household.ledger.redo().unwrap();

    assert_eq!(
        household.visibilities(),
        [Shown, Hidden, Shown, Shown, Shown, Shown]
    );
}

#[test]
fn undo_gives_each_category_under_a_hidden_one_its_old_visibility_back() {
    let mut household = Household::new();
    household.hide(household.groceries);
    household.hide(household.food);

    household.ledger.undo().unwrap();

    assert_eq!(
        household.visibilities(),
        [Shown, Hidden, Shown, Shown, Shown, Shown]
    );
}

#[test]
fn undo_hides_again_the_hidden_categories_under_one_shown() {
    let mut household = Household::new();
    household.hide(household.groceries);

    household
        .apply(set_visibility(household.food, Shown))
        .unwrap();
    household.ledger.undo().unwrap();

    assert_eq!(
        household.visibilities(),
        [Shown, Hidden, Shown, Shown, Shown, Shown]
    );
}

#[test]
fn redo_hides_the_category_and_those_under_it_again() {
    let mut household = Household::new();
    household.hide(household.groceries);
    household.hide(household.food);
    household.ledger.undo().unwrap();

    household.ledger.redo().unwrap();

    assert_eq!(
        household.visibilities(),
        [Hidden, Hidden, Hidden, Hidden, Shown, Shown]
    );
}

#[test]
fn hiding_an_unknown_category_is_refused() {
    let mut household = Household::new();
    household
        .apply(DomainCommand::DeleteCategory {
            category: household.salary,
        })
        .unwrap();
    let before = household.ledger.snapshot();

    let result = household.apply(set_visibility(household.salary, Hidden));

    assert_eq!(
        result,
        Err(DomainError::UnknownCategory {
            category: household.salary
        })
    );
    assert_eq!(household.ledger.snapshot(), before);
}

#[test]
fn a_deleted_hidden_category_comes_back_hidden() {
    let mut household = Household::new();
    household.hide(household.salary);
    household
        .apply(DomainCommand::DeleteCategory {
            category: household.salary,
        })
        .unwrap();

    household.ledger.undo().unwrap();

    assert_eq!(household.visibilities()[5], Hidden);
}

#[test]
fn hidden_categories_stay_hidden_after_reopening_the_data_file_and_undo_still_works() {
    let mut household = Household::new();
    household.hide(household.groceries);
    household.hide(household.food);
    let path = household.dir.path().join("Household.balanced");
    let ids = [household.food, household.groceries, household.restaurants];
    drop(household.ledger);

    let mut reopened = Ledger::open(&path).expect("the data file opens");
    assert_eq!(visibilities(&reopened, ids), [Hidden, Hidden, Hidden]);
    reopened.undo().unwrap();

    assert_eq!(visibilities(&reopened, ids), [Shown, Hidden, Shown]);
}

/// What hiding `category` in `household` gives back to undo it.
fn hiding_inverse(household: &mut Household, category: CategoryId) -> DomainCommand {
    household
        .ledger
        .apply(set_visibility(category, Hidden))
        .expect("the ledger hides the category")
        .inverse()
        .clone()
}

#[test]
fn hiding_a_branch_that_was_all_shown_is_undone_by_one_command() {
    let mut household = Household::new();

    let food = household.food;
    let inverse = hiding_inverse(&mut household, food);

    assert_eq!(inverse, set_visibility(household.food, Shown));
}

#[test]
fn hiding_a_branch_is_undone_by_a_command_for_each_category_shown_otherwise_than_its_parent() {
    let mut household = Household::new();
    household.hide(household.groceries);
    household.hide(household.restaurants);

    let food = household.food;
    let inverse = hiding_inverse(&mut household, food);

    assert_eq!(
        inverse,
        DomainCommand::Compound(vec![
            set_visibility(household.food, Shown),
            set_visibility(household.groceries, Hidden),
            set_visibility(household.restaurants, Hidden),
        ])
    );
}

#[test]
fn undo_and_redo_again_and_again_restore_each_category_the_same_way() {
    let mut household = Household::new();
    household.hide(household.groceries);
    household.hide(household.food);
    household.ledger.undo().unwrap();
    household.ledger.redo().unwrap();

    household.ledger.undo().unwrap();

    assert_eq!(
        household.visibilities(),
        [Shown, Hidden, Shown, Shown, Shown, Shown]
    );
}

#[test]
fn a_category_created_under_a_hidden_one_starts_hidden() {
    let mut household = Household::new();
    household.hide(household.restaurants);

    household
        .apply(DomainCommand::CreateCategory {
            name: "Takeout".to_owned(),
            parent: Some(household.restaurants),
            category_type: CategoryType::Expense,
        })
        .unwrap();

    let takeout = id(&household.ledger, "Takeout");
    assert_eq!(visibilities(&household.ledger, [takeout]), [Hidden]);
}

#[test]
fn a_category_moved_under_a_hidden_one_is_hidden_with_those_under_it() {
    let mut household = Household::new();
    household.hide(household.income);

    household
        .apply(DomainCommand::MoveCategory {
            category: household.restaurants,
            parent: Some(household.income),
        })
        .unwrap();

    assert_eq!(
        household.visibilities(),
        [Shown, Shown, Hidden, Hidden, Hidden, Hidden]
    );
}

#[test]
fn a_category_moved_under_a_shown_one_stays_shown() {
    let mut household = Household::new();

    household
        .apply(DomainCommand::MoveCategory {
            category: household.restaurants,
            parent: Some(household.income),
        })
        .unwrap();

    let view = household.ledger.snapshot();
    let restaurants = view.category(household.restaurants).unwrap();
    assert_eq!(restaurants.parent(), Some(household.income));
    assert_eq!(
        household.visibilities(),
        [Shown, Shown, Shown, Shown, Shown, Shown]
    );
}

#[test]
fn undo_moves_a_category_back_from_under_a_hidden_one_and_shows_it_again() {
    let mut household = Household::new();
    household.hide(household.coffee);
    household.hide(household.income);
    household
        .apply(DomainCommand::MoveCategory {
            category: household.restaurants,
            parent: Some(household.income),
        })
        .unwrap();

    household.ledger.undo().unwrap();

    let view = household.ledger.snapshot();
    let restaurants = view.category(household.restaurants).unwrap();
    assert_eq!(restaurants.parent(), Some(household.food));
    assert_eq!(
        household.visibilities(),
        [Shown, Shown, Shown, Hidden, Hidden, Hidden]
    );
}

#[test]
fn redo_moves_a_category_under_a_hidden_one_again_and_hides_it() {
    let mut household = Household::new();
    household.hide(household.income);
    household
        .apply(DomainCommand::MoveCategory {
            category: household.restaurants,
            parent: Some(household.income),
        })
        .unwrap();
    household.ledger.undo().unwrap();

    household.ledger.redo().unwrap();

    assert_eq!(
        household.visibilities(),
        [Shown, Shown, Hidden, Hidden, Hidden, Hidden]
    );
}
