# A split transaction's amount is derived, not stored

In the data file, a split transaction stores no amount and no category. Its amount is the sum of its `split_line` rows, which the `transaction_amount` view computes. `transaction.allocation` is `'opening_balance'`, `'category'` or `'split'`, and CHECK constraints make `amount` and `category_id` NULL exactly when it is `'split'`. Every reader that needs a split's amount, the other views included, goes through `transaction_amount`. We chose this because "a split's lines sum to its amount" can't be guarded in SQLite once the amount is stored, and a guard that can't hold is a copy that can drift.

Decided in [Data file schema](https://github.com/StanleyGoldman/balanced/issues/227) (Q2), recorded by the spec [#236](https://github.com/StanleyGoldman/balanced/issues/236).

## Why the sum can't be guarded

A split is written as several statements: the `transaction` row, then each `split_line`. A trigger checking that the lines sum to the stored amount fires after each statement, so it sees the transaction with only some of its lines and fails. SQLite triggers can't be deferred to commit; only foreign key checks can (`PRAGMA defer_foreign_keys`). With no stored amount, there is nothing for the lines to disagree with.

## Consequences

- `Ledger` still holds a split's amount in memory (`Transaction::amount`), computed from its lines. The `transaction_amount` view is the oracle it is checked against, with the other derived numbers, after a reopen.
- Turning a split into a one-category transaction sets `allocation`, `amount` and `category_id` in one `UPDATE`, and the store deletes the lines itself.
- Reading a split's amount costs a sum over its lines. Lines are indexed by `transaction_id`, so the sum reads only that transaction's lines.

## Considered options

- **Store the amount and guard the sum with a trigger:** rejected. The trigger fires per statement, so it can't see a whole split being written (above).
- **Store the amount and trust `Ledger`:** rejected. A bug in `Ledger`, or a hand edit with a SQLite tool, could leave a file whose split amounts disagree with their lines, and nothing would say which is right.
- **Write the lines before the transaction row:** rejected. The lines' foreign key then points at a missing row, and the trigger would still fire once per line.
