//! `Ledger` over a data file: what it writes is what a reopen reads back.

use std::path::{Path, PathBuf};

use balanced_core::{
    AccountGroup, AccountId, AccountKind, AccountStatus, AccountSummary, Category, CategoryId,
    CurrencyCode, DomainCommand, DomainError, Ledger, LedgerView, Money, MoneyError, MoveDirection,
    NewTransaction, SplitLine, TransactionEdit, TransactionId, Visibility,
};
use icu_calendar::{Date, Iso};
use tempfile::TempDir;

fn usd_code() -> CurrencyCode {
    CurrencyCode::new("USD").expect("USD is an ISO 4217 currency")
}

fn usd(amount: &str) -> Money {
    let amount = amount.parse().expect("test amounts are decimal numbers");
    Money::new(amount, usd_code()).expect("test amounts have two decimal places")
}

fn date(day: u8) -> Date<Iso> {
    Date::try_new_iso(2026, 9, day).expect("test dates are real dates")
}

/// A new data file in a temporary directory, open as a `Ledger`. The directory lives as
/// long as the test holds it.
fn new_file() -> (TempDir, PathBuf, Ledger) {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().join("Household.balanced");
    let ledger = Ledger::create(&path, usd_code()).expect("a new path takes a data file");
    (dir, path, ledger)
}

/// The ledger the file at `path` holds, opened afresh.
fn reopen(ledger: Ledger, path: &Path) -> Ledger {
    drop(ledger);
    Ledger::open(path).expect("the file the ledger wrote opens")
}

/// Everything a snapshot holds except its generation and its Undo and Redo state.
type Contents = (
    Vec<AccountSummary>,
    Vec<Category>,
    Money,
    Vec<Result<Money, MoneyError>>,
    Result<Money, MoneyError>,
);

fn contents(view: &LedgerView) -> Contents {
    (
        view.accounts().to_vec(),
        view.categories().to_vec(),
        view.net_worth(),
        [
            AccountGroup::Banking,
            AccountGroup::Investing,
            AccountGroup::PropertyDebt,
        ]
        .map(|group| view.group_total(group))
        .to_vec(),
        view.closed_total(),
    )
}

/// Changes a new file with `change`, then checks that reopening it gives the same
/// contents as the live ledger.
fn assert_reopens_as_changed(change: impl FnOnce(&mut Ledger)) {
    let (_dir, path, mut ledger) = new_file();
    change(&mut ledger);
    let live = contents(&ledger.snapshot());

    let reopened = reopen(ledger, &path);

    assert_eq!(contents(&reopened.snapshot()), live);
}

fn create(name: &str, kind: AccountKind, opening_balance: &str) -> DomainCommand {
    DomainCommand::CreateAccount {
        name: name.to_owned(),
        kind,
        opened: date(1),
        opening_balance: usd(opening_balance),
    }
}

/// Applies `command`, which the test expects the ledger to take.
fn apply(ledger: &mut Ledger, command: DomainCommand) {
    ledger.apply(command).expect("the test's command applies");
}

/// Creates checking accounts with `names`, each holding 10.00, in order.
fn create_checking(ledger: &mut Ledger, names: &[&str]) {
    for name in names {
        apply(ledger, create(name, AccountKind::Checking, "10.00"));
    }
}

/// The id of the account named `name`.
fn id(ledger: &Ledger, name: &str) -> AccountId {
    ledger
        .snapshot()
        .accounts()
        .iter()
        .map(AccountSummary::account)
        .find(|account| account.name() == name)
        .expect("the test created the account")
        .id()
}

fn names(ledger: &Ledger) -> Vec<String> {
    ledger
        .snapshot()
        .accounts()
        .iter()
        .map(|summary| summary.account().name().to_owned())
        .collect()
}

#[test]
fn a_created_account_is_there_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        apply(ledger, create("Everyday", AccountKind::Checking, "1200.00"));
        apply(ledger, create("House", AccountKind::Asset, "420000.00"));
    });
}

#[test]
fn a_renamed_account_keeps_its_name_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        create_checking(ledger, &["Everyday"]);
        let account = id(ledger, "Everyday");
        apply(
            ledger,
            DomainCommand::RenameAccount {
                account,
                name: "Joint".to_owned(),
            },
        );
    });
}

