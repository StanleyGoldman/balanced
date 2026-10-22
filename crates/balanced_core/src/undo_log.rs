// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The undo log's entries: each a [`DomainCommand`] as JSON, through the wire types in
//! [`wire`], which `DomainCommand` itself doesn't depend on (ADR 0007).

mod wire;

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
fn to_wire(command: &DomainCommand) -> Result<wire::Command, WriteError> {
    Ok(match command {
        DomainCommand::CreateAccount {
            name,
            kind,
            opened,
            opening_balance,
        } => wire::Command::CreateAccount {
            name: name.clone(),
            kind: kind_to_wire(*kind),
            opened: date_text(*opened),
            opening_balance: minor_units(*opening_balance)?,
        },
        DomainCommand::DeleteAccount { account } => wire::Command::DeleteAccount {
            account: account.raw(),
        },
        DomainCommand::RestoreAccount {
            account,
            transactions,
            place,
        } => wire::Command::RestoreAccount {
            account: account_to_wire(account),
            transactions: transactions
                .iter()
                .map(transaction_to_wire)
                .collect::<Result<_, _>>()?,
            place: u32::try_from(*place).expect("a sub-group holds fewer than 2^32 accounts"),
        },
        DomainCommand::RenameAccount { account, name } => wire::Command::RenameAccount {
            account: account.raw(),
            name: name.clone(),
        },
        DomainCommand::SetAccountVisibility {
            account,
            visibility,
        } => wire::Command::SetAccountVisibility {
            account: account.raw(),
            visibility: visibility_to_wire(*visibility),
        },
        DomainCommand::SetAccountStatus { account, status } => wire::Command::SetAccountStatus {
            account: account.raw(),
            status: status_to_wire(*status),
        },
        DomainCommand::MoveAccount { account, direction } => wire::Command::MoveAccount {
            account: account.raw(),
            direction: match direction {
                MoveDirection::Up => wire::MoveDirection::Up,
                MoveDirection::Down => wire::MoveDirection::Down,
            },
        },
        DomainCommand::Compound(commands) => {
            wire::Command::Compound(commands.iter().map(to_wire).collect::<Result<_, _>>()?)
        }
        DomainCommand::PostTransaction {
            account,
            transaction,
        } => wire::Command::PostTransaction {
            account: account.raw(),
            transaction: new_transaction_to_wire(transaction)?,
        },
        DomainCommand::EditTransaction { transaction, edit } => wire::Command::EditTransaction {
            transaction: transaction.raw(),
            edit: edit_to_wire(edit)?,
        },
        DomainCommand::SetFlagged {
            transaction,
            flagged,
        } => wire::Command::SetFlagged {
            transaction: transaction.raw(),
            flagged: *flagged,
        },
        DomainCommand::DeleteTransaction { transaction } => wire::Command::DeleteTransaction {
            transaction: transaction.raw(),
        },
        DomainCommand::RestoreTransaction {
            account,
            transaction,
            other_sides,
        } => wire::Command::RestoreTransaction {
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
        } => wire::Command::CreateCategory {
            name: name.clone(),
            parent: parent.map(CategoryId::raw),
            category_type: category_type_to_wire(*category_type),
        },
        DomainCommand::DeleteCategory { category } => wire::Command::DeleteCategory {
            category: category.raw(),
        },
        DomainCommand::RestoreCategory { category } => wire::Command::RestoreCategory {
            category: category_to_wire(category),
        },
        DomainCommand::RenameCategory { category, name } => wire::Command::RenameCategory {
            category: category.raw(),
            name: name.clone(),
        },
        DomainCommand::MoveCategory { category, parent } => wire::Command::MoveCategory {
            category: category.raw(),
            parent: parent.map(CategoryId::raw),
        },
        DomainCommand::EditCategory {
            category,
            name,
            parent,
        } => wire::Command::EditCategory {
            category: category.raw(),
            name: name.clone(),
            parent: parent.map(CategoryId::raw),
        },
        DomainCommand::SetCategoryType {
            category,
            category_type,
        } => wire::Command::SetCategoryType {
            category: category.raw(),
            category_type: category_type_to_wire(*category_type),
        },
        DomainCommand::SetCategoryVisibility {
            category,
            visibility,
        } => wire::Command::SetCategoryVisibility {
            category: category.raw(),
            visibility: visibility_to_wire(*visibility),
        },
        DomainCommand::ReassignCategory { from, to } => wire::Command::ReassignCategory {
            from: from.raw(),
            to: to.raw(),
        },
        DomainCommand::ReassignUses { from, to, uses } => wire::Command::ReassignUses {
            from: from.raw(),
            to: to.raw(),
            uses: uses.iter().copied().map(category_use_to_wire).collect(),
        },
    })
}

