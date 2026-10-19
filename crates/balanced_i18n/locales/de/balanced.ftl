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
# Coined from "Neue Datei" (GnuCash) and "Daten".
menu-file-new-data-file = Neue Datendatei …
# Coined from "Öffnen" (GnuCash) and "Datendatei", above.
menu-file-open-data-file = Datendatei öffnen …
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
register-column-balance = Saldo
register-new-transaction = + Buchung
register-current-balance = Aktuell { $amount }
register-ending-balance = Endsaldo { $amount }
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

# Why a data file wasn't opened or created.
# The first sentence follows GnuCash's "Die Datei %s konnte nicht gefunden werden"; the
# second is coined.
dialog-data-file-missing = Die Datei „{ $file }“ konnte nicht gefunden werden. Sie wurde vielleicht verschoben, umbenannt oder gelöscht.
# Coined from GnuCash's "verweist nicht auf GnuCash-Daten".
dialog-data-file-not-balanced = „{ $file }“ ist keine { -brand-name }-Datendatei.
# "fehlerhaft" follows GnuCash; "aus einer Sicherung wiederherstellen" is coined.
dialog-data-file-damaged = „{ $file }“ ist fehlerhaft. Stellen Sie die Datei aus einer Sicherung wieder her.
# GnuCash's "Diese Datei scheint von einer neueren Version von GnuCash zu sein. Sie
# brauchen eine neuere GnuCash-Version, um die Datei lesen zu können."
dialog-data-file-newer = „{ $file }“ scheint von einer neueren Version von { -brand-name } zu sein. Sie brauchen eine neuere { -brand-name }-Version, um die Datei lesen zu können.
# Coined.
dialog-data-file-locked = „{ $file }“ ist in einem anderen Programm geöffnet. Schließen Sie die Datei dort und versuchen Sie es erneut.
# Coined from GnuCash's "GnuCash konnte die verknüpfte Datei nicht öffnen".
dialog-data-file-unreadable = { -brand-name } konnte „{ $file }“ nicht lesen.
# Coined from the same.
dialog-data-file-not-created = { -brand-name } konnte „{ $file }“ nicht erstellen.
# Coined.
dialog-data-file-open-not-replaced = „{ $file }“ ist die geöffnete Datendatei und kann nicht durch eine neue ersetzt werden.
# A file name, the same in every language.
dialog-data-file-suggested-name = { -brand-name }.balanced
# Coined from GnuCash's "Nur Datenfiles".
dialog-data-file-filter = { -brand-name }-Datendateien
