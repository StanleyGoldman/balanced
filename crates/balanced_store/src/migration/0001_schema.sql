-- Version 1: the whole schema. Edited in place until the first release.
--
-- Built-in SQL only, so any SQLite tool can read the file. Every foreign key is
-- ON DELETE RESTRICT: the store deletes child rows itself. Ids are assigned by `Ledger`
-- and inserted explicitly; AUTOINCREMENT keeps the highest id ever used in
-- `sqlite_sequence`, so a deleted row's id is never reused.

-- The file itself: one row, holding the currency every amount is in.
CREATE TABLE data_file (
    id INTEGER PRIMARY KEY CONSTRAINT data_file_one_row CHECK (id = 1),
    currency TEXT NOT NULL CONSTRAINT data_file_currency_code CHECK (currency GLOB '[A-Z][A-Z][A-Z]')
) STRICT;

-- An account. `position` orders the accounts of one `kind`, densely from 0. It has no
-- lower bound so that a move can park a row outside the range while it swaps two.
CREATE TABLE account (
    id INTEGER PRIMARY KEY AUTOINCREMENT
        CONSTRAINT account_id_u32 CHECK (id BETWEEN 0 AND 4294967295),
    name TEXT NOT NULL CONSTRAINT account_name_not_blank CHECK (trim(name) <> ''),
    kind TEXT NOT NULL CONSTRAINT account_kind_known CHECK (kind IN (
        'checking', 'savings', 'credit_card', 'investment', 'retirement', 'asset', 'liability'
    )),
    position INTEGER NOT NULL,
    hidden INTEGER NOT NULL CONSTRAINT account_hidden_flag CHECK (hidden IN (0, 1)),
    closed INTEGER NOT NULL CONSTRAINT account_closed_flag CHECK (closed IN (0, 1)),
    CONSTRAINT account_kind_position UNIQUE (kind, position)
) STRICT;

-- A category, under `parent_id` or at the top of the tree. `Ledger` compares sibling
-- names after Unicode case folding; the unique index below catches ASCII case only.
CREATE TABLE category (
    id INTEGER PRIMARY KEY AUTOINCREMENT
        CONSTRAINT category_id_u32 CHECK (id BETWEEN 0 AND 4294967295),
    parent_id INTEGER REFERENCES category (id) ON DELETE RESTRICT,
    name TEXT NOT NULL
        CONSTRAINT category_name_not_blank CHECK (trim(name) <> '')
        CONSTRAINT category_name_trimmed CHECK (name = trim(name))
) STRICT;

CREATE UNIQUE INDEX category_sibling_name ON category (coalesce(parent_id, -1), name COLLATE NOCASE);
CREATE INDEX category_parent_id ON category (parent_id);

-- A transaction in one account. `amount` is in minor units. A split's amount is the sum
-- of its lines, so it stores neither an amount nor a category. Transactions on one date
-- are ordered by id.
CREATE TABLE "transaction" (
    id INTEGER PRIMARY KEY AUTOINCREMENT
        CONSTRAINT transaction_id_u32 CHECK (id BETWEEN 0 AND 4294967295),
    account_id INTEGER NOT NULL REFERENCES account (id) ON DELETE RESTRICT,
    date TEXT NOT NULL CONSTRAINT transaction_date_iso CHECK (date(date) IS date),
    number TEXT CONSTRAINT transaction_number_not_empty CHECK (number <> ''),
    payee TEXT NOT NULL,
    memo TEXT NOT NULL,
    allocation TEXT NOT NULL
        CONSTRAINT transaction_allocation_known CHECK (allocation IN ('opening_balance', 'category', 'split')),
    amount INTEGER,
    category_id INTEGER REFERENCES category (id) ON DELETE RESTRICT,
    CONSTRAINT transaction_amount_unless_split CHECK ((allocation = 'split') = (amount IS NULL)),
    CONSTRAINT transaction_category_when_category CHECK ((allocation = 'category') = (category_id IS NOT NULL)),
    CONSTRAINT transaction_opening_balance_no_payee CHECK (allocation <> 'opening_balance' OR payee = '')
) STRICT;