fn category_use_to_wire(category_use: CategoryUse) -> wire::CategoryUse {
    match category_use {
        CategoryUse::Transaction(transaction) => wire::CategoryUse::Transaction(transaction.raw()),
        CategoryUse::SplitLine { split, line } => wire::CategoryUse::SplitLine {
            split: split.raw(),
            line: u32::try_from(line).expect("a split holds fewer than 2^32 lines"),
        },
    }
}

fn account_to_wire(account: &Account) -> wire::Account {
    wire::Account {
        id: account.id().raw(),
        name: account.name().to_owned(),
        kind: kind_to_wire(account.kind()),
        visibility: visibility_to_wire(account.visibility()),
        status: status_to_wire(account.status()),
    }
}

fn new_transaction_to_wire(
    transaction: &NewTransaction,
) -> Result<wire::NewTransaction, WriteError> {
    let NewTransactionParts {
        date,
        number,
        payee,
        memo,
        allocation,
    } = transaction.parts();
    Ok(wire::NewTransaction {
        date: date_text(date),
        number: number.map(str::to_owned),
        payee: payee.to_owned(),
        memo: memo.to_owned(),
        allocation: match allocation {
            NewAllocation::OpeningBalance(amount) => wire::Allocation::OpeningBalance {
                amount: minor_units(*amount)?,
            },
            NewAllocation::Category(category, amount) => wire::Allocation::Category {
                category: category.raw(),
                amount: minor_units(*amount)?,
            },
            NewAllocation::Split(lines) => wire::Allocation::Split(
                lines
                    .iter()
                    .map(|line| {
                        let target = match line.target() {
                            NewSplitTarget::Category(category) => {
                                wire::SplitTarget::Category(category.raw())
                            }
                            NewSplitTarget::Transfer(account) => {
                                wire::SplitTarget::Transfer(wire::LineTo {
                                    account: account.raw(),
                                })
                            }
                        };
                        line_to_wire(target, line.memo(), line.amount())
                    })
                    .collect::<Result<_, _>>()?,
            ),
            NewAllocation::Transfer(account, amount) => {
                wire::Allocation::Transfer(wire::TransferTo {
                    account: account.raw(),
                    amount: minor_units(*amount)?,
                })
            }
        },
    })
}

fn edit_to_wire(edit: &TransactionEdit) -> Result<wire::TransactionEdit, WriteError> {
    Ok(match edit {
        TransactionEdit::Date(date) => wire::TransactionEdit::Date(date_text(*date)),
        TransactionEdit::Number(number) => wire::TransactionEdit::Number(number.clone()),
        TransactionEdit::Payee(payee) => wire::TransactionEdit::Payee(payee.clone()),
        TransactionEdit::Memo(memo) => wire::TransactionEdit::Memo(memo.clone()),
        TransactionEdit::Category(category) => wire::TransactionEdit::Category(category.raw()),
        TransactionEdit::Transfer(account) => wire::TransactionEdit::Transfer(account.raw()),
        TransactionEdit::Amount(amount) => wire::TransactionEdit::Amount(minor_units(*amount)?),
        TransactionEdit::ClearedStatus(status) => {
            wire::TransactionEdit::ClearedStatus(cleared_status_to_wire(*status))
        }
        TransactionEdit::Lines { amount, lines } => wire::TransactionEdit::Lines {
            amount: minor_units(*amount)?,
            lines: lines
                .iter()
                .map(|line| {
                    let target = match line.target() {
                        EditedSplitTarget::Category(category) => {
                            wire::SplitTarget::Category(category.raw())
                        }
                        EditedSplitTarget::Transfer(account) => wire::SplitTarget::Transfer(
                            wire::EditedLineTransfer::New(wire::LineTo {
                                account: account.raw(),
                            }),
                        ),
                        EditedSplitTarget::Kept(other_side) => wire::SplitTarget::Transfer(
                            wire::EditedLineTransfer::Kept(wire::LineSide {
                                other_side: other_side.raw(),
                            }),
                        ),
                    };
                    line_to_wire(target, line.memo(), line.amount())
                })
                .collect::<Result<_, _>>()?,
        },
    })
}

