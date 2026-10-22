// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The undo log's entries: each a [`DomainCommand`] as JSON, through the wire types in
//! [`v1`], which `DomainCommand` itself doesn't depend on (ADR 0007).

mod v1;

use icu_calendar::{Date, Iso};

use crate::data_file::{date_text, minor_units, money};
use crate::transaction::{
    EditedSplitLine, EditedSplitTarget, NewAllocation, NewSplitLine, NewSplitTarget,
    NewTransactionParts, SplitLine, SplitTarget,
};
use crate::{
    Account, AccountId, AccountKind, AccountStatus, Allocation, Category, CategoryId, CategoryType,
    CategoryUse, ClearedStatus, CurrencyCode, DomainCommand, Money, MoneyError, MoveDirection,
    NewTransaction, OtherSide, Transaction, TransactionEdit, TransactionId, Visibility, WriteError,
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

#[expect(
    clippy::too_many_lines,
    reason = "one arm per Command, each mapping its fields one by one"
)]
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
            other_sides,
        } => v1::Command::RestoreTransaction {
            account: account.raw(),
            transaction: transaction_to_wire(transaction)?,
            other_sides: other_sides
                .iter()
                .map(other_side_to_wire)
                .collect::<Result<_, _>>()?,
        },
        DomainCommand::CreateCategory {
            name,
            parent,
            category_type,
        } => v1::Command::CreateCategory {
            name: name.clone(),
            parent: parent.map(CategoryId::raw),
            category_type: category_type_to_wire(*category_type),
        },
        DomainCommand::DeleteCategory { category } => v1::Command::DeleteCategory {
            category: category.raw(),
        },
        DomainCommand::RestoreCategory { category } => v1::Command::RestoreCategory {
            category: category_to_wire(category),
        },
        DomainCommand::RenameCategory { category, name } => v1::Command::RenameCategory {
            category: category.raw(),
            name: name.clone(),
        },
        DomainCommand::MoveCategory { category, parent } => v1::Command::MoveCategory {
            category: category.raw(),
            parent: parent.map(CategoryId::raw),
        },
        DomainCommand::SetCategoryType {
            category,
            category_type,
        } => v1::Command::SetCategoryType {
            category: category.raw(),
            category_type: category_type_to_wire(*category_type),
        },
        DomainCommand::ReassignCategory { from, to } => v1::Command::ReassignCategory {
            from: from.raw(),
            to: to.raw(),
        },
        DomainCommand::ReassignUses { from, to, uses } => v1::Command::ReassignUses {
            from: from.raw(),
            to: to.raw(),
            uses: uses.iter().copied().map(category_use_to_wire).collect(),
        },
    })
}

fn category_use_to_wire(category_use: CategoryUse) -> v1::CategoryUse {
    match category_use {
        CategoryUse::Transaction(transaction) => v1::CategoryUse::Transaction(transaction.raw()),
        CategoryUse::SplitLine { split, line } => v1::CategoryUse::SplitLine {
            split: split.raw(),
            line: u32::try_from(line).expect("a split holds fewer than 2^32 lines"),
        },
    }
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
            NewAllocation::Split(lines) => v1::Allocation::Split(
                lines
                    .iter()
                    .map(|line| {
                        let target = match line.target() {
                            NewSplitTarget::Category(category) => {
                                v1::SplitTarget::Category(category.raw())
                            }
                            NewSplitTarget::Transfer(account) => {
                                v1::SplitTarget::Transfer(v1::LineTo {
                                    account: account.raw(),
                                })
                            }
                        };
                        line_to_wire(target, line.memo(), line.amount())
                    })
                    .collect::<Result<_, _>>()?,
            ),
            NewAllocation::Transfer(account, amount) => v1::Allocation::Transfer(v1::TransferTo {
                account: account.raw(),
                amount: minor_units(*amount)?,
            }),
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
        TransactionEdit::Transfer(account) => v1::TransactionEdit::Transfer(account.raw()),
        TransactionEdit::Amount(amount) => v1::TransactionEdit::Amount(minor_units(*amount)?),
        TransactionEdit::ClearedStatus(status) => {
            v1::TransactionEdit::ClearedStatus(cleared_status_to_wire(*status))
        }
        TransactionEdit::Lines { amount, lines } => v1::TransactionEdit::Lines {
            amount: minor_units(*amount)?,
            lines: lines
                .iter()
                .map(|line| {
                    let target = match line.target() {
                        EditedSplitTarget::Category(category) => {
                            v1::SplitTarget::Category(category.raw())
                        }
                        EditedSplitTarget::Transfer(account) => {
                            v1::SplitTarget::Transfer(v1::EditedLineTransfer::New(v1::LineTo {
                                account: account.raw(),
                            }))
                        }
                        EditedSplitTarget::Kept(other_side) => {
                            v1::SplitTarget::Transfer(v1::EditedLineTransfer::Kept(v1::LineSide {
                                other_side: other_side.raw(),
                            }))
                        }
                    };
                    line_to_wire(target, line.memo(), line.amount())
                })
                .collect::<Result<_, _>>()?,
        },
    })
}

