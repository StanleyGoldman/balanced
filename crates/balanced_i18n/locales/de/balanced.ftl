# Balanced's user-interface text in German: a seed set covering the Menu bar, the tab
# row, the Account bar, the Register headers, the modifier key names and the common
# Dialog buttons. Every message missing here falls back to en-US.
# Wording comes from the German GnuCash, KMyMoney, Qt (KDE) and LibreOffice
# translations. A message marked "Coined" was assembled from their words and needs a
# native speaker to check it.

-brand-name = Balanced
-account-bar = Kontenleiste
-register = Kontobuch

## Menu bar

menu-file = Datei
menu-file-new-account = Neues Konto …
menu-file-quit = Beenden
menu-edit = Bearbeiten
menu-edit-undo = Rückgängig
menu-edit-redo = Wiederherstellen
menu-view = Ansicht
# Coined from "Wechseln".
menu-view-switch-pane = Bereich wechseln
menu-tools = Extras
# Coined from "Konten" and "verwalten".
menu-tools-manage-accounts = Konten verwalten …
menu-tools-preferences = Einstellungen …
menu-help = Hilfe
menu-help-about = Über { -brand-name }
menu-new-transaction = Neue Buchung
menu-delete-transaction = Buchung löschen

menu-modifier-ctrl = Strg
menu-modifier-alt = Alt
menu-modifier-shift = Umschalt
menu-modifier-option = Option

## Tab row

tab-home = Übersicht
tab-accounts = Konten
tab-bills = Rechnungen
tab-spending = Ausgaben
# Coined: none of the sources has a planning view.
tab-planning = Planung
tab-investing = Investitionen
tab-property-debt = Anlagen & Verbindlichkeiten

## Account bar

account-bar-title = Konten
account-bar-total = Summe: { $amount }
    .tooltip = Summe der Salden der in der { -account-bar } angezeigten Konten
# Coined: none of the sources groups bank accounts under one heading.
account-bar-group-banking = Bankkonten
account-bar-group-investing = Investitionen
account-bar-group-property-debt = Anlagen & Verbindlichkeiten
account-bar-group-closed = Geschlossen
account-bar-net-worth = Reinvermögen
# Coined from "Einklappen"; "ausklappen" is its counterpart.
account-bar-fold = { -account-bar } einklappen
account-bar-unfold = { -account-bar } ausklappen
account-bar-open-register = { -register } öffnen
account-bar-reconcile = Abgleichen
account-bar-manage-accounts = Konten verwalten …
account-bar-new-account = + Konto hinzufügen

## Register

register-column-date = Datum
register-column-number = Nr
register-column-payee = Zahlungsempfänger
register-column-category = Kategorie
register-column-memo = Notiz
register-column-amount = Betrag
register-column-balance = Saldo
register-new-transaction = + Buchung
register-transaction-count =
    { $count ->
        [one] { $count } Buchung
       *[other] { $count } Buchungen
    }

## Dialogs

dialog-cancel = Abbrechen
dialog-close = Dialog schließen
dialog-ok = OK
dialog-delete-transaction-delete = Löschen
dialog-close-account-close = Schließen
dialog-delete-account-delete = Löschen
dialog-new-account-back = Zurück
dialog-new-account-next = Weiter
dialog-new-account-finish = Fertigstellen
