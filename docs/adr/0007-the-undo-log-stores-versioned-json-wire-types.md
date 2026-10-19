# The undo log stores versioned JSON wire types, not `DomainCommand` directly

The data file keeps `Ledger`'s Undo and Redo stacks in one `STRICT` table, `undo_entry`, one row per entry, so both stacks survive a reopen. Each entry is a `DomainCommand` as JSON text, checked by `CHECK (json_valid(command))`. The JSON comes from wire types in `balanced_core` (`undo_log::v1`), which derive `serde` and are mapped by hand to and from `DomainCommand`. `DomainCommand` and the types it holds carry no serde derives. In the wire format ids are integers, amounts are integer minor units in the file's currency, dates are ISO `YYYY-MM-DD` text, and a `Compound` is a nested array. `balanced_store` holds only the text and never parses it.

Decided in [Undo log in the data file](https://github.com/StanleyGoldman/balanced/issues/228) (Q1, A1), recorded by the spec [#236](https://github.com/StanleyGoldman/balanced/issues/236).

## Consequences

- The wire format is part of the schema version. Adding a Command or an optional field keeps it. Any other change goes through a migration that clears the log (#230 Q5), and the old entries are never rewritten.
- Renaming or restructuring a `DomainCommand` variant or field leaves stored entries readable: only its mapping to the wire type changes. An `insta` snapshot of every variant's JSON fails on any change to the format itself.
- Every new `DomainCommand` variant needs a wire variant and both mappings. The mappings match exhaustively, so the compiler names each one.
- Amounts don't need `rust_decimal`'s `serde` feature.
- An entry that fails to decode on open (unknown Command, bad date, a split whose lines overflow) makes the file Damaged, and open refuses it.
- The log can be read with any SQLite tool, through SQLite's JSON functions.

## Considered options

- **Derive `serde` on `DomainCommand` and store it directly:** rejected. Every rename in the domain would silently change the stored format, and the domain types would carry persistence concerns.
- **One table per Command variant:** rejected. `RestoreAccount` and `Compound` nest other values, so the tables would be many and deep.
- **A binary format:** rejected. It can't be read or checked in SQL.
