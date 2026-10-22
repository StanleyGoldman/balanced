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
menu-edit-go-to-date = Go to Date…
menu-view = View
menu-view-switch-pane = Switch Pane
menu-tools = Tools
menu-tools-manage-accounts = Manage Accounts…
menu-tools-category-list = Category List…
menu-tools-preferences = Preferences…
menu-tools-calculator = Calculator
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

## Category types

# A category for money coming in, such as a salary.
category-type-income = Income
# A category for money going out, such as groceries.
category-type-expense = Expense

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
# The buttons in the Actions cell of the row being edited. Save keeps the row's changes,
# and on the row for a new transaction Add adds it in Save's place. Cancel throws the
# changes away, and Split opens the transaction's split lines.
register-row-save = Save
register-row-add = Add
register-row-cancel = Cancel
register-row-split = Split
# The button beside them that opens the row's menu of commands, shown as More ⏷, and
# as assistive technology reads it.
register-row-more = More
    .a11y = More actions
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
# Asks, as the user leaves a Register row with changes not yet saved, whether to save
# them.
dialog-leave-row-message = Save the changed transaction?
# Asks, as the user leaves the Register's filled-in new transaction row, whether to
# add it.
dialog-leave-row-draft-message = Add the new transaction?
dialog-leave-row-save = Save
dialog-leave-row-add = Add
# Stays on the row, keeping its changes.
dialog-leave-row-continue = Continue editing
# Throws the row's changes away.
dialog-leave-row-discard = Discard
# Sets the Preference dialog-preferences-save-without-asking names.
dialog-leave-row-always = Always save without asking
# Heads the Dialog that says why a Register row's save was refused. The reason
# follows below it.
dialog-cant-save-title = Can't save this transaction
# Stays on the row with the field at fault open, keeping the row's changes.
dialog-cant-save-continue = Continue editing
# Throws the row's changes away.
dialog-cant-save-discard = Discard changes
# Titles the Split dialog, where a Register row's split lines are edited.
dialog-split-title = Split transaction
# Closes the Split dialog, keeping its lines on the row until the row is saved.
dialog-split-done = Done
# Heads the column of each split line's amount.
dialog-split-amount = Amount
# What a screen reader reads for each field of split line $line, counting from 1.
dialog-split-line-category = Line { $line } category
dialog-split-line-memo = Line { $line } memo
dialog-split-line-amount = Line { $line } amount
dialog-split-add-line = Add line
# .a11y is what a screen reader reads for the Remove button of split line $line.
dialog-split-remove-line = Remove
    .a11y = Remove line { $line }
# Labels the amount the split lines must add up to.
dialog-split-total = Total
# Why Done didn't close the Split dialog: the lines add up to $lines, not $total,
# and $difference is how far apart they are.
dialog-split-mismatch = The lines add up to { $lines }, not to the total of { $total }. Difference: { $difference }
dialog-split-no-category = Line { $line } has no category.
dialog-split-unknown-category = Line { $line }: no category or account is named “{ $name }”.
# Why split line $line's amount can't be read; $error says what is wrong with it.
dialog-split-line-error = Line { $line }: { $error }
# Why the total can't be read, or the lines' sum is too large; $error says which.
dialog-split-total-error = Total: { $error }
dialog-split-no-lines = Give at least one line a category.
dialog-ok = OK
dialog-new-category-title = New Category
dialog-new-category-name = Name
# Below the Name field. A colon in the name puts the category under the one the
# text before it names.
dialog-new-category-hint = Use “:” to put it under another category, as in Home:Garden.
# $parent is the text before the last colon of the typed name.
dialog-new-category-no-parent = No category is named “{ $parent }”.
# The label before the Income and Expense choice, shown for a top-level category only.
dialog-new-category-type = Type
dialog-edit-category-title = Edit Category
dialog-edit-category-name = Name
# The picker of the category this one goes under.
dialog-edit-category-parent = Parent
# The Parent picker's choice that puts the category at the top of the tree.
dialog-edit-category-top-level = (Top level)
# The label before the Income and Expense choice, which only a top-level category takes.
dialog-edit-category-type = Type
dialog-go-to-date-title = Go to Date
# The label of Go to date's date field.
dialog-go-to-date-date = Date
# The button that moves the Register to the date.
dialog-go-to-date-go = Go
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
# Committing a new transaction's payee fills its other cells from the last
# transaction with that payee.
dialog-preferences-payee-recall = Fill in a new transaction from its payee's last one
# Typing the start of a payee or category fills in the rest of the first one that
# starts with it, selected, for Tab or Enter to take.
dialog-preferences-inline-completion = Complete payees and categories as you type
# Enter or Ctrl+Enter, saving a Register row, opens the next row's cell in the same
# column. When off, the saved row stays highlighted with no cell open.
dialog-preferences-enter-moves-to-next-row = Enter moves to the next row
dialog-preferences-save-without-asking = Save a changed transaction without asking
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