fn cleared_status_to_wire(status: ClearedStatus) -> v1::ClearedStatus {
    match status {
        ClearedStatus::Uncleared => v1::ClearedStatus::Uncleared,
        ClearedStatus::Cleared => v1::ClearedStatus::Cleared,
        ClearedStatus::Reconciled => v1::ClearedStatus::Reconciled,
    }
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
                Allocation::Split(lines) => v1::Allocation::Split(
                    lines
                        .iter()
                        .map(|line| {
                            let target = match line.target() {
                                SplitTarget::Category(category) => {
                                    v1::SplitTarget::Category(category.raw())
                                }
                                SplitTarget::Transfer(other_side) => {
                                    v1::SplitTarget::Transfer(v1::LineSide {
                                        other_side: other_side.raw(),
                                    })
                                }
                            };
                            line_to_wire(target, line.memo(), line.amount())
                        })
                        .collect::<Result<_, _>>()?,
                ),
                Allocation::Transfer(other_side) => v1::Allocation::Transfer(v1::TransferSide {
                    other_side: other_side.raw(),
                    amount: amount()?,
                }),
            },
        },
        flagged: transaction.is_flagged(),
        cleared_status: cleared_status_to_wire(transaction.cleared_status()),
    })
}

fn other_side_to_wire(other_side: &OtherSide) -> Result<v1::OtherSide, WriteError> {
    Ok(v1::OtherSide {
        account: other_side.account().raw(),
        transaction: transaction_to_wire(other_side.transaction())?,
    })
}

/// A split line going to `target`, on the wire.
fn line_to_wire<L>(
    target: v1::SplitTarget<L>,
    memo: &str,
    amount: Money,
) -> Result<v1::SplitLine<L>, WriteError> {
    Ok(v1::SplitLine {
        target,
        memo: memo.to_owned(),
        amount: minor_units(amount)?,
    })
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

fn category_to_wire(category: &Category) -> v1::Category {
    v1::Category {
        id: category.id().raw(),
        name: category.name().to_owned(),
        parent: category.parent().map(CategoryId::raw),
        category_type: category_type_to_wire(category.category_type()),
    }
}

fn category_type_to_wire(category_type: CategoryType) -> v1::CategoryType {
    match category_type {
        CategoryType::Income => v1::CategoryType::Income,
        CategoryType::Expense => v1::CategoryType::Expense,
    }
}

fn status_to_wire(status: AccountStatus) -> v1::AccountStatus {
    match status {
        AccountStatus::Open => v1::AccountStatus::Open,
        AccountStatus::Closed => v1::AccountStatus::Closed,
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per Command, each mapping its fields one by one"
)]
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
            other_sides,
        } => DomainCommand::RestoreTransaction {
            account: AccountId::new(account),
            transaction: transaction_from_wire(transaction, currency)?,
            other_sides: other_sides
                .into_iter()
                .map(|other_side| other_side_from_wire(other_side, currency))
                .collect::<Result<_, _>>()?,
        },
        v1::Command::CreateCategory {
            name,
            parent,
            category_type,
        } => DomainCommand::CreateCategory {
            name,
            parent: parent.map(CategoryId::new),
            category_type: category_type_from_wire(category_type),
        },
        v1::Command::DeleteCategory { category } => DomainCommand::DeleteCategory {
            category: CategoryId::new(category),
        },
        v1::Command::RestoreCategory { category } => DomainCommand::RestoreCategory {
            category: category_from_wire(category),
        },
        v1::Command::RenameCategory { category, name } => DomainCommand::RenameCategory {
            category: CategoryId::new(category),
            name,
        },
        v1::Command::MoveCategory { category, parent } => DomainCommand::MoveCategory {
            category: CategoryId::new(category),
            parent: parent.map(CategoryId::new),
        },
        v1::Command::SetCategoryType {
            category,
            category_type,
        } => DomainCommand::SetCategoryType {
            category: CategoryId::new(category),
            category_type: category_type_from_wire(category_type),
        },
        v1::Command::ReassignCategory { from, to } => DomainCommand::ReassignCategory {
            from: CategoryId::new(from),
            to: CategoryId::new(to),
        },
        v1::Command::ReassignUses { from, to, uses } => DomainCommand::ReassignUses {
            from: CategoryId::new(from),
            to: CategoryId::new(to),
            uses: uses.into_iter().map(category_use_from_wire).collect(),
        },
    })
}

