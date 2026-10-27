# Balanced

A desktop personal-finance application in the tradition of Quicken classic: one main window with a menu bar, an account list, and account registers.

## Language

### Application shell

**Main window**:
The single primary application window that hosts the menu bar, account bar, main view, and status bar.
_Avoid_: Root window, app window

**Menu bar**:
The traditional File/Edit/View/… menu strip along the top edge of the main window.

**Account bar**:
The panel along the left edge of the main window listing the user's accounts, grouped and with balances.
_Avoid_: Sidebar, account list, nav panel

**Status bar**:
The strip along the bottom edge of the main window showing transient status and summary information.

**Main view**:
The central area of the main window, showing whatever the account bar or top-level tabs have navigated to.
_Avoid_: Content pane, workspace

**Detached window**:
A long-lived secondary OS window that can move independently of the main window (e.g. a report or reconcile window).
_Avoid_: Popup, child window, floating window

**Dialog**:
A short-lived window for a single task, usually modal, that blocks the main window until dismissed (e.g. confirm, New Account, split).
_Avoid_: Popup, modal window

**Tool window**:
A long-lived, non-modal window drawn inside the main window, over the main view, that stays open while the user works beside it (e.g. the Calculator).
_Avoid_: Floating window, palette, popup

**Preference**:
A choice about how Balanced looks or behaves that belongs to the person using this computer and applies to every data file they open (e.g. dense rows, gridlines). Set in Tools → Preferences.
_Avoid_: Application-level option, setting, option

**Data file**:
The one file that holds a household's finances: its accounts, transactions and undo history. Balanced has exactly one data file open at a time, and every change is kept in it the moment it's made.
_Avoid_: Database, document, project, Quicken file

**Recent files**:
The data files the person using this computer has opened lately, newest first, listed in the File menu. The newest one opens at startup. Each entry also remembers where the person left that file: the top-level tab and the account open when it last closed, which it reopens on. Recent files are a record, not a Preference.
_Avoid_: MRU list, history

**Shortcut**:
A key combination that runs a command from anywhere in the main window, shown beside its menu item (e.g. Ctrl+D deletes a transaction).
_Avoid_: Hotkey, accelerator, keybinding

### Finance

**Register**:
The editable ledger of transactions for one account, shown in the main view.
_Avoid_: Transaction list, ledger view

**Draft row**:
The empty row at the bottom of a register where the user types a new transaction; it becomes a transaction when added.
_Avoid_: New row, blank row, entry row

**Editing row**:
The register row being changed. Its changes are kept back until the row is saved, and Esc throws them away. It holds the Save, Cancel, Split and More actions buttons; the draft row has Add in place of Save. Save and Cancel stay disabled until a value in the row changes.
_Avoid_: Open row, active row, entry row

**Leave-row prompt**:
The Dialog that asks, as a changed editing row is left, whether to save it (Add on the draft row), continue editing it, or discard its changes. The "Save a changed transaction without asking" Preference skips it and saves.
_Avoid_: Save prompt, unsaved-changes dialog

**Cell cursor**:
The outline marking the one register cell that keys act on. A row with the cell cursor but not being edited is highlighted.
_Avoid_: Selection, focus cell

**Split dialog**:
The Dialog where a transaction's split lines are edited. The lines stay pending until the row is saved.
_Avoid_: Split view, split panel

**Payee recall**:
Filling a draft row's empty cells from the latest transaction, in any account, with the payee just entered. A payee remembers only what its transactions say, so editing or deleting the latest one changes what it recalls.
_Avoid_: QuickFill, memorized payee, autofill

**Inline completion**:
The rest of the closest matching payee or category, shown selected in the cell as the user types, so Tab or Enter takes it and typing on replaces it.
_Avoid_: Autocomplete, type-ahead

**Today divider**:
The labelled line in a register sorted by date between the rows dated today or earlier and the first row dated later. It is not a row, so the cell cursor never rests on it.
_Avoid_: Today line, future separator

**Actions column**:
The untitled last column of a register, holding a menu of commands for its row (e.g. Split, Void, Delete).
_Avoid_: Row menu, context column

**Cleared status**:
Whether a transaction has shown up on the bank's records. It is one of three states: Uncleared, Cleared (`c`: the bank has it) or Reconciled (`R`: matched against a statement in Reconcile). It is shown in the register's Clr column.
_Avoid_: Clr flag, cleared flag, check state

**Cleared balance**:
The total of an account's Cleared and Reconciled transactions, whatever their date: what the bank should show once every cleared transaction has posted.
_Avoid_: Bank balance, statement balance

**Reconcile**:
Matching an account's Cleared transactions against a Statement until the Statement's ending balance equals the Cleared balance, then marking them Reconciled.
_Avoid_: Balance the account, match statement

**Statement**:
The bank's periodic record of an account, as far as Reconcile needs it: an ending date and an ending balance.
_Avoid_: Bank statement, period

**Balance adjustment**:
A transaction Reconcile posts, at the user's request, for whatever difference remains between a Statement and the Cleared balance.
_Avoid_: Correction, plug, fudge entry

**Column chooser**:
The control at the right end of a register's header that shows, hides and reorders the register's columns.
_Avoid_: Column picker, column settings

**Account Actions menu**:
The ⚙ menu at the right end of a register's context header that starts the open account's tasks: Reconcile, editing its details, register preferences, printing and exporting.
_Avoid_: Account settings, gear menu

**Filter bar**:
The row above a list that narrows the rows it shows.
In a register it sits above the column headings and narrows by search text, date range and transaction type; it never changes a row's running balance.
In the Category List it sits above the tree and narrows by Show, Search and Show hidden categories.
_Avoid_: Search bar, filter panel, filter row

**Transfer**:
Money moved between two of the household's accounts, recorded as a transaction in each account's register. It's picked by choosing `[Account]` in the Category field, of a transaction or of a split line.
_Avoid_: Journal entry

**Other side**:
The transaction a transfer records in the other account. The two sides share their date and amount, with the sign flipped; everything else belongs to each side. Each split line to an account has an other side of its own, which points back at the split and takes the line's memo; its date and amount change, and it is deleted, only from the split.
_Avoid_: Mirror transaction, linked transaction, counterpart

### Categories

**Category**:
What a transaction's money was spent on or came from, such as Groceries or Salary. Categories form a tree, written as a path with `:` between levels (`Food:Coffee`).
_Avoid_: Tag, class, envelope

**Subcategory**:
A category under another category, its parent. It takes its parent's category type.
_Avoid_: Child category, sub-item

**Category type**:
Whether a top-level category, and every subcategory under it, is Income or Expense.
_Avoid_: Category kind, category class

**Hidden category**:
A category left out of the Category field's choices while it stays on its transactions and in reports. Hiding a parent hides its subcategories, and showing a subcategory shows each hidden category above it, so no shown category sits under a hidden one.
_Avoid_: Archived category, inactive category

**Category List**:
The Dialog that lists every category with its usage and creates, edits, merges, hides and deletes them.
_Avoid_: Category manager, categories window

**Merge**:
Moving every transaction and split line of the chosen categories into one other category, optionally deleting the emptied ones.
_Avoid_: Combine, consolidate

**Reassign**:
Moving a deleted category's transactions and split lines to another category as part of the delete.
_Avoid_: Recategorize, remap
