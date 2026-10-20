# Both sides of a transfer store the date and amount

A transfer is two transactions, one in each account, and they share their date and their amount, with the sign flipped. In the data file, each side is a full `transaction` row, with `allocation = 'transfer'` and a link to its other side, and stores its own date and amount. `Ledger` writes both sides in one SQLite transaction, and the schema guards the link, so a side can't point at a missing or unrelated row. Opening the file checks that each pair has the same date and amounts that cancel out. It reports a pair that doesn't without refusing the file, and editing either side's date or amount writes both and repairs it.

Decided in [Account transfers](https://github.com/StanleyGoldman/balanced/issues/211) (Q3).

## Why this differs from ADR 0005

ADR 0005 keeps no copy of a split's amount, because a copy that can't be guarded can drift. A transfer's pair can't be guarded either: a trigger fires after the first side is written, before the second exists. We store the copy anyway, for two reasons:

- **The date must be on both rows.** Register order and its index read `transaction.date`. Deriving one side's date would put a join under every register query and every view.
- **A drifted pair doesn't corrupt any number.** Every balance sums stored rows, so `Ledger` and the views still agree. Net Worth is off by the difference, which both registers show. Only a bug in `Ledger` or a hand edit with a SQLite tool can cause drift, since both sides are committed together.

## Consequences

- The open-time check is a report, not `OpenError::Damaged`. Refusing the file would lock the household out of all its data over one pair.
- `register_entry`, `account_balance` and the register-order index read transfers like any other transaction.
- Balanced never repairs a pair by itself, because it can't tell which side is right.

## Considered options

- **One side stores the amount and the other derives it,** as ADR 0005 does for splits: rejected. The date still needs both rows, and it makes one side the "owner", which means nothing for a plain transfer.
- **A `transfer` row holding the date and amount, with both sides pointing at it:** rejected. It can't drift, but the date leaves `transaction`, so register order and every view join through it on every read.
- **Refuse to open a file whose pair has drifted:** rejected (above).