#[test]
fn a_hidden_account_stays_hidden_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        create_checking(ledger, &["Everyday"]);
        let account = id(ledger, "Everyday");
        apply(
            ledger,
            DomainCommand::SetAccountVisibility {
                account,
                visibility: Visibility::Hidden,
            },
        );
    });
}

#[test]
fn a_closed_account_stays_closed_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        create_checking(ledger, &["Everyday", "Old"]);
        let account = id(ledger, "Old");
        apply(
            ledger,
            DomainCommand::SetAccountStatus {
                account,
                status: AccountStatus::Closed,
            },
        );
    });
}

#[test]
fn a_moved_account_keeps_its_place_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        create_checking(ledger, &["First", "Second", "Third"]);
        let account = id(ledger, "Third");
        apply(
            ledger,
            DomainCommand::MoveAccount {
                account,
                direction: MoveDirection::Up,
            },
        );
        assert_eq!(names(ledger), ["First", "Third", "Second"]);
    });
}

#[test]
fn an_account_created_after_a_delete_lands_last_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        create_checking(ledger, &["First", "Second", "Third"]);
        let account = id(ledger, "Second");
        apply(ledger, DomainCommand::DeleteAccount { account });
        create_checking(ledger, &["Fourth"]);
        assert_eq!(names(ledger), ["First", "Third", "Fourth"]);
    });
}

#[test]
fn an_undeleted_account_is_back_in_its_place_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        create_checking(ledger, &["First", "Second", "Third"]);
        let account = id(ledger, "Second");
        apply(ledger, DomainCommand::DeleteAccount { account });
        ledger.undo().expect("the delete undoes");
        assert_eq!(names(ledger), ["First", "Second", "Third"]);
    });
}

#[test]
fn a_redone_change_is_there_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        create_checking(ledger, &["Everyday"]);
        ledger.undo().expect("the create undoes");
        ledger.redo().expect("the create redoes");
    });
}

#[test]
fn a_compound_of_account_changes_is_there_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        create_checking(ledger, &["Everyday"]);
        let account = id(ledger, "Everyday");
        apply(
            ledger,
            DomainCommand::Compound(vec![
                create("Savings", AccountKind::Savings, "500.00"),
                DomainCommand::RenameAccount {
                    account,
                    name: "Joint".to_owned(),
                },
            ]),
        );
    });
}

#[test]
fn a_refused_compound_leaves_the_file_as_it_was() {
    let (_dir, path, mut ledger) = new_file();
    create_checking(&mut ledger, &["Everyday"]);
    let before = contents(&ledger.snapshot());
    let account = id(&ledger, "Everyday");

    let result = ledger.apply(DomainCommand::Compound(vec![
        create("Savings", AccountKind::Savings, "500.00"),
        DomainCommand::RenameAccount {
            account,
            name: " ".to_owned(),
        },
    ]));

    assert_eq!(result, Err(DomainError::BlankAccountName));
    let reopened = reopen(ledger, &path);
    assert_eq!(contents(&reopened.snapshot()), before);
}

#[test]
fn a_write_the_file_refuses_leaves_memory_undo_and_the_file_as_they_were() {
    let (_dir, path, mut ledger) = new_file();
    create_checking(&mut ledger, &["Everyday"]);
    let before = ledger.snapshot();
    // 10^17 dollars is 10^19 cents, more than an i64 holds.
    let too_large = create("Vault", AccountKind::Asset, "100000000000000000.00");

    let result = ledger.apply(DomainCommand::Compound(vec![
        create("Savings", AccountKind::Savings, "500.00"),
        too_large,
    ]));

    assert!(matches!(result, Err(DomainError::Store(_))), "{result:?}");
    assert_eq!(ledger.snapshot(), before);
    ledger.undo().expect("the create still undoes");
    assert_eq!(names(&ledger), Vec::<String>::new());
    let reopened = reopen(ledger, &path);
    assert_eq!(names(&reopened), Vec::<String>::new());
}

