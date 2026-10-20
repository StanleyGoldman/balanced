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
menu-file-new-data-file = New Data File…
menu-file-open-data-file = Open Data File…
menu-file-quit = Quit
# A Recent file in the File menu. $number counts from 1 at the newest, and $file is
# the file's name, such as Household.balanced.
menu-file-recent = { $number } { $file }
menu-edit = Edit
menu-edit-undo = Undo
menu-edit-redo = Redo
menu-edit-go-to-transfer = Go to Transfer
menu-view = View
menu-view-switch-pane = Switch Pane
menu-tools = Tools
menu-tools-manage-accounts = Manage Accounts…
menu-tools-preferences = Preferences…
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
account-bar-reconcile = Reconcile
account-bar-manage-accounts = Manage Accounts…
# The button in the Account bar's footer that opens the New Account wizard.
account-bar-new-account = + Add Account

## Register

register-column-date = Date
# The cheque or reference number.
register-column-number = Num
register-column-payee = Payee
register-column-category = Category
register-column-memo = Memo
# The headings of the columns of money out and money in on a checking, savings,
# investment or retirement account.
register-column-payment = Payment
register-column-deposit = Deposit
# The headings of the columns of money out and money in on a credit card.
register-column-charge = Charge
register-column-card-payment = Payment
# The headings of the columns of money out and money in on an asset or a liability.
register-column-decrease = Decrease
register-column-increase = Increase
register-column-balance = Balance
# The column showing how far each transaction has been checked against the bank: its
# Cleared status.
register-column-clr = Clr
# The column showing which transactions are flagged to come back to.
register-column-flag = Flag
# The handle on a column heading's right edge that resizes the column.
register-resize-column = Resize { $column }
# The ⚙ button at the right end of the Register header, which opens the column chooser.
register-column-chooser = Choose columns
# The column chooser's buttons that move a column one place up or down its list.
register-column-move-up = Move { $column } up
register-column-move-down = Move { $column } down
# The Category cell of a transaction divided between several categories.
register-split = [Split]
# The Payee cell of an account's first transaction, its starting balance.
register-opening-balance = Opening Balance
# The menu of commands on a Register row.
register-actions = Actions
# The Actions menu item that asks to delete the row's transaction.
register-actions-delete = Delete…
# The Actions menu items that flag the row's transaction, or take its flag off.
register-actions-flag = Flag
register-actions-unflag = Unflag
# The Actions menu item on one side of a transfer that opens the other side's Register,
# naming the other account as the Category cell does: Go to [Savings].
register-actions-go-to = Go to { $account }
# A Flag cell, which flags or unflags its transaction, as assistive technology reads it.
register-flag = Flag { $payee }, { $date }, { $amount }
# A Clr cell, which marks its transaction Cleared or Uncleared, or asks before setting
# a Reconciled one back to Cleared, as assistive technology reads it.
register-clr = Clr { $payee }, { $date }, { $amount }
# The Cleared statuses, as assistive technology reads a Clr cell's value. A Clr cell
# shows each status's .mark, and an Uncleared transaction's cell is blank.
register-clr-uncleared = Uncleared
register-clr-cleared = Cleared
    .mark = c
register-clr-reconciled = Reconciled
    .mark = R
# The ⏷ button beside the Category cell being edited, which lists every category.
register-category-list = Show all categories
# The row of the Category list that creates the category typed in the cell.
register-create-category = Create "{ $name }"…
register-new-transaction = + Transaction
# The context header's menu button holding the open account's tasks, and its items.
register-account-actions = Account Actions ⚙
register-account-actions-reconcile = Reconcile…
register-account-actions-edit-account-details = Edit account details…
register-account-actions-register-preferences = { -register } preferences…
register-account-actions-print-register = Print { -register }…
register-account-actions-export-to-csv = Export to CSV…
# The context header's balances, each with its formatted amount: Current counts the
# transactions dated today or earlier, Ending counts every one, the future-dated ones too,
# and Cleared counts the Cleared and Reconciled ones, whatever their date.
register-current-balance = Current { $amount }
register-ending-balance = Ending { $amount }
register-cleared-balance = Cleared { $amount }
# Why the draft row at the bottom of the Register wasn't added, in a bubble below
# the first empty cell the transaction needs.
register-draft-date-missing = Date is missing
register-draft-category-missing = Category is missing
register-draft-amount-missing = Amount is missing
# The line between the Register's rows dated today or earlier and its rows dated
# later, labelled with today's date.
register-today-divider = Today · { $date }
# Beside a closed account's name, above its Register.
register-closed-read-only = Closed — read-only

# The filter bar above the Register. Its controls show no label, so screen readers
# read these names.
register-filter-search = Search
    .placeholder = Search payee, category, memo
register-filter-dates = Dates
register-filter-dates-all = All dates
register-filter-dates-last-30-days = Last 30 days
register-filter-dates-last-12-months = Last 12 months
register-filter-dates-this-year = This year
register-filter-dates-last-year = Last year
register-filter-type = Transaction type
register-filter-type-all = All transactions
register-filter-type-payments = Payments
register-filter-type-deposits = Deposits
register-filter-type-flagged = Flagged
register-filter-reset = Reset
# How many of the account's transactions the filter bar lets the Register show.
register-filter-count = { $shown } of { $total } shown
register-transaction-count =
    { $count ->
        [one] { $count } transaction
       *[other] { $count } transactions
    }

## Dialogs

