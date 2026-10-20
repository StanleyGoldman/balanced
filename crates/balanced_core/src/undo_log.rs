// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The undo log's entries: each a [`DomainCommand`] as JSON, through the wire types in
//! [`v1`], which `DomainCommand` itself doesn't depend on (ADR 0007).

mod v1;

use icu_calendar::{Date, Iso};

use crate::data_file::{date_text, minor_units, money};
use crate::transaction::{NewAllocation, NewTransactionParts, SplitLine, SplitTarget};
use crate::{
    Account, AccountId, AccountKind, AccountStatus, Allocation, Category, CategoryId, CurrencyCode,
    DomainCommand, MoneyError, MoveDirection, NewTransaction, Transaction, TransactionEdit,
    TransactionId, Visibility, WriteError,
};

/// Why an undo log entry couldn't be read back as a Command.
#[derive(Debug, thiserror::Error)]
pub enum UndoEntryError {
    /// The entry isn't JSON of a Command in the wire format.
    #[error("the entry isn't a Command in the undo log's format")]
    Json(#[from] serde_json::Error),
    /// A date Balanced's calendar can't hold. `reason` is ICU's, which implements
    /// `Display` but not `Error`, so it can't be a source.
    #[error("the entry holds a date Balanced can't read ({reason}): {value:?}")]
    Date {
        value: String,
        reason: icu_calendar::ParseError,
    },
    /// A split's lines add up to more than an amount holds.
    #[error("the entry holds a split whose lines overflow")]
    Money(#[from] MoneyError),
}

/// `command` as an undo log entry. Fails when an amount has more minor units than the
/// file holds.
pub(crate) fn encode(command: &DomainCommand) -> Result<String, WriteError> {
    let wire = to_wire(command)?;
    Ok(serde_json::to_string(&wire).expect("the wire types have string keys and no floats"))
}

/// The Command the undo log entry `entry` holds, its amounts in `currency`.
pub(crate) fn decode(entry: &str, currency: CurrencyCode) -> Result<DomainCommand, UndoEntryError> {
    from_wire(serde_json::from_str(entry)?, currency)
}

fn to_wire(command: &DomainCommand) -> Result<v1::Command, WriteError> {
    Ok(match command {
        DomainCommand::CreateAccount {
            name,
            kind,
            opened,
            opening_balance,
        } => v1::Command::CreateAccount {
            name: name.clone(),
            kind: kind_to_wire(*kind),
            opened: date_text(*opened),
            opening_balance: minor_units(*opening_balance)?,
        },
        DomainCommand::DeleteAccount { account } => v1::Command::DeleteAccount {
            account: account.raw(),
        },
        DomainCommand::RestoreAccount {
            account,
            transactions,
            place,
        } => v1::Command::RestoreAccount {
            account: account_to_wire(account),
            transactions: transactions
                .iter()
                .map(transaction_to_wire)
                .collect::<Result<_, _>>()?,
            place: u32::try_from(*place).expect("a sub-group holds fewer than 2^32 accounts"),
        },
        DomainCommand::RenameAccount { account, name } => v1::Command::RenameAccount {
            account: account.raw(),
            name: name.clone(),
        },
        DomainCommand::SetAccountVisibility {
            account,
            visibility,
        } => v1::Command::SetAccountVisibility {
            account: account.raw(),
            visibility: visibility_to_wire(*visibility),
        },
        DomainCommand::SetAccountStatus { account, status } => v1::Command::SetAccountStatus {
            account: account.raw(),
            status: status_to_wire(*status),
        },
        DomainCommand::MoveAccount { account, direction } => v1::Command::MoveAccount {
            account: account.raw(),
            direction: match direction {
                MoveDirection::Up => v1::MoveDirection::Up,
                MoveDirection::Down => v1::MoveDirection::Down,
            },
        },
        DomainCommand::Compound(commands) => {
            v1::Command::Compound(commands.iter().map(to_wire).collect::<Result<_, _>>()?)
        }
        DomainCommand::PostTransaction {
            account,
            transaction,
        } => v1::Command::PostTransaction {
            account: account.raw(),
            transaction: new_transaction_to_wire(transaction)?,
        },
        DomainCommand::EditTransaction { transaction, edit } => v1::Command::EditTransaction {
            transaction: transaction.raw(),
            edit: edit_to_wire(edit)?,
        },
        DomainCommand::SetFlagged {
            transaction,
            flagged,
        } => v1::Command::SetFlagged {
            transaction: transaction.raw(),
            flagged: *flagged,
        },
        DomainCommand::DeleteTransaction { transaction } => v1::Command::DeleteTransaction {
            transaction: transaction.raw(),
        },
        DomainCommand::RestoreTransaction {
            account,
            transaction,
        } => v1::Command::RestoreTransaction {
            account: account.raw(),
            transaction: transaction_to_wire(transaction)?,
        },
        DomainCommand::CreateCategory { name, parent } => v1::Command::CreateCategory {
            name: name.clone(),
            parent: parent.map(CategoryId::raw),
        },
        DomainCommand::DeleteCategory { category } => v1::Command::DeleteCategory {
            category: category.raw(),
        },
        DomainCommand::RestoreCategory { category } => v1::Command::RestoreCategory {
            category: v1::Category {
                id: category.id().raw(),
                name: category.name().to_owned(),
                parent: category.parent().map(CategoryId::raw),
            },
        },
    })
}

fn account_to_wire(account: &Account) -> v1::Account {
    v1::Account {
        id: account.id().raw(),
        name: account.name().to_owned(),
        kind: kind_to_wire(account.kind()),
        visibility: visibility_to_wire(account.visibility()),
        status: status_to_wire(account.status()),
    }
}

fn new_transaction_to_wire(transaction: &NewTransaction) -> Result<v1::NewTransaction, WriteError> {
    let NewTransactionParts {
        date,
        number,
        payee,
        memo,
        allocation,
    } = transaction.parts();
    Ok(v1::NewTransaction {
        date: date_text(date),
        number: number.map(str::to_owned),
        payee: payee.to_owned(),
        memo: memo.to_owned(),
        allocation: match allocation {
            NewAllocation::OpeningBalance(amount) => v1::Allocation::OpeningBalance {
                amount: minor_units(*amount)?,
            },
            NewAllocation::Category(category, amount) => v1::Allocation::Category {
                category: category.raw(),
                amount: minor_units(*amount)?,
            },
            NewAllocation::Split(lines) => split_to_wire(lines)?,
        },
    })
}

fn edit_to_wire(edit: &TransactionEdit) -> Result<v1::TransactionEdit, WriteError> {
    Ok(match edit {
        TransactionEdit::Date(date) => v1::TransactionEdit::Date(date_text(*date)),
        TransactionEdit::Number(number) => v1::TransactionEdit::Number(number.clone()),
        TransactionEdit::Payee(payee) => v1::TransactionEdit::Payee(payee.clone()),
        TransactionEdit::Memo(memo) => v1::TransactionEdit::Memo(memo.clone()),
        TransactionEdit::Category(category) => v1::TransactionEdit::Category(category.raw()),
        TransactionEdit::Amount(amount) => v1::TransactionEdit::Amount(minor_units(*amount)?),
    })
}

fn transaction_to_wire(transaction: &Transaction) -> Result<v1::Transaction, WriteError> {
    let amount = || minor_units(transaction.amount());
    Ok(v1::Transaction {
        id: transaction.id().raw(),
        fields: v1::NewTransaction {
            date: date_text(transaction.date()),
            number: transaction.number().map(str::to_owned),
            payee: transaction.payee().to_owned(),
            memo: transaction.memo().to_owned(),
            allocation: match transaction.allocation() {
                Allocation::OpeningBalance => v1::Allocation::OpeningBalance { amount: amount()? },
                Allocation::Category(category) => v1::Allocation::Category {
                    category: category.raw(),
                    amount: amount()?,
                },
                Allocation::Split(lines) => split_to_wire(lines)?,
            },
        },
        flagged: transaction.is_flagged(),
    })
}

fn split_to_wire(lines: &[SplitLine]) -> Result<v1::Allocation, WriteError> {
    Ok(v1::Allocation::Split(
        lines
            .iter()
            .map(|line| {
                Ok(v1::SplitLine {
                    target: match line.target() {
                        SplitTarget::Category(category) => {
                            v1::SplitTarget::Category(category.raw())
                        }
                    },
                    memo: line.memo().to_owned(),
                    amount: minor_units(line.amount())?,
                })
            })
            .collect::<Result<_, WriteError>>()?,
    ))
}

fn kind_to_wire(kind: AccountKind) -> v1::AccountKind {
    match kind {
        AccountKind::Checking => v1::AccountKind::Checking,
        AccountKind::Savings => v1::AccountKind::Savings,
        AccountKind::CreditCard => v1::AccountKind::CreditCard,
        AccountKind::Investment => v1::AccountKind::Investment,
        AccountKind::Retirement => v1::AccountKind::Retirement,
        AccountKind::Asset => v1::AccountKind::Asset,
        AccountKind::Liability => v1::AccountKind::Liability,
    }
}

fn visibility_to_wire(visibility: Visibility) -> v1::Visibility {
    match visibility {
        Visibility::Shown => v1::Visibility::Shown,
        Visibility::Hidden => v1::Visibility::Hidden,
    }
}

fn status_to_wire(status: AccountStatus) -> v1::AccountStatus {
    match status {
        AccountStatus::Open => v1::AccountStatus::Open,
        AccountStatus::Closed => v1::AccountStatus::Closed,
    }
}

fn from_wire(
    command: v1::Command,
    currency: CurrencyCode,
) -> Result<DomainCommand, UndoEntryError> {
    Ok(match command {
        v1::Command::CreateAccount {
            name,
            kind,
            opened,
            opening_balance,
        } => DomainCommand::CreateAccount {
            name,
            kind: kind_from_wire(kind),
            opened: date(&opened)?,
            opening_balance: money(opening_balance, currency),
        },
        v1::Command::DeleteAccount { account } => DomainCommand::DeleteAccount {
            account: AccountId::new(account),
        },
        v1::Command::RestoreAccount {
            account,
            transactions,
            place,
        } => DomainCommand::RestoreAccount {
            account: account_from_wire(account, currency),
            transactions: transactions
                .into_iter()
                .map(|transaction| transaction_from_wire(transaction, currency))
                .collect::<Result<_, _>>()?,
            place: usize::try_from(place).expect("a usize holds any u32"),
        },
        v1::Command::RenameAccount { account, name } => DomainCommand::RenameAccount {
            account: AccountId::new(account),
            name,
        },
        v1::Command::SetAccountVisibility {
            account,
            visibility,
        } => DomainCommand::SetAccountVisibility {
            account: AccountId::new(account),
            visibility: visibility_from_wire(visibility),
        },
        v1::Command::SetAccountStatus { account, status } => DomainCommand::SetAccountStatus {
            account: AccountId::new(account),
            status: status_from_wire(status),
        },
        v1::Command::MoveAccount { account, direction } => DomainCommand::MoveAccount {
            account: AccountId::new(account),
            direction: match direction {
                v1::MoveDirection::Up => MoveDirection::Up,
                v1::MoveDirection::Down => MoveDirection::Down,
            },
        },
        v1::Command::Compound(commands) => DomainCommand::Compound(
            commands
                .into_iter()
                .map(|command| from_wire(command, currency))
                .collect::<Result<_, _>>()?,
        ),
        v1::Command::PostTransaction {
            account,
            transaction,
        } => DomainCommand::PostTransaction {
            account: AccountId::new(account),
            transaction: new_transaction_from_wire(transaction, currency)?,
        },
        v1::Command::EditTransaction { transaction, edit } => DomainCommand::EditTransaction {
            transaction: TransactionId::new(transaction),
            edit: edit_from_wire(edit, currency)?,
        },
        v1::Command::SetFlagged {
            transaction,
            flagged,
        } => DomainCommand::SetFlagged {
            transaction: TransactionId::new(transaction),
            flagged,
        },
        v1::Command::DeleteTransaction { transaction } => DomainCommand::DeleteTransaction {
            transaction: TransactionId::new(transaction),
        },
        v1::Command::RestoreTransaction {
            account,
            transaction,
        } => DomainCommand::RestoreTransaction {
            account: AccountId::new(account),
            transaction: transaction_from_wire(transaction, currency)?,
        },
        v1::Command::CreateCategory { name, parent } => DomainCommand::CreateCategory {
            name,
            parent: parent.map(CategoryId::new),
        },
        v1::Command::DeleteCategory { category } => DomainCommand::DeleteCategory {
            category: CategoryId::new(category),
        },
        v1::Command::RestoreCategory { category } => DomainCommand::RestoreCategory {
            category: Category::new(
                CategoryId::new(category.id),
                category.name,
                category.parent.map(CategoryId::new),
            ),
        },
    })
}

fn account_from_wire(account: v1::Account, currency: CurrencyCode) -> Account {
    Account::new(
        AccountId::new(account.id),
        account.name,
        kind_from_wire(account.kind),
        currency,
        visibility_from_wire(account.visibility),
        status_from_wire(account.status),
    )
}

fn new_transaction_from_wire(
    transaction: v1::NewTransaction,
    currency: CurrencyCode,
) -> Result<NewTransaction, UndoEntryError> {
    let allocation = match transaction.allocation {
        v1::Allocation::OpeningBalance { amount } => {
            NewAllocation::OpeningBalance(money(amount, currency))
        }
        v1::Allocation::Category { category, amount } => {
            NewAllocation::Category(CategoryId::new(category), money(amount, currency))
        }
        v1::Allocation::Split(lines) => NewAllocation::Split(split_from_wire(lines, currency)),
    };
    Ok(NewTransaction::from_parts(
        date(&transaction.date)?,
        transaction.number,
        transaction.payee,
        transaction.memo,
        allocation,
    ))
}

fn edit_from_wire(
    edit: v1::TransactionEdit,
    currency: CurrencyCode,
) -> Result<TransactionEdit, UndoEntryError> {
    Ok(match edit {
        v1::TransactionEdit::Date(text) => TransactionEdit::Date(date(&text)?),
        v1::TransactionEdit::Number(number) => TransactionEdit::Number(number),
        v1::TransactionEdit::Payee(payee) => TransactionEdit::Payee(payee),
        v1::TransactionEdit::Memo(memo) => TransactionEdit::Memo(memo),
        v1::TransactionEdit::Category(category) => {
            TransactionEdit::Category(CategoryId::new(category))
        }
        v1::TransactionEdit::Amount(amount) => TransactionEdit::Amount(money(amount, currency)),
    })
}

/// The transaction `transaction` holds. A split's amount is the sum of its lines.
fn transaction_from_wire(
    transaction: v1::Transaction,
    currency: CurrencyCode,
) -> Result<Transaction, UndoEntryError> {
    Ok(new_transaction_from_wire(transaction.fields, currency)?
        .into_transaction(TransactionId::new(transaction.id), currency)?
        .with_flag(transaction.flagged))
}

fn split_from_wire(lines: Vec<v1::SplitLine>, currency: CurrencyCode) -> Vec<SplitLine> {
    lines
        .into_iter()
        .map(|line| {
            SplitLine::new(
                match line.target {
                    v1::SplitTarget::Category(category) => {
                        SplitTarget::Category(CategoryId::new(category))
                    }
                },
                &line.memo,
                money(line.amount, currency),
            )
        })
        .collect()
}

fn kind_from_wire(kind: v1::AccountKind) -> AccountKind {
    match kind {
        v1::AccountKind::Checking => AccountKind::Checking,
        v1::AccountKind::Savings => AccountKind::Savings,
        v1::AccountKind::CreditCard => AccountKind::CreditCard,
        v1::AccountKind::Investment => AccountKind::Investment,
        v1::AccountKind::Retirement => AccountKind::Retirement,
        v1::AccountKind::Asset => AccountKind::Asset,
        v1::AccountKind::Liability => AccountKind::Liability,
    }
}

fn visibility_from_wire(visibility: v1::Visibility) -> Visibility {
    match visibility {
        v1::Visibility::Shown => Visibility::Shown,
        v1::Visibility::Hidden => Visibility::Hidden,
    }
}

fn status_from_wire(status: v1::AccountStatus) -> AccountStatus {
    match status {
        v1::AccountStatus::Open => AccountStatus::Open,
        v1::AccountStatus::Closed => AccountStatus::Closed,
    }
}

fn date(text: &str) -> Result<Date<Iso>, UndoEntryError> {
    Date::try_from_str(text, Iso).map_err(|reason| UndoEntryError::Date {
        value: text.to_owned(),
        reason,
    })
}

#[cfg(test)]
mod tests {
    use icu_calendar::{Date, Iso};
    use rust_decimal_macros::dec;

    use super::*;
    use crate::transaction::{SplitLine, SplitTarget};
    use crate::{
        Account, AccountId, AccountKind, AccountStatus, Allocation, Category, CategoryId, Money,
        MoveDirection, NewTransaction, Transaction, TransactionEdit, TransactionId, Visibility,
    };

    fn usd_code() -> CurrencyCode {
        CurrencyCode::new("USD").expect("USD is an ISO 4217 currency")
    }

    fn usd(amount: rust_decimal::Decimal) -> Money {
        Money::new(amount, usd_code()).expect("test amounts have two decimal places")
    }

    fn date(day: u8) -> Date<Iso> {
        Date::try_new_iso(2026, 9, day).expect("test dates are real dates")
    }

    fn costco() -> Allocation {
        Allocation::Split(vec![
            SplitLine::new(
                SplitTarget::Category(CategoryId::new(3)),
                "Groceries",
                usd(dec!(-80.00)),
            ),
            SplitLine::new(
                SplitTarget::Category(CategoryId::new(4)),
                "",
                usd(dec!(-20.50)),
            ),
        ])
    }

    /// An opening balance, a split with a number and a memo, and a payment in one
    /// category: Shell, with id 12.
    fn transactions() -> [Transaction; 3] {
        let restored = Transaction::from_parts(
            TransactionId::new(13),
            date(2),
            Some("1042".to_owned()),
            "Costco".to_owned(),
            "Weekly shop".to_owned(),
            costco(),
            usd(dec!(-100.50)),
        );
        let opening = Transaction::from_parts(
            TransactionId::new(11),
            date(1),
            None,
            String::new(),
            String::new(),
            Allocation::OpeningBalance,
            usd(dec!(1200.00)),
        );
        let paid = Transaction::from_parts(
            TransactionId::new(12),
            date(3),
            None,
            "Shell".to_owned(),
            String::new(),
            Allocation::Category(CategoryId::new(5)),
            usd(dec!(-48.20)),
        );
        [opening, restored, paid]
    }

    /// A payment in one category with a number and a memo, a split, and an opening
    /// balance.
    fn new_transactions() -> [NewTransaction; 3] {
        [
            NewTransaction::new(date(4), "Shell", CategoryId::new(5), usd(dec!(-48.20)))
                .number("1043")
                .memo("Fill up"),
            NewTransaction::split(
                date(4),
                "Costco",
                vec![SplitLine::new(
                    SplitTarget::Category(CategoryId::new(3)),
                    "Groceries",
                    usd(dec!(-80.00)),
                )],
            ),
            NewTransaction::opening_balance(date(1), usd(dec!(0.00))),
        ]
    }

    /// One of every Command, every field set and unset, nested in a Compound too.
    fn every_command() -> Vec<DomainCommand> {
        let account = AccountId::new(7);
        let transaction = TransactionId::new(12);
        let edit = |edit| DomainCommand::EditTransaction { transaction, edit };
        let [opening, restored, paid] = transactions();
        let [shell, split, opening_balance] = new_transactions();
        let rename = DomainCommand::RenameAccount {
            account,
            name: "Joint".to_owned(),
        };
        vec![
            DomainCommand::CreateAccount {
                name: "Everyday".to_owned(),
                kind: AccountKind::CreditCard,
                opened: date(1),
                opening_balance: usd(dec!(-412.30)),
            },
            DomainCommand::DeleteAccount { account },
            DomainCommand::RestoreAccount {
                account: Account::new(
                    account,
                    "Everyday".to_owned(),
                    AccountKind::Checking,
                    usd_code(),
                    Visibility::Hidden,
                    AccountStatus::Closed,
                ),
                transactions: vec![opening, restored.clone(), paid],
                place: 2,
            },
            rename.clone(),
            DomainCommand::SetAccountVisibility {
                account,
                visibility: Visibility::Shown,
            },
            DomainCommand::SetAccountStatus {
                account,
                status: AccountStatus::Open,
            },
            DomainCommand::MoveAccount {
                account,
                direction: MoveDirection::Up,
            },
            DomainCommand::MoveAccount {
                account,
                direction: MoveDirection::Down,
            },
            DomainCommand::Compound(vec![
                rename,
                DomainCommand::Compound(vec![DomainCommand::DeleteTransaction { transaction }]),
                DomainCommand::Compound(Vec::new()),
            ]),
            DomainCommand::PostTransaction {
                account,
                transaction: shell,
            },
            DomainCommand::PostTransaction {
                account,
                transaction: split,
            },
            DomainCommand::PostTransaction {
                account,
                transaction: opening_balance,
            },
            edit(TransactionEdit::Date(date(9))),
            edit(TransactionEdit::Number(Some("1044".to_owned()))),
            edit(TransactionEdit::Number(None)),
            edit(TransactionEdit::Payee("Fresh Market".to_owned())),
            edit(TransactionEdit::Memo("Weekly shop".to_owned())),
            edit(TransactionEdit::Category(CategoryId::new(6))),
            edit(TransactionEdit::Amount(usd(dec!(-90.00)))),
            DomainCommand::SetFlagged {
                transaction,
                flagged: true,
            },
            DomainCommand::SetFlagged {
                transaction,
                flagged: false,
            },
            DomainCommand::DeleteTransaction { transaction },
            DomainCommand::RestoreTransaction {
                account,
                transaction: restored.clone(),
            },
            DomainCommand::RestoreTransaction {
                account,
                transaction: restored.with_flag(true),
            },
        ]
        .into_iter()
        .chain(category_commands())
        .collect()
    }

    /// One of every category Command, every field set and unset.
    fn category_commands() -> [DomainCommand; 4] {
        [
            DomainCommand::CreateCategory {
                name: "Fuel".to_owned(),
                parent: Some(CategoryId::new(5)),
            },
            DomainCommand::CreateCategory {
                name: "Coffee".to_owned(),
                parent: None,
            },
            DomainCommand::DeleteCategory {
                category: CategoryId::new(6),
            },
            DomainCommand::RestoreCategory {
                category: Category::new(CategoryId::new(6), "Fuel".to_owned(), None),
            },
        ]
    }

    /// The place of `command`'s variant in the declaration of [`DomainCommand`]. A new
    /// variant fails to compile here until it is added, and then fails
    /// `every_command_covers_every_variant` until [`every_command`] holds it.
    fn variant(command: &DomainCommand) -> usize {
        match command {
            DomainCommand::CreateAccount { .. } => 0,
            DomainCommand::DeleteAccount { .. } => 1,
            DomainCommand::RestoreAccount { .. } => 2,
            DomainCommand::RenameAccount { .. } => 3,
            DomainCommand::SetAccountVisibility { .. } => 4,
            DomainCommand::SetAccountStatus { .. } => 5,
            DomainCommand::MoveAccount { .. } => 6,
            DomainCommand::Compound(_) => 7,
            DomainCommand::PostTransaction { .. } => 8,
            DomainCommand::EditTransaction { .. } => 9,
            DomainCommand::SetFlagged { .. } => 10,
            DomainCommand::DeleteTransaction { .. } => 11,
            DomainCommand::RestoreTransaction { .. } => 12,
            DomainCommand::CreateCategory { .. } => 13,
            DomainCommand::DeleteCategory { .. } => 14,
            DomainCommand::RestoreCategory { .. } => 15,
        }
    }

    #[test]
    fn every_command_covers_every_variant() {
        let mut seen: Vec<usize> = every_command().iter().map(variant).collect();
        seen.dedup();

        assert_eq!(seen, (0..16).collect::<Vec<_>>());
    }

    #[test]
    fn every_command_reads_back_as_it_was_written() {
        for command in every_command() {
            let entry = encode(&command).unwrap();

            assert_eq!(decode(&entry, usd_code()).unwrap(), command, "{entry}");
        }
    }

    #[test]
    fn the_wire_format_of_every_command_is_frozen() {
        let entries = every_command()
            .iter()
            .map(|command| encode(command).unwrap())
            .collect::<Vec<_>>()
            .join("\n");

        insta::assert_snapshot!(entries);
    }
}
