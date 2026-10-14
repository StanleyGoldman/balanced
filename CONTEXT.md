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

**Register tab**:
A closeable tab above the main view holding one open register, so several registers stay open at once. Used when the user's Preference opens accounts in tabs.
_Avoid_: Document tab, account tab

**Preference**:
A choice about how Balanced looks or behaves that belongs to the person using this computer and applies to every data file they open (e.g. dense rows, register tabs). Set in Tools → Preferences.
_Avoid_: Application-level option, setting, option

### Finance

**Register**:
The editable ledger of transactions for one account, shown in the main view.
_Avoid_: Transaction list, ledger view