CREATE UNIQUE INDEX transaction_one_opening_balance ON "transaction" (account_id)
    WHERE allocation = 'opening_balance';
CREATE INDEX transaction_register_order ON "transaction" (account_id, date, id);
CREATE INDEX transaction_category_id ON "transaction" (category_id);

-- One line of a split transaction, in the order the split lists them. The primary key
-- also serves as the index on `transaction_id`.
CREATE TABLE split_line (
    transaction_id INTEGER NOT NULL REFERENCES "transaction" (id) ON DELETE RESTRICT,
    position INTEGER NOT NULL,
    category_id INTEGER NOT NULL REFERENCES category (id) ON DELETE RESTRICT,
    memo TEXT NOT NULL,
    amount INTEGER NOT NULL,
    PRIMARY KEY (transaction_id, position)
) STRICT;

CREATE INDEX split_line_category_id ON split_line (category_id);

-- The views below are the test oracle for the numbers `Ledger` derives. Every SUM raises
-- "integer overflow" rather than wrapping.

-- Each transaction's amount: its own, or the sum of its split lines.
CREATE VIEW transaction_amount AS
SELECT
    t.id AS transaction_id,
    t.account_id,
    CASE t.allocation
        WHEN 'split' THEN (SELECT coalesce(sum(s.amount), 0) FROM split_line AS s WHERE s.transaction_id = t.id)
        ELSE t.amount
    END AS amount
FROM "transaction" AS t;

-- Each transaction with the account's balance once it and every earlier one have been
-- counted, in register order: by date, then id.
CREATE VIEW register_entry AS
SELECT
    t.account_id,
    t.id AS transaction_id,
    t.date,
    a.amount,
    sum(a.amount) OVER (
        PARTITION BY t.account_id ORDER BY t.date, t.id ROWS UNBOUNDED PRECEDING
    ) AS balance
FROM "transaction" AS t
JOIN transaction_amount AS a ON a.transaction_id = t.id;

-- Each account's balance, 0 for an account with no transactions.
CREATE VIEW account_balance AS
SELECT ac.id AS account_id, coalesce(sum(a.amount), 0) AS balance
FROM account AS ac
LEFT JOIN transaction_amount AS a ON a.account_id = ac.id
GROUP BY ac.id;

-- Each Account bar group's total over its open accounts, hidden ones included. Every
-- group has a row, 0 when it has no open accounts.
CREATE VIEW group_total AS
WITH kind_group (kind, account_group) AS (
    VALUES
        ('checking', 'banking'), ('savings', 'banking'), ('credit_card', 'banking'),
        ('investment', 'investing'), ('retirement', 'investing'),
        ('asset', 'property_debt'), ('liability', 'property_debt')
)
SELECT g.account_group, coalesce(sum(b.balance), 0) AS total
FROM (SELECT DISTINCT account_group FROM kind_group) AS g
JOIN kind_group AS k ON k.account_group = g.account_group
LEFT JOIN account AS ac ON ac.kind = k.kind AND ac.closed = 0
LEFT JOIN account_balance AS b ON b.account_id = ac.id
GROUP BY g.account_group;

-- The total of every closed account, whatever its group.
CREATE VIEW closed_total AS
SELECT coalesce(sum(b.balance), 0) AS total
FROM account AS ac
JOIN account_balance AS b ON b.account_id = ac.id
WHERE ac.closed = 1;

-- The total of every account: hidden and closed ones included.
CREATE VIEW net_worth AS
SELECT coalesce(sum(balance), 0) AS total
FROM account_balance;

-- The undo log: the inverses of the changes Undo and Redo can make, as `Ledger` holds
-- them on its two stacks. Each stack's latest entry has its highest `position`. `command`
-- is a Command in `balanced_core`'s JSON wire format, which this schema version fixes.
CREATE TABLE undo_entry (
    stack TEXT NOT NULL CONSTRAINT undo_entry_stack_known CHECK (stack IN ('undo', 'redo')),
    position INTEGER NOT NULL,
    command TEXT NOT NULL CONSTRAINT undo_entry_command_json CHECK (json_valid(command)),
    CONSTRAINT undo_entry_stack_position UNIQUE (stack, position)
) STRICT;
