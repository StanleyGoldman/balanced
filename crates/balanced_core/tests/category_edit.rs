// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Editing a category: renaming it, moving it to another parent, and changing its type.

use balanced_core::{
    AccountKind, CategoryId, CategoryType, CurrencyCode, DomainCommand, DomainError, Ledger,
    LedgerView, Money, NewTransaction,
};
use icu_calendar::{Date, Iso};
use tempfile::TempDir;

fn usd(amount: &str) -> Money {
    let currency = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
    let amount = amount.parse().expect("test amounts are decimal numbers");
    Money::new(amount, currency).expect("test amounts have two decimal places")
}

fn date(day: u8) -> Date<Iso> {
    Date::try_new_iso(2026, 9, day).expect("test dates are real dates")
}

/// A household's categories in a new data file:
///
/// ```text
/// Food (Expense)          Income (Income)
///   Groceries               Salary
///   Restaurants
///     Coffee
/// ```
///
/// and a checking account, Everyday, with a payment to Bean There in Coffee.
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
        ledger
            .apply(DomainCommand::CreateAccount {
                name: "Everyday".to_owned(),
                kind: AccountKind::Checking,
                opened: date(1),
                opening_balance: usd("100.00"),
            })
            .expect("the ledger creates the account");
        let everyday = ledger.snapshot().accounts()[0].account().id();
        ledger
            .apply(DomainCommand::PostTransaction {
                account: everyday,
                transaction: NewTransaction::new(date(3), "Bean There", coffee, usd("-4.50")),
            })
            .expect("the ledger posts the payment");
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

    fn view(&self) -> std::sync::Arc<LedgerView> {
        self.ledger.snapshot()
    }

    /// The full path of category `id`, as `Food:Restaurants:Coffee`.
    fn path(&self, id: CategoryId) -> String {
        self.view()
            .category_path(id)
            .expect("the category is in the ledger")
            .join(":")
    }

    fn type_of(&self, id: CategoryId) -> CategoryType {
        self.view()
            .category(id)
            .expect("the category is in the ledger")
            .category_type()
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

fn rename(category: CategoryId, name: &str) -> DomainCommand {
    DomainCommand::RenameCategory {
        category,
        name: name.to_owned(),
    }
}

#[test]
fn a_renamed_category_shows_its_new_name_in_every_path_under_it() {
    let mut household = Household::new();

    household
        .apply(rename(household.restaurants, "Eating Out"))
        .unwrap();

    assert_eq!(household.path(household.restaurants), "Food:Eating Out");
    assert_eq!(household.path(household.coffee), "Food:Eating Out:Coffee");
}

#[test]
fn a_rename_to_a_siblings_name_ignoring_case_is_refused() {
    let mut household = Household::new();
    let before = household.view();

    let result = household.apply(rename(household.restaurants, "groceries"));

    assert_eq!(
        result,
        Err(DomainError::DuplicateCategory {
            name: "groceries".to_owned()
        })
    );
    assert_eq!(household.view(), before);
}

#[test]
fn a_category_can_change_the_case_of_its_own_name() {
    let mut household = Household::new();

    household.apply(rename(household.food, "FOOD")).unwrap();

    assert_eq!(household.path(household.groceries), "FOOD:Groceries");
}

#[test]
fn undo_gives_a_renamed_category_its_old_name_back() {
    let mut household = Household::new();
    household
        .apply(rename(household.restaurants, "Eating Out"))
        .unwrap();

    household.ledger.undo().unwrap();

    assert_eq!(household.path(household.coffee), "Food:Restaurants:Coffee");
}

fn move_to(category: CategoryId, parent: Option<CategoryId>) -> DomainCommand {
    DomainCommand::MoveCategory { category, parent }
}

#[test]
fn a_moved_category_takes_its_subcategories_with_it() {
    let mut household = Household::new();

    household
        .apply(move_to(household.restaurants, Some(household.income)))
        .unwrap();

    assert_eq!(
        household.path(household.coffee),
        "Income:Restaurants:Coffee"
    );
}

#[test]
fn a_category_can_move_under_one_created_after_it() {
    let mut household = Household::new();

    household
        .apply(move_to(household.food, Some(household.salary)))
        .unwrap();

    assert_eq!(
        household.path(household.groceries),
        "Income:Salary:Food:Groceries"
    );
}

#[test]
fn a_category_moved_to_the_top_of_the_tree_has_no_parent() {
    let mut household = Household::new();

    household.apply(move_to(household.coffee, None)).unwrap();

    assert_eq!(household.path(household.coffee), "Coffee");
}

#[test]
fn a_move_under_the_category_itself_or_one_of_its_subcategories_is_refused() {
    let mut household = Household::new();
    let before = household.view();

    let under_itself = household.apply(move_to(household.food, Some(household.food)));
    let under_its_own = household.apply(move_to(household.food, Some(household.coffee)));

    let refused = Err(DomainError::CategoryUnderItself {
        category: household.food,
    });
    assert_eq!(under_itself, refused);
    assert_eq!(under_its_own, refused);
    assert_eq!(household.view(), before);
}

#[test]
fn a_move_under_a_parent_with_a_category_of_that_name_ignoring_case_is_refused() {
    let mut household = Household::new();
    household
        .apply(DomainCommand::CreateCategory {
            name: "groceries".to_owned(),
            parent: Some(household.income),
            category_type: CategoryType::Income,
        })
        .unwrap();
    let before = household.view();

    let result = household.apply(move_to(household.groceries, Some(household.income)));

    assert_eq!(
        result,
        Err(DomainError::DuplicateCategory {
            name: "Groceries".to_owned()
        })
    );
    assert_eq!(household.view(), before);
}

#[test]
fn a_move_under_an_unknown_parent_is_refused() {
    let mut household = Household::new();
    household
        .apply(DomainCommand::DeleteCategory {
            category: household.groceries,
        })
        .unwrap();

    let result = household.apply(move_to(household.salary, Some(household.groceries)));

    assert_eq!(
        result,
        Err(DomainError::UnknownCategory {
            category: household.groceries
        })
    );
}

#[test]
fn a_subcategory_moved_under_another_parent_takes_its_new_top_level_categorys_type() {
    let mut household = Household::new();

    household
        .apply(move_to(household.restaurants, Some(household.salary)))
        .unwrap();

    assert_eq!(
        household.type_of(household.restaurants),
        CategoryType::Income
    );
    assert_eq!(household.type_of(household.coffee), CategoryType::Income);
}

#[test]
fn a_subcategory_moved_to_the_top_keeps_its_type() {
    let mut household = Household::new();

    household.apply(move_to(household.salary, None)).unwrap();

    assert_eq!(household.type_of(household.salary), CategoryType::Income);
}

#[test]
fn undo_puts_a_moved_subcategory_back_with_its_type() {
    let mut household = Household::new();
    household
        .apply(move_to(household.restaurants, Some(household.income)))
        .unwrap();

    household.ledger.undo().unwrap();

    assert_eq!(household.path(household.coffee), "Food:Restaurants:Coffee");
    assert_eq!(household.type_of(household.coffee), CategoryType::Expense);
}

#[test]
fn undo_moves_a_top_level_category_back_to_the_top_with_its_type() {
    let mut household = Household::new();
    household
        .apply(move_to(household.food, Some(household.income)))
        .unwrap();

    household.ledger.undo().unwrap();

    assert_eq!(household.path(household.coffee), "Food:Restaurants:Coffee");
    assert_eq!(household.type_of(household.food), CategoryType::Expense);
    assert_eq!(household.type_of(household.coffee), CategoryType::Expense);
}

#[test]
fn a_top_level_category_moved_under_a_parent_of_its_own_type_is_undone_by_a_move_alone() {
    // The category keeps its type, so an Undo that also set it back would leave the
    // same categories; only the Command that undoes the move shows the extra step.
    let mut household = Household::new();
    household.apply(move_to(household.groceries, None)).unwrap();

    let applied = household
        .ledger
        .apply(move_to(household.groceries, Some(household.food)))
        .unwrap();

    assert_eq!(applied.inverse(), &move_to(household.groceries, None));
}

fn set_type(category: CategoryId, category_type: CategoryType) -> DomainCommand {
    DomainCommand::SetCategoryType {
        category,
        category_type,
    }
}

#[test]
fn a_top_level_categorys_new_type_is_every_subcategorys_under_it() {
    let mut household = Household::new();

    household
        .apply(set_type(household.food, CategoryType::Income))
        .unwrap();

    assert_eq!(household.type_of(household.food), CategoryType::Income);
    assert_eq!(household.type_of(household.coffee), CategoryType::Income);
    assert_eq!(household.type_of(household.salary), CategoryType::Income);
}

#[test]
fn a_subcategory_cant_be_given_a_type() {
    let mut household = Household::new();
    let before = household.view();

    let result = household.apply(set_type(household.restaurants, CategoryType::Income));

    assert_eq!(
        result,
        Err(DomainError::SubcategoryType {
            category: household.restaurants
        })
    );
    assert_eq!(household.view(), before);
}

#[test]
fn undo_gives_a_top_level_category_and_its_subcategories_their_old_type_back() {
    let mut household = Household::new();
    household
        .apply(set_type(household.food, CategoryType::Income))
        .unwrap();

    household.ledger.undo().unwrap();

    assert_eq!(household.type_of(household.food), CategoryType::Expense);
    assert_eq!(household.type_of(household.coffee), CategoryType::Expense);
}

#[test]
fn a_type_change_survives_reopening_the_data_file() {
    let mut household = Household::new();
    household
        .apply(set_type(household.food, CategoryType::Income))
        .unwrap();
    household
        .apply(rename(household.coffee, "Espresso"))
        .unwrap();
    household.apply(move_to(household.groceries, None)).unwrap();
    let path = household.dir.path().join("Household.balanced");
    drop(household.ledger);

    let reopened = Ledger::open(&path).expect("the data file opens");

    let view = reopened.snapshot();
    let category = |id| view.category(id).expect("the category is in the file");
    assert_eq!(category(household.coffee).name(), "Espresso");
    assert_eq!(
        category(household.coffee).category_type(),
        CategoryType::Income
    );
    assert_eq!(category(household.groceries).parent(), None);
}