fn category_use_from_wire(category_use: v1::CategoryUse) -> CategoryUse {
    match category_use {
        v1::CategoryUse::Transaction(transaction) => {
            CategoryUse::Transaction(TransactionId::new(transaction))
        }
        v1::CategoryUse::SplitLine { split, line } => CategoryUse::SplitLine {
            split: TransactionId::new(split),
            line: usize::try_from(line).expect("a usize holds every u32 Balanced runs on"),
        },
    }
}

fn category_from_wire(category: v1::Category) -> Category {
    Category::new(
        CategoryId::new(category.id),
        category.name,
        category.parent.map(CategoryId::new),
        category_type_from_wire(category.category_type),
    )
}

fn category_type_from_wire(category_type: v1::CategoryType) -> CategoryType {
    match category_type {
        v1::CategoryType::Income => CategoryType::Income,
        v1::CategoryType::Expense => CategoryType::Expense,
    }
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
        v1::Allocation::Split(lines) => NewAllocation::Split(
            lines
                .into_iter()
                .map(|line| {
                    let target = match line.target {
                        v1::SplitTarget::Category(category) => {
                            NewSplitTarget::Category(CategoryId::new(category))
                        }
                        v1::SplitTarget::Transfer(to) => {
                            NewSplitTarget::Transfer(AccountId::new(to.account))
                        }
                    };
                    NewSplitLine::new(target, &line.memo, money(line.amount, currency))
                })
                .collect(),
        ),
        v1::Allocation::Transfer(to) => {
            NewAllocation::Transfer(AccountId::new(to.account), money(to.amount, currency))
        }
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
        v1::TransactionEdit::Transfer(account) => {
            TransactionEdit::Transfer(AccountId::new(account))
        }
        v1::TransactionEdit::Amount(amount) => TransactionEdit::Amount(money(amount, currency)),
        v1::TransactionEdit::ClearedStatus(status) => {
            TransactionEdit::ClearedStatus(cleared_status_from_wire(status))
        }
        v1::TransactionEdit::Lines { amount, lines } => TransactionEdit::Lines {
            amount: money(amount, currency),
            lines: lines
                .into_iter()
                .map(|line| {
                    let target = match line.target {
                        v1::SplitTarget::Category(category) => {
                            EditedSplitTarget::Category(CategoryId::new(category))
                        }
                        v1::SplitTarget::Transfer(v1::EditedLineTransfer::New(to)) => {
                            EditedSplitTarget::Transfer(AccountId::new(to.account))
                        }
                        v1::SplitTarget::Transfer(v1::EditedLineTransfer::Kept(side)) => {
                            EditedSplitTarget::Kept(TransactionId::new(side.other_side))
                        }
                    };
                    EditedSplitLine::new(target, &line.memo, money(line.amount, currency))
                })
                .collect(),
        },
    })
}