fn cleared_status_to_wire(status: ClearedStatus) -> wire::ClearedStatus {
    match status {
        ClearedStatus::Uncleared => wire::ClearedStatus::Uncleared,
        ClearedStatus::Cleared => wire::ClearedStatus::Cleared,
        ClearedStatus::Reconciled => wire::ClearedStatus::Reconciled,
    }
}

fn transaction_to_wire(transaction: &Transaction) -> Result<wire::Transaction, WriteError> {
    let amount = || minor_units(transaction.amount());
    Ok(wire::Transaction {
        id: transaction.id().raw(),
        fields: wire::NewTransaction {
            date: date_text(transaction.date()),
            number: transaction.number().map(str::to_owned),
            payee: transaction.payee().to_owned(),
            memo: transaction.memo().to_owned(),
            allocation: match transaction.allocation() {
                Allocation::OpeningBalance => {
                    wire::Allocation::OpeningBalance { amount: amount()? }
                }
                Allocation::Category(category) => wire::Allocation::Category {
                    category: category.raw(),
                    amount: amount()?,
                },
                Allocation::Split(lines) => wire::Allocation::Split(
                    lines
                        .iter()
                        .map(|line| {
                            let target = match line.target() {
                                SplitTarget::Category(category) => {
                                    wire::SplitTarget::Category(category.raw())
                                }
                                SplitTarget::Transfer(other_side) => {
                                    wire::SplitTarget::Transfer(wire::LineSide {
                                        other_side: other_side.raw(),
                                    })
                                }
                            };
                            line_to_wire(target, line.memo(), line.amount())
                        })
                        .collect::<Result<_, _>>()?,
                ),
                Allocation::Transfer(other_side) => {
                    wire::Allocation::Transfer(wire::TransferSide {
                        other_side: other_side.raw(),
                        amount: amount()?,
                    })
                }
            },
        },
        flagged: transaction.is_flagged(),
        cleared_status: cleared_status_to_wire(transaction.cleared_status()),
    })
}

fn other_side_to_wire(other_side: &OtherSide) -> Result<wire::OtherSide, WriteError> {
    Ok(wire::OtherSide {
        account: other_side.account().raw(),
        transaction: transaction_to_wire(other_side.transaction())?,
    })
}

/// A split line going to `target`, on the wire.
fn line_to_wire<L>(
    target: wire::SplitTarget<L>,
    memo: &str,
    amount: Money,
) -> Result<wire::SplitLine<L>, WriteError> {
    Ok(wire::SplitLine {
        target,
        memo: memo.to_owned(),
        amount: minor_units(amount)?,
    })
}

fn kind_to_wire(kind: AccountKind) -> wire::AccountKind {
    match kind {
        AccountKind::Checking => wire::AccountKind::Checking,
        AccountKind::Savings => wire::AccountKind::Savings,
        AccountKind::CreditCard => wire::AccountKind::CreditCard,
        AccountKind::Investment => wire::AccountKind::Investment,
        AccountKind::Retirement => wire::AccountKind::Retirement,
        AccountKind::Asset => wire::AccountKind::Asset,
        AccountKind::Liability => wire::AccountKind::Liability,
    }
}

fn visibility_to_wire(visibility: Visibility) -> wire::Visibility {
    match visibility {
        Visibility::Shown => wire::Visibility::Shown,
        Visibility::Hidden => wire::Visibility::Hidden,
    }
}

fn category_to_wire(category: &Category) -> wire::Category {
    wire::Category {
        id: category.id().raw(),
        name: category.name().to_owned(),
        parent: category.parent().map(CategoryId::raw),
        category_type: category_type_to_wire(category.category_type()),
        visibility: visibility_to_wire(category.visibility()),
    }
}

fn category_type_to_wire(category_type: CategoryType) -> wire::CategoryType {
    match category_type {
        CategoryType::Income => wire::CategoryType::Income,
        CategoryType::Expense => wire::CategoryType::Expense,
    }
}

