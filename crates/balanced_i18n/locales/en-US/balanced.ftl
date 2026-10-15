# Balanced's user-interface text in en-US, the source locale.
# `fl!` checks message ids and arguments in code against this file, and the
# locale-consistency test checks every other locale against it.

-brand-name = Balanced
-account-bar = Account bar
-register = Register

## Main window

main-window-title = { -brand-name }

## Menu bar

menu-file = File
menu-file-close-register-tab = Close { -register } Tab
menu-file-quit = Quit
menu-edit = Edit
menu-view = View
menu-view-next-register-tab = Next { -register } Tab
menu-view-previous-register-tab = Previous { -register } Tab
menu-tools = Tools
menu-help = Help
menu-help-about = About { -brand-name }

# Modifier key names in a menu item's Shortcut text, such as Ctrl+Q.
menu-modifier-ctrl = Ctrl
menu-modifier-alt = Alt
menu-modifier-shift = Shift
# The macOS Command key.
menu-modifier-cmd = Cmd
# The macOS Option key.
menu-modifier-option = Option

## Tab row

tab-home = Home
tab-home-placeholder = Your overview of accounts, bills and spending will appear here.
tab-accounts = Accounts
tab-accounts-placeholder = Select an account in the { -account-bar } to open its { -register }.
tab-bills = Bills
tab-bills-placeholder = Upcoming bills and deposits will appear here.
tab-spending = Spending
tab-spending-placeholder = Where your money goes will appear here.
tab-planning = Planning
tab-planning-placeholder = Budgets and savings goals will appear here.
tab-investing = Investing
tab-investing-placeholder = Your investment accounts and holdings will appear here.
tab-property-debt = Property & Debt
tab-property-debt-placeholder = Your assets, loans and other debts will appear here.
# The × on a Register tab, named for the account the tab shows.
tab-register-close = Close { $account }

## Account bar

account-bar-title = Accounts
account-bar-total = Total: { $amount }
    .tooltip = Sum of the balances of the accounts shown in the { -account-bar }
account-bar-group-banking = Banking
account-bar-group-investing = Investing
account-bar-group-property-debt = Property & Debt
account-bar-group-closed = Closed
account-bar-net-worth = Net Worth
# The buttons that fold the Account bar to a narrow rail and open it again.
account-bar-fold = Fold the { -account-bar }
account-bar-unfold = Unfold the { -account-bar }
# An account's right-click menu.
account-bar-open-register = Open { -register }
account-bar-open-in-new-tab = Open in New Tab

## Register

register-column-date = Date
# The cheque or reference number.
register-column-number = Num
register-column-payee = Payee
register-column-category = Category
register-column-memo = Memo
register-column-amount = Amount
register-column-balance = Balance
# The handle on a column heading's right edge that resizes the column.
register-resize-column = Resize { $column }
# The Category cell of a transaction divided between several categories.
register-split = [Split]
# The Payee cell of an account's first transaction, its starting balance.
register-opening-balance = Opening Balance
# The menu of commands on a Register row.
register-actions = Actions
register-new-transaction = + Transaction
# Beside a closed account's name, above its Register.
register-closed-read-only = Closed — read-only
register-transaction-count =
    { $count ->
        [one] { $count } transaction
       *[other] { $count } transactions
    }

## Status bar

status-about = { -brand-name } { $version }
status-not-available-yet = Not available yet
status-account-closed = This account is closed, so its transactions can't be changed.
# A change to a transaction that no longer exists, such as one just deleted.
status-unknown-transaction = That transaction no longer exists.
status-opening-balance-payee = The opening balance has no payee.
