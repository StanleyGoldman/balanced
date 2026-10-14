# Balanced's user-interface text in German: a seed set that proves the fallback
# chain. Every message missing here falls back to en-US.
# Terms come from the German GnuCash, KMyMoney, KDE and LibreOffice translations.

-brand-name = Balanced
-account-bar = Kontenleiste

## Menu bar

menu-file = Datei
menu-edit = Bearbeiten
menu-view = Ansicht
menu-tools = Extras
menu-help = Hilfe

## Account bar

account-bar-title = Konten
account-bar-total = Summe: { $amount }
    .tooltip = Summe der Salden der in der { -account-bar } angezeigten Konten

## Register

register-column-date = Datum
register-column-payee = Zahlungsempfänger
register-transaction-count =
    { $count ->
        [one] { $count } Buchung
       *[other] { $count } Buchungen
    }