fn status_to_wire(status: AccountStatus) -> wire::AccountStatus {
    match status {
        AccountStatus::Open => wire::AccountStatus::Open,
        AccountStatus::Closed => wire::AccountStatus::Closed,
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per Command, each mapping its fields one by one"
)]
fn from_wire(
    command: wire::Command,
    currency: CurrencyCode,
) -> Result<DomainCommand, UndoEntryError> {
    Ok(match command {
        wire::Command::CreateAccount {
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
        wire::Command::DeleteAccount { account } => DomainCommand::DeleteAccount {
            account: AccountId::new(account),
        },
        wire::Command::RestoreAccount {
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
        wire::Command::RenameAccount { account, name } => DomainCommand::RenameAccount {
            account: AccountId::new(account),
            name,
        },
        wire::Command::SetAccountVisibility {
            account,
            visibility,
        } => DomainCommand::SetAccountVisibility {
            account: AccountId::new(account),
            visibility: visibility_from_wire(visibility),
        },
        wire::Command::SetAccountStatus { account, status } => DomainCommand::SetAccountStatus {
            account: AccountId::new(account),
            status: status_from_wire(status),
        },
        wire::Command::MoveAccount { account, direction } => DomainCommand::MoveAccount {
            account: AccountId::new(account),
            direction: match direction {
                wire::MoveDirection::Up => MoveDirection::Up,
                wire::MoveDirection::Down => MoveDirection::Down,
            },
        },
        wire::Command::Compound(commands) => DomainCommand::Compound(
            commands
                .into_iter()
                .map(|command| from_wire(command, currency))
                .collect::<Result<_, _>>()?,
        ),
        wire::Command::PostTransaction {
            account,
            transaction,
        } => DomainCommand::PostTransaction {
            account: AccountId::new(account),
            transaction: new_transaction_from_wire(transaction, currency)?,
        },
        wire::Command::EditTransaction { transaction, edit } => DomainCommand::EditTransaction {
            transaction: TransactionId::new(transaction),
            edit: edit_from_wire(edit, currency)?,
        },
        wire::Command::SetFlagged {
            transaction,
            flagged,
        } => DomainCommand::SetFlagged {
            transaction: TransactionId::new(transaction),
            flagged,
        },
        wire::Command::DeleteTransaction { transaction } => DomainCommand::DeleteTransaction {
            transaction: TransactionId::new(transaction),
        },
        wire::Command::RestoreTransaction {
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
        wire::Command::CreateCategory {
            name,
            parent,
            category_type,
        } => DomainCommand::CreateCategory {
            name,
            parent: parent.map(CategoryId::new),
            category_type: category_type_from_wire(category_type),
        },
        wire::Command::DeleteCategory { category } => DomainCommand::DeleteCategory {
            category: CategoryId::new(category),
        },
        wire::Command::RestoreCategory { category } => DomainCommand::RestoreCategory {
            category: category_from_wire(category),
        },
        wire::Command::RenameCategory { category, name } => DomainCommand::RenameCategory {
            category: CategoryId::new(category),
            name,
        },
        wire::Command::MoveCategory { category, parent } => DomainCommand::MoveCategory {
            category: CategoryId::new(category),
            parent: parent.map(CategoryId::new),
        },
        wire::Command::EditCategory {
            category,
            name,
            parent,
        } => DomainCommand::EditCategory {
            category: CategoryId::new(category),
            name,
            parent: parent.map(CategoryId::new),
        },
        wire::Command::SetCategoryType {
            category,
            category_type,
        } => DomainCommand::SetCategoryType {
            category: CategoryId::new(category),
            category_type: category_type_from_wire(category_type),
        },
        wire::Command::SetCategoryVisibility {
            category,
            visibility,
        } => DomainCommand::SetCategoryVisibility {
            category: CategoryId::new(category),
            visibility: visibility_from_wire(visibility),
        },
        wire::Command::ReassignCategory { from, to } => DomainCommand::ReassignCategory {
            from: CategoryId::new(from),
            to: CategoryId::new(to),
        },
        wire::Command::ReassignUses { from, to, uses } => DomainCommand::ReassignUses {
            from: CategoryId::new(from),
            to: CategoryId::new(to),
            uses: uses.into_iter().map(category_use_from_wire).collect(),
        },
    })
}

fn category_use_from_wire(category_use: wire::CategoryUse) -> CategoryUse {
    match category_use {
        wire::CategoryUse::Transaction(transaction) => {
            CategoryUse::Transaction(TransactionId::new(transaction))
        }
        wire::CategoryUse::SplitLine { split, line } => CategoryUse::SplitLine {
            split: TransactionId::new(split),
            line: usize::try_from(line).expect("a usize holds every u32 Balanced runs on"),
        },
    }
}

fn category_from_wire(category: wire::Category) -> Category {
    Category::new(
        CategoryId::new(category.id),
        category.name,
        category.parent.map(CategoryId::new),
        category_type_from_wire(category.category_type),
        visibility_from_wire(category.visibility),
    )
}

fn category_type_from_wire(category_type: wire::CategoryType) -> CategoryType {
    match category_type {
        wire::CategoryType::Income => CategoryType::Income,
        wire::CategoryType::Expense => CategoryType::Expense,
    }
}

fn account_from_wire(account: wire::Account, currency: CurrencyCode) -> Account {
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
    transaction: wire::NewTransaction,
    currency: CurrencyCode,
) -> Result<NewTransaction, UndoEntryError> {
    let allocation = match transaction.allocation {
        wire::Allocation::OpeningBalance { amount } => {
            NewAllocation::OpeningBalance(money(amount, currency))
        }
        wire::Allocation::Category { category, amount } => {
            NewAllocation::Category(CategoryId::new(category), money(amount, currency))
        }
        wire::Allocation::Split(lines) => NewAllocation::Split(
            lines
                .into_iter()
                .map(|line| {
                    let target = match line.target {
                        wire::SplitTarget::Category(category) => {
                            NewSplitTarget::Category(CategoryId::new(category))
                        }
                        wire::SplitTarget::Transfer(to) => {
                            NewSplitTarget::Transfer(AccountId::new(to.account))
                        }
                    };
                    NewSplitLine::new(target, &line.memo, money(line.amount, currency))
                })
                .collect(),
        ),
        wire::Allocation::Transfer(to) => {
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
    edit: wire::TransactionEdit,
    currency: CurrencyCode,
) -> Result<TransactionEdit, UndoEntryError> {
    Ok(match edit {
        wire::TransactionEdit::Date(text) => TransactionEdit::Date(date(&text)?),
        wire::TransactionEdit::Number(number) => TransactionEdit::Number(number),
        wire::TransactionEdit::Payee(payee) => TransactionEdit::Payee(payee),
        wire::TransactionEdit::Memo(memo) => TransactionEdit::Memo(memo),
        wire::TransactionEdit::Category(category) => {
            TransactionEdit::Category(CategoryId::new(category))
        }
        wire::TransactionEdit::Transfer(account) => {
            TransactionEdit::Transfer(AccountId::new(account))
        }
        wire::TransactionEdit::Amount(amount) => TransactionEdit::Amount(money(amount, currency)),
        wire::TransactionEdit::ClearedStatus(status) => {
            TransactionEdit::ClearedStatus(cleared_status_from_wire(status))
        }
        wire::TransactionEdit::Lines { amount, lines } => TransactionEdit::Lines {
            amount: money(amount, currency),
            lines: lines
                .into_iter()
                .map(|line| {
                    let target = match line.target {
                        wire::SplitTarget::Category(category) => {
                            EditedSplitTarget::Category(CategoryId::new(category))
                        }
                        wire::SplitTarget::Transfer(wire::EditedLineTransfer::New(to)) => {
                            EditedSplitTarget::Transfer(AccountId::new(to.account))
                        }
                        wire::SplitTarget::Transfer(wire::EditedLineTransfer::Kept(side)) => {
                            EditedSplitTarget::Kept(TransactionId::new(side.other_side))
                        }
                    };
                    EditedSplitLine::new(target, &line.memo, money(line.amount, currency))
                })
                .collect(),
        },
    })
}

fn cleared_status_from_wire(status: wire::ClearedStatus) -> ClearedStatus {
    match status {
        wire::ClearedStatus::Uncleared => ClearedStatus::Uncleared,
        wire::ClearedStatus::Cleared => ClearedStatus::Cleared,
        wire::ClearedStatus::Reconciled => ClearedStatus::Reconciled,
    }
}

/// The transaction `transaction` holds. A split's amount is the sum of its lines.
fn transaction_from_wire(
    transaction: wire::Transaction,
    currency: CurrencyCode,
) -> Result<Transaction, UndoEntryError> {
    let wire::NewTransaction {
        date: date_text,
        number,
        payee,
        memo,
        allocation,
    } = transaction.fields;
    let (allocation, amount) = match allocation {
        wire::Allocation::OpeningBalance { amount } => {
            (Allocation::OpeningBalance, money(amount, currency))
        }
        wire::Allocation::Category { category, amount } => (
            Allocation::Category(CategoryId::new(category)),
            money(amount, currency),
        ),
        wire::Allocation::Split(lines) => {
            let lines: Vec<SplitLine> = lines
                .into_iter()
                .map(|line| {
                    let target = match line.target {
                        wire::SplitTarget::Category(category) => {
                            SplitTarget::Category(CategoryId::new(category))
                        }
                        wire::SplitTarget::Transfer(side) => {
                            SplitTarget::Transfer(TransactionId::new(side.other_side))
                        }
                    };
                    SplitLine::new(target, &line.memo, money(line.amount, currency))
                })
                .collect();
            let amount = SplitLine::sum(&lines, currency)?;
            (Allocation::Split(lines), amount)
        }
        wire::Allocation::Transfer(side) => (
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
    other_side: wire::OtherSide,
    currency: CurrencyCode,
) -> Result<OtherSide, UndoEntryError> {
    Ok(OtherSide::new(
        AccountId::new(other_side.account),
        transaction_from_wire(other_side.transaction, currency)?,
    ))
}

fn kind_from_wire(kind: wire::AccountKind) -> AccountKind {
    match kind {
        wire::AccountKind::Checking => AccountKind::Checking,
        wire::AccountKind::Savings => AccountKind::Savings,
        wire::AccountKind::CreditCard => AccountKind::CreditCard,
        wire::AccountKind::Investment => AccountKind::Investment,
        wire::AccountKind::Retirement => AccountKind::Retirement,
        wire::AccountKind::Asset => AccountKind::Asset,
        wire::AccountKind::Liability => AccountKind::Liability,
    }
}

fn visibility_from_wire(visibility: wire::Visibility) -> Visibility {
    match visibility {
        wire::Visibility::Shown => Visibility::Shown,
        wire::Visibility::Hidden => Visibility::Hidden,
    }
}

fn status_from_wire(status: wire::AccountStatus) -> AccountStatus {
    match status {
        wire::AccountStatus::Open => AccountStatus::Open,
        wire::AccountStatus::Closed => AccountStatus::Closed,
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
    fn category_commands() -> [DomainCommand; 17] {
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
                    Visibility::Shown,
                ),
            },
            DomainCommand::RestoreCategory {
                category: Category::new(
                    CategoryId::new(7),
                    "Salary".to_owned(),
                    None,
                    CategoryType::Income,
                    Visibility::Hidden,
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
            DomainCommand::EditCategory {
                category: CategoryId::new(6),
                name: "Petrol".to_owned(),
                parent: Some(CategoryId::new(5)),
            },
            DomainCommand::EditCategory {
                category: CategoryId::new(6),
                name: "Petrol".to_owned(),
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
            DomainCommand::SetCategoryVisibility {
                category: CategoryId::new(5),
                visibility: Visibility::Hidden,
            },
            DomainCommand::SetCategoryVisibility {
                category: CategoryId::new(5),
                visibility: Visibility::Shown,
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
            DomainCommand::EditCategory { .. } => 18,
            DomainCommand::SetCategoryType { .. } => 19,
            DomainCommand::SetCategoryVisibility { .. } => 20,
            DomainCommand::ReassignCategory { .. } => 21,
            DomainCommand::ReassignUses { .. } => 22,
        }
    }

    #[test]
    fn every_command_covers_every_variant() {
        let mut seen: Vec<usize> = every_command().iter().map(variant).collect();
        seen.dedup();

        assert_eq!(seen, (0..23).collect::<Vec<_>>());
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
    fn a_category_written_without_a_visibility_reads_back_shown() {
        let entry = r#"{"restore_category":{"category":{"id":6,"name":"Fuel","parent":null,"category_type":"expense"}}}"#;

        let command = decode(entry, usd_code()).unwrap();

        let DomainCommand::RestoreCategory { category } = command else {
            panic!("{command:?}");
        };
        assert_eq!(category.visibility(), Visibility::Shown);
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
