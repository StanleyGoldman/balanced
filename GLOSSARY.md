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

**Preference**:
A choice about how Balanced looks or behaves that belongs to the person using this computer and applies to every data file they open (e.g. dense rows, gridlines). Set in Tools → Preferences.
_Avoid_: Application-level option, setting, option

**Data file**:
The one file that holds a household's finances: its accounts, transactions and undo history. Balanced has exactly one data file open at a time, and every change is kept in it the moment it's made.
_Avoid_: Database, document, project, Quicken file

**Recent files**:
The data files the person using this computer has opened lately, newest first, listed in the File menu. The newest one opens at startup. Recent files are a record, not a Preference.
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

**Actions column**:
The untitled last column of a register, holding a menu of commands for its row (e.g. Split, Void, Delete).
_Avoid_: Row menu, context column

**Column chooser**:
The control at the right end of a register's header that shows, hides and reorders the register's columns.
_Avoid_: Column picker, column settings

**Filter bar**:
The row above a register's column headings that narrows the rows it shows by search text, date range and transaction type; it never changes a row's running balance.
_Avoid_: Search bar, filter panel