fn cleared_status_from_wire(status: v1::ClearedStatus) -> ClearedStatus {
    match status {
        v1::ClearedStatus::Uncleared => ClearedStatus::Uncleared,
        v1::ClearedStatus::Cleared => ClearedStatus::Cleared,
        v1::ClearedStatus::Reconciled => ClearedStatus::Reconciled,
    }
}

/// The transaction `transaction` holds. A split's amount is the sum of its lines.
fn transaction_from_wire(
    transaction: v1::Transaction,
    currency: CurrencyCode,
) -> Result<Transaction, UndoEntryError> {
    let v1::NewTransaction {
        date: date_text,
        number,
        payee,
        memo,
        allocation,
    } = transaction.fields;
    let (allocation, amount) = match allocation {
        v1::Allocation::OpeningBalance { amount } => {
            (Allocation::OpeningBalance, money(amount, currency))
        }
        v1::Allocation::Category { category, amount } => (
            Allocation::Category(CategoryId::new(category)),
            money(amount, currency),
        ),
        v1::Allocation::Split(lines) => {
            let lines: Vec<SplitLine> = lines
                .into_iter()
                .map(|line| {
                    let target = match line.target {
                        v1::SplitTarget::Category(category) => {
                            SplitTarget::Category(CategoryId::new(category))
                        }
                        v1::SplitTarget::Transfer(side) => {
                            SplitTarget::Transfer(TransactionId::new(side.other_side))
                        }
                    };
                    SplitLine::new(target, &line.memo, money(line.amount, currency))
                })
                .collect();
            let amount = SplitLine::sum(&lines, currency)?;
            (Allocation::Split(lines), amount)
        }
        v1::Allocation::Transfer(side) => (
            Allocation::Transfer(TransactionId::new(side.other_side)),
            money(side.amount, currency),
        ),
    };
    Ok(Transaction::from_parts(
        TransactionId::new(transaction.id),
        date(&date_text)?,
        number,
        payee,
        memo,
        allocation,
        amount,
    )
    .with_flag(transaction.flagged)
    .with_cleared_status(cleared_status_from_wire(transaction.cleared_status)))
}

