-- Version 2: each category's type, Income or Expense. `Ledger` keeps a subcategory's
-- type equal to its top-level category's.
ALTER TABLE category ADD COLUMN type TEXT NOT NULL DEFAULT 'expense'
    CONSTRAINT category_type_known CHECK (type IN ('income', 'expense'));

-- A top-level category is Income when the transactions and split lines in it and every
-- category under it add up to more than zero, and Expense otherwise, unused ones
-- included. `total` adds in floating point, so a sum too large for an integer still has
-- a sign instead of failing the upgrade.
WITH RECURSIVE
    tree (id, top_id) AS (
        SELECT id, id FROM category WHERE parent_id IS NULL
        UNION ALL
        SELECT c.id, t.top_id FROM category AS c JOIN tree AS t ON c.parent_id = t.id
    ),
    used (category_id, amount) AS (
        SELECT category_id, amount FROM "transaction" WHERE allocation = 'category'
        UNION ALL
        SELECT category_id, amount FROM split_line WHERE category_id IS NOT NULL
    ),
    top_type (top_id, type) AS (
        SELECT t.top_id, CASE WHEN total(u.amount) > 0 THEN 'income' ELSE 'expense' END
        FROM tree AS t
        LEFT JOIN used AS u ON u.category_id = t.id
        GROUP BY t.top_id
    )
UPDATE category
SET type = (
    SELECT tt.type FROM tree AS t JOIN top_type AS tt ON tt.top_id = t.top_id
    WHERE t.id = category.id
);
