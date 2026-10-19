//! `Ledger` over a data file: what it writes is what a reopen reads back.

use std::path::{Path, PathBuf};

use balanced_core::{
    AccountGroup, AccountId, AccountKind, AccountStatus, AccountSummary, Category, CurrencyCode,
    DomainCommand, DomainError, Ledger, LedgerView, Money, MoneyError, MoveDirection, Visibility,
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