#[test]
fn a_deleted_accounts_id_is_not_reused_after_a_reopen() {
    let (_dir, path, mut ledger) = new_file();
    create_checking(&mut ledger, &["Kept", "Deleted"]);
    let deleted = id(&ledger, "Deleted");
    apply(
        &mut ledger,
        DomainCommand::DeleteAccount { account: deleted },
    );
    let mut ledger = reopen(ledger, &path);

    create_checking(&mut ledger, &["Created"]);

    assert_ne!(id(&ledger, "Created"), deleted);
}

#[test]
fn group_totals_and_the_closed_total_are_the_same_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        apply(ledger, create("Everyday", AccountKind::Checking, "1200.00"));
        apply(ledger, create("Card", AccountKind::CreditCard, "-412.30"));
        apply(
            ledger,
            create("401(k)", AccountKind::Retirement, "88420.00"),
        );
        apply(ledger, create("House", AccountKind::Asset, "420000.00"));
        apply(
            ledger,
            create("Mortgage", AccountKind::Liability, "-310500.00"),
        );
        apply(
            ledger,
            create("Old savings", AccountKind::Savings, "250.00"),
        );
        let card = id(ledger, "Card");
        apply(
            ledger,
            DomainCommand::SetAccountVisibility {
                account: card,
                visibility: Visibility::Hidden,
            },
        );
        let old = id(ledger, "Old savings");
        apply(
            ledger,
            DomainCommand::SetAccountStatus {
                account: old,
                status: AccountStatus::Closed,
            },
        );
    });
}

/// Creates a category named `name` at the top of the tree, returning its id.
fn create_category(ledger: &mut Ledger, name: &str) -> CategoryId {
    apply(
        ledger,
        DomainCommand::CreateCategory {
            name: name.to_owned(),
            parent: None,
        },
    );
    ledger
        .snapshot()
        .categories()
        .iter()
        .find(|category| category.name() == name)
        .expect("the test created the category")
        .id()
}

#[test]
fn a_created_category_is_there_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        let food = create_category(ledger, "Food");
        apply(
            ledger,
            DomainCommand::CreateCategory {
                name: "Groceries".to_owned(),
                parent: Some(food),
            },
        );
    });
}

#[test]
fn a_deleted_category_is_gone_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        create_category(ledger, "Food");
        let fuel = create_category(ledger, "Fuel");
        apply(ledger, DomainCommand::DeleteCategory { category: fuel });
    });
}

#[test]
fn an_undeleted_category_is_back_with_its_id_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        let food = create_category(ledger, "Food");
        create_category(ledger, "Fuel");
        apply(ledger, DomainCommand::DeleteCategory { category: food });
        ledger.undo().expect("the delete undoes");
    });
}

/// A checking account, Everyday, holding 10.00, and a category, Food. Returns their ids.
fn everyday_and_food(ledger: &mut Ledger) -> (AccountId, CategoryId) {
    create_checking(ledger, &["Everyday"]);
    let food = create_category(ledger, "Food");
    (id(ledger, "Everyday"), food)
}

/// Posts `amount` on `day` to `account`, paid to `payee`, in `category`, and returns the
/// new transaction's id.
fn post(
    ledger: &mut Ledger,
    account: AccountId,
    day: u8,
    payee: &str,
    category: CategoryId,
    amount: &str,
) -> TransactionId {
    post_new(
        ledger,
        account,
        NewTransaction::new(date(day), payee, category, usd(amount)),
    )
}

/// Posts `transaction` to `account` and returns its id.
fn post_new(ledger: &mut Ledger, account: AccountId, transaction: NewTransaction) -> TransactionId {
    let applied = ledger
        .apply(DomainCommand::PostTransaction {
            account,
            transaction,
        })
        .expect("the test's transaction posts");
    let DomainCommand::DeleteTransaction { transaction } = applied.into_inverse() else {
        panic!("a post's inverse deletes the transaction");
    };
    transaction
}

#[test]
fn a_posted_transaction_is_there_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        let (everyday, food) = everyday_and_food(ledger);
        post_new(
            ledger,
            everyday,
            NewTransaction::new(date(3), "Fresh Market", food, usd("-86.42"))
                .number("1041")
                .memo("Weekly shop"),
        );
        post(ledger, everyday, 2, "Bakery", food, "-4.10");
    });
}

