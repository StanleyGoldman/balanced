# Balanced's user-interface text in en-US, the source locale.
# `fl!` checks message ids and arguments in code against this file, and the
# locale-consistency test checks every other locale against it.

-brand-name = Balanced
-account-bar = Account bar

## Menu bar

menu-file = File
menu-edit = Edit
menu-view = View
menu-tools = Tools
menu-help = Help
menu-help-about = About { -brand-name }

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
