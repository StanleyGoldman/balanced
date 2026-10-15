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
menu-file-quit = Quit
menu-edit = Edit
menu-view = View
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

## Account bar

account-bar-title = Accounts
account-bar-total = Total: { $amount }
    .tooltip = Sum of the balances of the accounts shown in the { -account-bar }

## Register

register-column-date = Date
register-column-payee = Payee
register-column-amount = Amount
register-transaction-count =
    { $count ->
        [one] { $count } transaction
       *[other] { $count } transactions
    }

## Status bar

status-about = { -brand-name } { $version }