dialog-cancel = Cancel
# The screen reader's name for the × at the right of a heavy Dialog's title bar.
dialog-close = Close dialog
# Asks before deleting the transaction the payee, date and amount describe.
dialog-delete-transaction-message = Delete this transaction? { $payee }, { $date }, { $amount }
# Asks before deleting a reconciled transaction, which the payee, date and amount
# describe.
dialog-delete-reconciled-transaction-message = This transaction is reconciled. Delete it anyway? { $payee }, { $date }, { $amount }
dialog-delete-transaction-delete = Delete
# Asks before saving a new amount, $new, on the reconciled transaction the payee,
# date and amount describe.
dialog-reconciled-amount-message = This transaction is reconciled. Change its amount to { $new }? { $payee }, { $date }, { $amount }
dialog-reconciled-amount-change = Change Amount
# Asks before setting the Reconciled transaction the payee, date and amount describe
# back to Cleared.
dialog-unreconcile-message = Un-reconcile this transaction? It has been matched against a statement, and will be marked Cleared. { $payee }, { $date }, { $amount }
dialog-unreconcile-accept = Mark Cleared
dialog-ok = OK
dialog-new-category-title = New Category
dialog-new-category-name = Name
# Below the Name field. A colon in the name puts the category under the one the
# text before it names.
dialog-new-category-hint = Use “:” to put it under another category, as in Home:Garden.
# $parent is the text before the last colon of the typed name.
dialog-new-category-no-parent = No category is named “{ $parent }”.
dialog-preferences-title = Preferences
# The groups listed down the left of the Preferences Dialog.
dialog-preferences-group-display = Display
dialog-preferences-scheme = Theme
dialog-preferences-scheme-light = Light
dialog-preferences-scheme-dark = Dark
dialog-preferences-font-face = Font
# The font egui, the toolkit Balanced is drawn with, uses by default.
dialog-preferences-font-face-egui = Standard
dialog-preferences-font-face-noto-sans = Noto Sans
dialog-preferences-font-face-liberation-sans = Liberation Sans
dialog-preferences-dense-rows = Dense rows
dialog-preferences-monospace-money = Line up amounts in a fixed-width font
dialog-preferences-coloured-negatives = Show negative amounts in red
dialog-preferences-group-register = { -register }
dialog-preferences-alternating-rows = Alternating rows
# The line in the Register between today's rows and the rows dated later.
dialog-preferences-today-divider = Today divider
# The rules drawn between the Register's cells.
dialog-preferences-gridlines = Gridlines
dialog-preferences-gridlines-off = Off
dialog-preferences-gridlines-horizontal = Horizontal
dialog-preferences-gridlines-vertical = Vertical
dialog-preferences-gridlines-both = Both
dialog-preferences-group-account-bar = { -account-bar }
# Each group heading in the Account bar shows the sum of its accounts' balances.
dialog-preferences-group-totals = Show group totals
dialog-preferences-net-worth = Show Net Worth
dialog-preferences-net-worth-account-bar = In the { -account-bar }'s footer
dialog-preferences-net-worth-status-bar = In the Status bar

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

# Why a data file wasn't opened or created. $file is the file's name, such as
# Household.balanced. The data file open before stays open, or the welcome Dialog
# follows when none is.
dialog-data-file-missing = { -brand-name } can't find “{ $file }”. It may have been moved, renamed or deleted.
dialog-data-file-not-balanced = “{ $file }” isn't a { -brand-name } data file.
dialog-data-file-damaged = “{ $file }” is damaged. Restore it from a backup.
dialog-data-file-newer = “{ $file }” was made by a newer version of { -brand-name }. Update { -brand-name } to open it.
dialog-data-file-locked = “{ $file }” is open in another program. Close it there, then try again.
dialog-data-file-backup-failed = { -brand-name } couldn't make a backup copy of “{ $file }” before upgrading it, so it was left as it was. Check that its folder has room and lets you save files there, then try again.
# $backup is the name of the copy made beside the file before the upgrade began.
dialog-data-file-upgrade-failed = { -brand-name } couldn't upgrade “{ $file }”, so it was left as it was. A backup copy made before the upgrade is beside it, named “{ $backup }”.
dialog-data-file-unreadable = { -brand-name } couldn't read “{ $file }”.
dialog-data-file-not-created = { -brand-name } couldn't create “{ $file }”.
# New Data File… picked the data file that is open.
dialog-data-file-open-not-replaced = “{ $file }” is the data file you have open, so a new one can't replace it.
# The name the save picker suggests for a new data file.
dialog-data-file-suggested-name = { -brand-name }.balanced
# The file type the native file picker lists, for files ending in .balanced.
dialog-data-file-filter = { -brand-name } data files

# The welcome Dialog, shown while no data file is open: at first run, or when the data
# file opened last can't be opened. Continue shows the picker for the choice made.
dialog-welcome-message = { -brand-name } keeps your finances in a data file.
dialog-welcome-create = Create a new data file…
dialog-welcome-open = Open an existing data file…
dialog-welcome-continue = Continue
dialog-welcome-quit = Quit

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
status-transfer-account-closed = The other account of this transfer is closed, so the transfer can't be made, changed or deleted.
status-transfer-to-own-account = A transfer moves money to another account. Pick an account other than this one.
status-split-line-other-side = This is the other side of a split line. Change its date, amount or category, or delete it, from the split.
# $account is the name of the account holding the split.
status-split-line-other-side-in = This is the other side of a split line. Change its date, amount or category, or delete it, from the split in { $account }.
status-account-has-transfers = An account that holds a transfer can't be deleted, as the other account would keep its side. Close the account instead.
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
# The data file refused a change, so Balanced didn't make it.
status-change-not-saved = That change couldn't be saved to the data file, so it wasn't made.
status-preferences-reset = Your preferences couldn't be read, so the defaults are in use. The old file is kept as preferences.toml.broken.
status-preferences-unreadable = Your preferences couldn't be read, so the defaults are in use.
status-preferences-not-saved = Your preferences couldn't be saved, so they last only until you quit.