/// Costco, on the 12th: groceries and household goods, split between `food` and
/// `household`.
fn costco(food: CategoryId, household: CategoryId) -> NewTransaction {
    NewTransaction::split(
        date(12),
        "Costco",
        vec![
            SplitLine::new(food, "Food", usd("-142.37")),
            SplitLine::new(household, "Paper towels, detergent", usd("-63.90")),
        ],
    )
}

#[test]
fn a_posted_split_transaction_keeps_its_lines_in_order_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        let (everyday, food) = everyday_and_food(ledger);
        let household = create_category(ledger, "Household");
        post_new(ledger, everyday, costco(household, food));
    });
}

/// Everyday with three payments, on the 3rd, 5th and 7th, all in Food, and a second
/// category, Fuel. Returns the account, the payments in date order, and Fuel.
fn three_payments(ledger: &mut Ledger) -> (AccountId, [TransactionId; 3], CategoryId) {
    let (everyday, food) = everyday_and_food(ledger);
    let fuel = create_category(ledger, "Fuel");
    let payments = [
        post(ledger, everyday, 3, "Bakery", food, "-4.10"),
        post(ledger, everyday, 5, "Fresh Market", food, "-86.42"),
        post(ledger, everyday, 7, "Corner Shop", food, "-12.00"),
    ];
    (everyday, payments, fuel)
}

#[test]
fn every_edit_to_a_transaction_is_there_after_a_reopen() {
    let edits: [fn(CategoryId) -> TransactionEdit; 7] = [
        |_| TransactionEdit::Number(Some("1041".to_owned())),
        |_| TransactionEdit::Payee("Fresh Market Downtown".to_owned()),
        |_| TransactionEdit::Memo("Weekly shop".to_owned()),
        TransactionEdit::Category,
        |_| TransactionEdit::Amount(usd("-90.00")),
        // Before the 3rd, and after the 7th: each moves the payment in the register.
        |_| TransactionEdit::Date(date(2)),
        |_| TransactionEdit::Date(date(9)),
    ];
    for edit in edits {
        assert_reopens_as_changed(|ledger| {
            let (_, [_, middle, _], fuel) = three_payments(ledger);
            apply(
                ledger,
                DomainCommand::EditTransaction {
                    transaction: middle,
                    edit: edit(fuel),
                },
            );
        });
    }
}

#[test]
fn a_cleared_number_is_gone_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        let (everyday, food) = everyday_and_food(ledger);
        let rent = post_new(
            ledger,
            everyday,
            NewTransaction::new(date(3), "Landlord", food, usd("-1450.00")).number("1041"),
        );
        apply(
            ledger,
            DomainCommand::EditTransaction {
                transaction: rent,
                edit: TransactionEdit::Number(None),
            },
        );
    });
}

#[test]
fn a_deleted_transaction_is_gone_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        let (_, [_, middle, _], _) = three_payments(ledger);
        apply(
            ledger,
            DomainCommand::DeleteTransaction {
                transaction: middle,
            },
        );
    });
}

#[test]
fn a_deleted_split_transaction_is_gone_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        let (everyday, food) = everyday_and_food(ledger);
        let household = create_category(ledger, "Household");
        let split = post_new(ledger, everyday, costco(food, household));
        apply(
            ledger,
            DomainCommand::DeleteTransaction { transaction: split },
        );
        // Its lines are gone too, so their categories can go.
        apply(
            ledger,
            DomainCommand::DeleteCategory {
                category: household,
            },
        );
    });
}

#[test]
fn an_undeleted_transaction_is_back_in_its_place_after_a_reopen() {
    assert_reopens_as_changed(|ledger| {
        let (everyday, food) = everyday_and_food(ledger);
        let household = create_category(ledger, "Household");
        post(ledger, everyday, 3, "Bakery", food, "-4.10");
        let split = post_new(ledger, everyday, costco(food, household));
        post(ledger, everyday, 20, "Corner Shop", food, "-12.00");
        apply(
            ledger,
            DomainCommand::DeleteTransaction { transaction: split },
        );
        ledger.undo().expect("the delete undoes");
    });
}