# The Category List: every category as a tree, with how much each is used.
dialog-category-list-title = Category List
# The filter row above the tree. Show picks rows by type or by whether anything uses
# them; Search keeps the rows whose names hold the text typed.
dialog-category-list-show = Show
dialog-category-list-show-all = All
dialog-category-list-show-income = Income
dialog-category-list-show-expense = Expense
dialog-category-list-show-used = Used
dialog-category-list-show-unused = Unused
dialog-category-list-search = Search
    .placeholder = Category name
# Ticked, the tree also lists the hidden categories, which the Category field leaves out.
dialog-category-list-show-hidden = Show hidden categories
dialog-category-list-close = Close
dialog-category-list-category = Category
# Whether a category is Income or Expense.
dialog-category-list-type = Type
# How many transactions and split lines are in a category or transfer to an account.
dialog-category-list-txns = Txns
dialog-category-list-total = Total
# The triangle that hides or shows the categories under the category named $name, for
# screen readers.
dialog-category-list-collapse = Collapse { $name }
dialog-category-list-expand = Expand { $name }
# A row's usage, as a screen reader reads it after the row's name. $total is an amount.
dialog-category-list-usage = { $count ->
        [one] 1 transaction
       *[other] { $count } transactions
    }, totalling { $total }
# A category's row as a screen reader reads it after its name: $category_type is Income
# or Expense, and $usage the row's usage, as above or below.
dialog-category-list-typed-usage = { $category_type }: { $usage }
# A hidden category's row as a screen reader reads it after its name: $usage is the
# row's type and usage, as above.
dialog-category-list-hidden-usage = Hidden, { $usage }
# Beside a hidden category's name in the tree.
dialog-category-list-hidden = hidden
# The usage of a row nothing uses, or whose total is too large to add up.
dialog-category-list-usage-count = { $count ->
        [0] No transactions
        [one] 1 transaction
       *[other] { $count } transactions
    }
# The buttons beside the tree, each acting on the selected categories.
dialog-category-list-new = New…
dialog-category-list-edit = Edit…
dialog-category-list-merge = Merge…
dialog-category-list-hide = Hide
# Hide's text while the selected category is hidden.
dialog-category-list-unhide = Unhide
dialog-category-list-delete = Delete…
# Below the tree: no OK button saves the changes made in the Category List.
dialog-category-list-status = Changes apply as soon as you confirm them.
# The confirm for deleting a category nothing uses. $name is its full path, as Food:Coffee.
dialog-delete-category-message = Delete { $name }? No transaction uses it.
dialog-delete-category-delete = Delete
# The confirm for deleting a category nothing uses, with the $subcategories categories
# under it, which nothing uses either.
dialog-delete-category-unused-parent = Delete { $name } and { $subcategories ->
        [one] its subcategory
       *[other] its { $subcategories } subcategories
    }? No transaction uses them.
# The list of categories that take a deleted category's transactions.
dialog-delete-category-move-to = Move those transactions to:
# What the list of categories that take a deleted or merged category's transactions
# shows until one is picked.
dialog-category-list-choose = Choose a category
# The prompt for deleting a category that transactions use. $name is its full path, as
# Food:Coffee, $subcategories how many categories under it are deleted with it, and
# $total an amount.
dialog-delete-category-used = { $subcategories ->
        [0] { $name } is
        [one] { $name } and its subcategory are
       *[other] { $name } and its { $subcategories } subcategories are
    } used by { $count ->
        [one] 1 transaction
       *[other] { $count } transactions
    } totalling { $total }.
