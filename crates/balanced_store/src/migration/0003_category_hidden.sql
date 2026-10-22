-- Version 3: whether each category is hidden from the Category field's list. Every
-- category starts shown.
ALTER TABLE category ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0
    CONSTRAINT category_hidden_flag CHECK (hidden IN (0, 1));