/// What [`household`] made, for a test to name in its commands.
struct Household {
    everyday: AccountId,
    savings: AccountId,
    holiday: AccountId,
    payments: [TransactionId; 3],
    split: TransactionId,
    food: CategoryId,
    fuel: CategoryId,
    unused: CategoryId,
}

/// Everyday, with three payments and a split, and Savings and Holiday, with categories
/// Food, Fuel, Household and an unused one.
fn household(ledger: &mut Ledger) -> Household {
    let (everyday, payments, fuel) = three_payments(ledger);
    let food = id_of_category(ledger, "Food");
    let household = create_category(ledger, "Household");
    let unused = create_category(ledger, "Unused");
    let split = post_new(ledger, everyday, costco(food, household));
    apply(ledger, create("Savings", AccountKind::Savings, "500.00"));
    apply(ledger, create("Holiday", AccountKind::Savings, "800.00"));
    Household {
        everyday,
        savings: id(ledger, "Savings"),
        holiday: id(ledger, "Holiday"),
        payments,
        split,
        food,
        fuel,
        unused,
    }
}

fn id_of_category(ledger: &Ledger, name: &str) -> CategoryId {
    ledger
        .snapshot()
        .categories()
        .iter()
        .find(|category| category.name() == name)
        .expect("the test created the category")
        .id()
}

/// One of every Command a person sends, each against a [`household`]. The Restore
/// Commands are the inverses of the deletes, so undoing those applies them.
fn every_command(h: &Household) -> Vec<DomainCommand> {
    let edit = |edit| DomainCommand::EditTransaction {
        transaction: h.payments[1],
        edit,
    };
    vec![
        create("Visa", AccountKind::CreditCard, "-412.30"),
        DomainCommand::DeleteAccount {
            account: h.everyday,
        },
        DomainCommand::RenameAccount {
            account: h.savings,
            name: "Rainy day".to_owned(),
        },
        DomainCommand::SetAccountVisibility {
            account: h.savings,
            visibility: Visibility::Hidden,
        },
        DomainCommand::SetAccountStatus {
            account: h.savings,
            status: AccountStatus::Closed,
        },
        DomainCommand::MoveAccount {
            account: h.holiday,
            direction: MoveDirection::Up,
        },
        DomainCommand::PostTransaction {
            account: h.everyday,
            transaction: NewTransaction::new(date(4), "Shell", h.fuel, usd("-48.20")),
        },
        DomainCommand::PostTransaction {
            account: h.savings,
            transaction: costco(h.food, h.fuel),
        },
        edit(TransactionEdit::Date(date(9))),
        edit(TransactionEdit::Number(Some("1042".to_owned()))),
        edit(TransactionEdit::Payee("Fresh Market Downtown".to_owned())),
        edit(TransactionEdit::Memo("Weekly shop".to_owned())),
        edit(TransactionEdit::Category(h.fuel)),
        edit(TransactionEdit::Amount(usd("-90.00"))),
        DomainCommand::DeleteTransaction {
            transaction: h.split,
        },
        DomainCommand::CreateCategory {
            name: "Fuel".to_owned(),
            parent: Some(h.food),
        },
        DomainCommand::DeleteCategory { category: h.unused },
        DomainCommand::Compound(vec![
            DomainCommand::CreateCategory {
                name: "Coffee".to_owned(),
                parent: None,
            },
            DomainCommand::DeleteTransaction {
                transaction: h.payments[0],
            },
            DomainCommand::EditTransaction {
                transaction: h.payments[2],
                edit: TransactionEdit::Date(date(1)),
            },
            DomainCommand::DeleteCategory { category: h.unused },
            DomainCommand::DeleteAccount {
                account: h.everyday,
            },
        ]),
    ]
}

#[test]
fn every_command_undone_and_redone_is_there_after_a_reopen() {
    let count = every_command(&household(&mut new_file().2)).len();
    for index in 0..count {
        for undo_redo in [0, 1, 2] {
            assert_reopens_as_changed(|ledger| {
                let h = household(ledger);
                let command = every_command(&h).swap_remove(index);
                apply(ledger, command);
                if undo_redo >= 1 {
                    ledger.undo().expect("the command undoes");
                }
                if undo_redo == 2 {
                    ledger.redo().expect("the command redoes");
                }
            });
        }
    }
}