# As above, for transactions whose total is too large to add up.
dialog-delete-category-used-count = { $subcategories ->
        [0] { $name } is
        [one] { $name } and its subcategory are
       *[other] { $name } and its { $subcategories } subcategories are
    } used by { $count ->
        [one] 1 transaction
       *[other] { $count } transactions
    }.
# The merge Dialog, which moves the transactions of the $categories categories it lists
# below this line to another category.
dialog-merge-categories-message = Merge { $categories ->
        [one] this category
       *[other] these { $categories } categories
    } into another:
dialog-merge-categories-merge = Merge
# How many transactions and split lines the merge moves.
dialog-merge-categories-count = { $count ->
        [0] No transaction moves.
        [one] 1 transaction moves.
       *[other] { $count } transactions move.
    }
# The list of categories that take the merged transactions.
dialog-merge-categories-into = Merge transactions into:
# Shown when the category picked has another type than a merged one. $source_type and
# $target_type are each Income or Expense.
dialog-merge-categories-type-warning = This merges { $source_type } into { $target_type }.
# The box that has the merge delete the categories it empties, ticked when it opens.
dialog-merge-categories-delete-sources = Delete unused selected categories
# Why a merge is refused, above the full path of each subcategory. $name is the full path
# of a category it lists, as Food:Coffee.
dialog-merge-categories-has-subcategories = { $name } has subcategories, so it can't be merged:

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

## Keypad

# The button beside an amount field that opens its keypad, a calculator under the
# field. The keypad's sum is labelled with the same word.
field-amount-keypad = Keypad
    .tooltip = Keypad (Alt+Down)
# The keypad's keys, as a screen reader reads them. Each shows its symbol.
keypad-add = Add
keypad-subtract = Subtract
keypad-multiply = Multiply
keypad-divide = Divide
keypad-decimal = Decimal mark
# .key is the letter on the key itself.
keypad-clear = Clear
    .key = C
keypad-backspace = Backspace
# Puts the keypad's result in the amount field.
keypad-ok = OK
# Shown under the keypad's sum while it ends in an operator, waiting for the next
# amount.
keypad-waiting = …
# Shown under the keypad's sum when it can't be read as an amount.
keypad-not-an-amount = Not an amount
# The Calculator's tall key, shown as "=", as a screen reader reads it. It replaces
# the sum with its result.
keypad-equals = Equals

## Calculator

# The Calculator, a window that stays open over the Main window. Its title, and the
# label of its sum.
calculator = Calculator
# Puts the Calculator's result into the amount cell last edited, as if typed there.
calculator-paste = Paste
# The Calculator's close button, as a screen reader reads it.
calculator-close = Close calculator

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
# The button under the calendar that picks today's date.
field-date-today = Today

## Status bar

status-about = { -brand-name } { $version }
status-not-available-yet = Not available yet
status-no-paste-target = Click a Payment or Deposit cell first, then Paste puts the result there.
status-paste-not-an-amount = The calculator's sum isn't an amount, so there is nothing to paste.
status-no-such-category = No category has that name. Pick one from the list, or create it.
status-reconciled-amount-not-saved = The changed row wasn't saved: a reconciled transaction's amount changes only once you confirm it.
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
status-category-under-itself = A category can't go under itself or one of its own subcategories.
# Giving a type to a subcategory, which takes its top-level category's type instead.
status-subcategory-type = A subcategory takes the type of the category at the top of its tree.
# Moving a deleted category's transactions to it or a category under it, which go with it.
status-reassign-into-itself = A category's transactions can't move to it or one of its own subcategories.
# Undoing a move of transactions to another category, after one of them left it.
status-not-in-category = That transaction is no longer in the category it was moved to.
# The data file refused a change, so Balanced didn't make it.
status-change-not-saved = That change couldn't be saved to the data file, so it wasn't made.
status-preferences-reset = Your preferences couldn't be read, so the defaults are in use. The old file is kept as preferences.toml.broken.
status-preferences-unreadable = Your preferences couldn't be read, so the defaults are in use.
status-preferences-not-saved = Your preferences couldn't be saved, so they last only until you quit.
