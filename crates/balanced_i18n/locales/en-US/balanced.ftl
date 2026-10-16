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
menu-file-new-account = New Account…
menu-file-close-register-tab = Close { -register } Tab
menu-file-quit = Quit
menu-edit = Edit
menu-edit-undo = Undo
menu-edit-redo = Redo
menu-view = View
menu-view-next-register-tab = Next { -register } Tab
menu-view-previous-register-tab = Previous { -register } Tab
menu-view-switch-pane = Switch Pane
menu-tools = Tools
menu-tools-manage-accounts = Manage Accounts…
menu-help = Help
menu-help-about = About { -brand-name }
menu-new-transaction = New Transaction
menu-delete-transaction = Delete Transaction

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

## Account kinds

account-kind-checking = Checking
account-kind-savings = Savings
account-kind-credit-card = Credit Card
account-kind-investment = Investment
# Retirement savings, such as a 401(k) or an IRA.
account-kind-retirement = Retirement
# Property, such as a house or a car.
account-kind-asset = Asset
# Debt, such as a mortgage or a loan.
account-kind-liability = Liability

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
account-bar-reconcile = Reconcile
account-bar-manage-accounts = Manage Accounts…
# The button in the Account bar's header that opens the New Account wizard.
account-bar-new-account = + Account

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
# The Actions menu item that asks to delete the row's transaction.
register-actions-delete = Delete…
# The ⏷ button beside the Category cell being edited, which lists every category.
register-category-list = Show all categories
# The row of the Category list that creates the category typed in the cell.
register-create-category = Create "{ $name }"…
register-new-transaction = + Transaction
# Why the draft row at the bottom of the Register wasn't added, in a bubble below
# the first empty cell the transaction needs.
register-draft-date-missing = Date is missing
register-draft-category-missing = Category is missing
register-draft-amount-missing = Amount is missing
# Beside a closed account's name, above its Register.
register-closed-read-only = Closed — read-only
register-transaction-count =
    { $count ->
        [one] { $count } transaction
       *[other] { $count } transactions
    }

## Dialogs

dialog-cancel = Cancel
# Asks before deleting the transaction the payee, date and amount describe.
dialog-delete-transaction-message = Delete this transaction? { $payee }, { $date }, { $amount }
dialog-delete-transaction-delete = Delete
dialog-ok = OK
dialog-new-category-title = New Category
dialog-new-category-name = Name
# Below the Name field. A colon in the name puts the category under the one the
# text before it names.
dialog-new-category-hint = Use “:” to put it under another category, as in Home:Garden.
# $parent is the text before the last colon of the typed name.
dialog-new-category-no-parent = No category is named “{ $parent }”.

# Manage Accounts: a table of every account, with what can be changed about each.
dialog-manage-accounts-title = Manage Accounts
dialog-manage-accounts-name = Name
dialog-manage-accounts-type = Type
dialog-manage-accounts-balance = Balance
dialog-manage-accounts-hidden = Hidden
# The Hidden checkbox of the account named $name, for screen readers.
dialog-manage-accounts-hide = Hide { $name }
dialog-manage-accounts-status = Status
# Beside the type of a closed account.
dialog-manage-accounts-closed = closed
dialog-manage-accounts-close = Close…
    .a11y = Close { $name }…
dialog-manage-accounts-reopen = Reopen
    .a11y = Reopen { $name }
dialog-manage-accounts-delete = Delete…
    .a11y = Delete { $name }…
# The buttons that move an account up and down within its sub-group, for screen
# readers. On screen they are arrows.
dialog-manage-accounts-move-up = Move { $name } up
dialog-manage-accounts-move-down = Move { $name } down
dialog-close-account-message = Close { $name }? Its { -register } becomes read-only. You can reopen it later.
# The close confirm for an account whose balance isn't zero.
dialog-close-account-message-balance = Close { $name }? It still has a balance of { $amount }. Its { -register } becomes read-only. You can reopen it later.
dialog-close-account-close = Close
dialog-delete-account-message = Delete { $name } and every transaction in it?
dialog-delete-account-delete = Delete

# The New Account wizard. Each step's heading says how far through it is.
dialog-new-account-step-type = Step 1 of 4: Type
dialog-new-account-step-details = Step 2 of 4: Details
dialog-new-account-step-opening-balance = Step 3 of 4: Opening balance
dialog-new-account-step-review = Step 4 of 4: Review
dialog-new-account-back = Back
dialog-new-account-next = Next
dialog-new-account-finish = Finish
dialog-new-account-name = Name
dialog-new-account-balance = Opening balance
# The date of the opening balance.
dialog-new-account-date = As of
# The Review step, one line per choice. $kind is the account type.
dialog-new-account-review-type = Type: { $kind }
dialog-new-account-review-name = Name: { $name }
# $amount is the opening balance, $date the day it is as of.
dialog-new-account-review-balance = Opening balance: { $amount } as of { $date }

## Amount field

# Committing an Amount cell with nothing in it.
field-amount-empty = Enter an amount.
# Typed text that isn't an amount written the formatting locale's way. $decimal is
# the decimal mark and $group the separator between thousands, such as "." and ",".
field-amount-invalid = Write amounts with “{ $decimal }” as the decimal mark and “{ $group }” between thousands.
# $currency is a currency code such as USD.
field-amount-too-many-decimal-places =
    { $places ->
        [0] { $currency } amounts have no decimal places
        [one] { $currency } amounts have 1 decimal place
       *[other] { $currency } amounts have { $places } decimal places
    }
field-amount-out-of-range = That amount is too large.
field-amount-division-by-zero = Can't divide by zero.
# The result of arithmetic typed in an Amount cell, shown while typing.
field-amount-preview = = { $amount }
# The same, when the result was rounded to the currency's decimal places.
field-amount-preview-rounded = ~ { $amount }

## Date field

# The letters standing for each part of a date in a date pattern, such as M/D/YYYY.
# The day and month letters are doubled where the locale pads them (DD.MM.YYYY), and
# the year letter is written four times.
field-date-day-letter = D
field-date-month-letter = M
field-date-year-letter = Y
# $pattern is the date pattern (M/D/YYYY); $example is today's date in that pattern.
field-date-invalid = Not a date. Expected { $pattern }, for example { $example }.
field-date-calendar = Calendar
    .tooltip = Calendar (Alt+Down)
field-date-previous-month = Previous month
field-date-next-month = Next month

## Status bar

status-about = { -brand-name } { $version }
status-not-available-yet = Not available yet
status-no-such-category = No category has that name. Pick one from the list, or create it.
status-account-closed = This account is closed, so its transactions can't be changed.
# A change to a transaction that no longer exists, such as one just deleted.
status-unknown-transaction = That transaction no longer exists.
status-blank-account-name = Enter a name for the account.
status-sub-group-edge = That account can't move any further in its group.
status-opening-balance-payee = The opening balance has no payee.
# A change to an account that no longer exists, such as one just deleted.
status-unknown-account = That account no longer exists.
status-amount-too-large = That change would make a balance too large to hold.
status-not-in-one-category = Only a transaction in one category can change its category.
status-unknown-category = That category no longer exists.
status-split-amount = A split transaction's amount is the sum of its lines.
status-currency-mismatch = The amount is in another currency than the account.
status-duplicate-category = A category named “{ $name }” is already there.
# Deleting a category that a transaction or another category still uses.
status-category-in-use = That category is still in use, so it can't be deleted.
status-blank-category-name = Enter a name for the category.