#[test]
fn an_undeleted_account_with_a_split_keeps_its_ids_after_a_reopen() {
    let (_dir, path, mut ledger) = new_file();
    let h = household(&mut ledger);
    let before = contents(&ledger.snapshot());
    apply(
        &mut ledger,
        DomainCommand::DeleteAccount {
            account: h.everyday,
        },
    );
    ledger.undo().expect("the delete undoes");

    let reopened = reopen(ledger, &path);

    assert_eq!(contents(&reopened.snapshot()), before);
}

#[test]
fn a_refused_compound_of_transaction_and_category_changes_leaves_the_file_as_it_was() {
    let (_dir, path, mut ledger) = new_file();
    let h = household(&mut ledger);
    let before = contents(&ledger.snapshot());

    let result = ledger.apply(DomainCommand::Compound(vec![
        DomainCommand::CreateCategory {
            name: "Coffee".to_owned(),
            parent: None,
        },
        DomainCommand::PostTransaction {
            account: h.everyday,
            transaction: costco(h.food, h.fuel),
        },
        DomainCommand::EditTransaction {
            transaction: h.payments[0],
            edit: TransactionEdit::Date(date(20)),
        },
        DomainCommand::DeleteTransaction {
            transaction: h.split,
        },
        DomainCommand::DeleteCategory { category: h.unused },
        DomainCommand::CreateCategory {
            name: "food".to_owned(),
            parent: None,
        },
    ]));

    assert_eq!(
        result,
        Err(DomainError::DuplicateCategory {
            name: "food".to_owned()
        })
    );
    let reopened = reopen(ledger, &path);
    assert_eq!(contents(&reopened.snapshot()), before);
}

#[test]
fn an_amount_beyond_the_files_range_leaves_memory_undo_and_the_file_as_they_were() {
    // 10^17 dollars is 10^19 cents, more than an i64 holds.
    let too_large = usd("100000000000000000.00");
    let refused: [fn(&Household, Money) -> DomainCommand; 3] = [
        |h, amount| DomainCommand::PostTransaction {
            account: h.everyday,
            transaction: NewTransaction::new(date(4), "Lottery", h.food, amount),
        },
        |h, amount| DomainCommand::PostTransaction {
            account: h.everyday,
            transaction: NewTransaction::split(
                date(4),
                "Lottery",
                vec![SplitLine::new(h.food, "Prize", amount)],
            ),
        },
        |h, amount| DomainCommand::EditTransaction {
            transaction: h.payments[1],
            edit: TransactionEdit::Amount(amount),
        },
    ];
    for command in refused {
        let (_dir, path, mut ledger) = new_file();
        let h = household(&mut ledger);
        let before = ledger.snapshot();

        let result = ledger.apply(DomainCommand::Compound(vec![
            DomainCommand::CreateCategory {
                name: "Coffee".to_owned(),
                parent: None,
            },
            command(&h, too_large),
        ]));

        assert!(matches!(result, Err(DomainError::Store(_))), "{result:?}");
        assert_eq!(ledger.snapshot(), before);
        ledger.undo().expect("the last change before still undoes");
        let undone = contents(&ledger.snapshot());
        let reopened = reopen(ledger, &path);
        assert_eq!(contents(&reopened.snapshot()), undone);
    }
}

#[test]
fn a_category_the_file_would_keep_is_refused_as_in_use() {
    let (_dir, path, mut ledger) = new_file();
    let h = household(&mut ledger);
    let household = id_of_category(&ledger, "Household");
    let groceries = DomainCommand::CreateCategory {
        name: "Groceries".to_owned(),
        parent: Some(h.unused),
    };
    apply(&mut ledger, groceries);
    let before = contents(&ledger.snapshot());

    // Food is in payments, Household only in a split line, and Unused has a child.
    for category in [h.food, household, h.unused] {
        let result = ledger.apply(DomainCommand::DeleteCategory { category });

        assert_eq!(result, Err(DomainError::CategoryInUse { category }));
    }
    let reopened = reopen(ledger, &path);
    assert_eq!(contents(&reopened.snapshot()), before);
}