fn other_side_from_wire(
    other_side: v1::OtherSide,
    currency: CurrencyCode,
) -> Result<OtherSide, UndoEntryError> {
    Ok(OtherSide::new(
        AccountId::new(other_side.account),
        transaction_from_wire(other_side.transaction, currency)?,
    ))
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
    use crate::transaction::{
        EditedSplitLine, EditedSplitTarget, NewSplitLine, NewSplitTarget, SplitLine, SplitTarget,
    };
    use crate::{
        Account, AccountId, AccountKind, AccountStatus, Allocation, Category, CategoryId,
        CategoryUse, Money, MoveDirection, NewTransaction, Transaction, TransactionEdit,
        TransactionId, Visibility,
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
                vec![NewSplitLine::new(
                    NewSplitTarget::Category(CategoryId::new(3)),
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
        let restore = |transaction| DomainCommand::RestoreTransaction {
            account,
            transaction,
            other_sides: Vec::new(),
        };
        let [opening, restored, paid] = transactions();
        let [shell, split, opening_balance] = new_transactions();
        let [post_transfer, restore_transfer, post_split, restore_split] = transfer_commands();
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
            post_transfer,
            post_split,
        ]
        .into_iter()
        .chain(edit_commands(transaction))
        .chain([
            DomainCommand::SetFlagged {
                transaction,
                flagged: true,
            },
            DomainCommand::SetFlagged {
                transaction,
                flagged: false,
            },
            DomainCommand::DeleteTransaction { transaction },
            restore(restored.clone()),
            restore(restored.clone().with_flag(true)),
            restore(restored.with_cleared_status(ClearedStatus::Reconciled)),
            restore_transfer,
            restore_split,
        ])
        .chain(category_commands())
        .collect()
    }

    /// A transfer posted, and one restored with its other side; then a split with a line
    /// to an account posted, and one restored with its line's other side.
    fn transfer_commands() -> [DomainCommand; 4] {
        let sent = Transaction::from_parts(
            TransactionId::new(14),
            date(5),
            Some("1045".to_owned()),
            "Saving up".to_owned(),
            "September".to_owned(),
            Allocation::Transfer(TransactionId::new(15)),
            usd(dec!(-200.00)),
        );
        let received = sent.other_side(TransactionId::new(15)).with_flag(true);
        let payday = Transaction::from_parts(
            TransactionId::new(16),
            date(6),
            None,
            "Payday".to_owned(),
            String::new(),
            Allocation::Split(vec![
                SplitLine::new(
                    SplitTarget::Category(CategoryId::new(3)),
                    "Food",
                    usd(dec!(-50.00)),
                ),
                SplitLine::new(
                    SplitTarget::Transfer(TransactionId::new(17)),
                    "Rainy day",
                    usd(dec!(-200.00)),
                ),
            ]),
            usd(dec!(-250.00)),
        );
        let [rainy_day] = <[Transaction; 1]>::try_from(payday.other_sides())
            .expect("the split has one line to an account");
        [
            DomainCommand::PostTransaction {
                account: AccountId::new(7),
                transaction: NewTransaction::transfer(
                    date(5),
                    "Saving up",
                    AccountId::new(8),
                    usd(dec!(-200.00)),
                ),
            },
            DomainCommand::RestoreTransaction {
                account: AccountId::new(7),
                transaction: sent,
                other_sides: vec![OtherSide::new(AccountId::new(8), received)],
            },
            DomainCommand::PostTransaction {
                account: AccountId::new(7),
                transaction: NewTransaction::split(
                    date(6),
                    "Payday",
                    vec![
                        NewSplitLine::new(
                            NewSplitTarget::Category(CategoryId::new(3)),
                            "Food",
                            usd(dec!(-50.00)),
                        ),
                        NewSplitLine::new(
                            NewSplitTarget::Transfer(AccountId::new(8)),
                            "Rainy day",
                            usd(dec!(-200.00)),
                        ),
                    ],
                ),
            },
            DomainCommand::RestoreTransaction {
                account: AccountId::new(7),
                transaction: payday,
                other_sides: vec![OtherSide::new(AccountId::new(8), rainy_day)],
            },
        ]
    }

    /// One edit of every field of `transaction`, every value set and unset. Its lines
    /// go to a category, a new account and a kept other side, then to nothing.
    fn edit_commands(transaction: TransactionId) -> Vec<DomainCommand> {
        let edit = |edit| DomainCommand::EditTransaction { transaction, edit };
        vec![
            edit(TransactionEdit::Date(date(9))),
            edit(TransactionEdit::Number(Some("1044".to_owned()))),
            edit(TransactionEdit::Number(None)),
            edit(TransactionEdit::Payee("Fresh Market".to_owned())),
            edit(TransactionEdit::Memo("Weekly shop".to_owned())),
            edit(TransactionEdit::Category(CategoryId::new(6))),
            edit(TransactionEdit::Transfer(AccountId::new(8))),
            edit(TransactionEdit::Amount(usd(dec!(-90.00)))),
            edit(TransactionEdit::ClearedStatus(ClearedStatus::Uncleared)),
            edit(TransactionEdit::ClearedStatus(ClearedStatus::Cleared)),
            edit(TransactionEdit::ClearedStatus(ClearedStatus::Reconciled)),
            edit(TransactionEdit::Lines {
                amount: usd(dec!(-350.00)),
                lines: vec![
                    EditedSplitLine::new(
                        EditedSplitTarget::Category(CategoryId::new(3)),
                        "Food",
                        usd(dec!(-50.00)),
                    ),
                    EditedSplitLine::new(
                        EditedSplitTarget::Transfer(AccountId::new(8)),
                        "Car",
                        usd(dec!(-100.00)),
                    ),
                    EditedSplitLine::new(
                        EditedSplitTarget::Kept(TransactionId::new(17)),
                        "Rainy day",
                        usd(dec!(-200.00)),
                    ),
                ],
            }),
            edit(TransactionEdit::Lines {
                amount: usd(dec!(0.00)),
                lines: Vec::new(),
            }),
        ]
    }

    /// One of every category Command, every field set and unset.
    fn category_commands() -> [DomainCommand; 13] {
        [
            DomainCommand::CreateCategory {
                name: "Fuel".to_owned(),
                parent: Some(CategoryId::new(5)),
                category_type: CategoryType::Expense,
            },
            DomainCommand::CreateCategory {
                name: "Salary".to_owned(),
                parent: None,
                category_type: CategoryType::Income,
            },
            DomainCommand::DeleteCategory {
                category: CategoryId::new(6),
            },
            DomainCommand::RestoreCategory {
                category: Category::new(
                    CategoryId::new(6),
                    "Fuel".to_owned(),
                    None,
                    CategoryType::Expense,
                ),
            },
            DomainCommand::RestoreCategory {
                category: Category::new(
                    CategoryId::new(7),
                    "Salary".to_owned(),
                    None,
                    CategoryType::Income,
                ),
            },
            DomainCommand::RenameCategory {
                category: CategoryId::new(6),
                name: "Petrol".to_owned(),
            },
            DomainCommand::MoveCategory {
                category: CategoryId::new(6),
                parent: Some(CategoryId::new(5)),
            },
            DomainCommand::MoveCategory {
                category: CategoryId::new(6),
                parent: None,
            },
            DomainCommand::SetCategoryType {
                category: CategoryId::new(5),
                category_type: CategoryType::Income,
            },
            DomainCommand::SetCategoryType {
                category: CategoryId::new(5),
                category_type: CategoryType::Expense,
            },
            DomainCommand::ReassignCategory {
                from: CategoryId::new(6),
                to: CategoryId::new(3),
            },
            DomainCommand::ReassignUses {
                from: CategoryId::new(3),
                to: CategoryId::new(6),
                uses: vec![
                    CategoryUse::Transaction(TransactionId::new(12)),
                    CategoryUse::SplitLine {
                        split: TransactionId::new(13),
                        line: 1,
                    },
                ],
            },
            DomainCommand::ReassignUses {
                from: CategoryId::new(3),
                to: CategoryId::new(6),
                uses: Vec::new(),
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
            DomainCommand::RenameCategory { .. } => 16,
            DomainCommand::MoveCategory { .. } => 17,
            DomainCommand::SetCategoryType { .. } => 18,
            DomainCommand::ReassignCategory { .. } => 19,
            DomainCommand::ReassignUses { .. } => 20,
        }
    }

    #[test]
    fn every_command_covers_every_variant() {
        let mut seen: Vec<usize> = every_command().iter().map(variant).collect();
        seen.dedup();

        assert_eq!(seen, (0..21).collect::<Vec<_>>());
    }

    #[test]
    fn every_command_reads_back_as_it_was_written() {
        for command in every_command() {
            let entry = encode(&command).unwrap();

            assert_eq!(decode(&entry, usd_code()).unwrap(), command, "{entry}");
        }
    }

    #[test]
    fn a_transaction_written_without_a_status_reads_back_uncleared() {
        let entry = r#"{"restore_transaction":{"account":7,"transaction":{"id":12,"date":"2026-09-03","number":null,"payee":"Shell","memo":"","allocation":{"category":{"category":5,"amount":-4820}}}}}"#;

        let command = decode(entry, usd_code()).unwrap();

        let [_, _, paid] = transactions();
        assert_eq!(
            command,
            DomainCommand::RestoreTransaction {
                account: AccountId::new(7),
                transaction: paid,
                other_sides: Vec::new(),
            }
        );
    }

    #[test]
    fn an_edited_lines_transfer_naming_both_an_account_and_an_other_side_is_refused() {
        let entry = r#"{"edit_transaction":{"transaction":12,"edit":{"lines":{"amount":-100,"lines":[{"transfer":{"account":8,"other_side":17},"memo":"","amount":-100}]}}}}"#;

        let error = decode(entry, usd_code()).unwrap_err();

        assert!(matches!(error, UndoEntryError::Json(_)), "{error:?}");
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
