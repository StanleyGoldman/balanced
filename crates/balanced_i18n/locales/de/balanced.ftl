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
menu-file-recent = { $number } { $file }
menu-edit = Bearbeiten
menu-edit-undo = Rückgängig
menu-edit-redo = Wiederherstellen
menu-edit-go-to-date = Zu Datum gehen …
menu-view = Ansicht
# Coined from "Wechseln".
menu-view-switch-pane = Bereich wechseln
menu-tools = Extras
# Coined from "Konten" and "verwalten".
menu-tools-manage-accounts = Konten verwalten …
menu-tools-preferences = Einstellungen …
menu-tools-calculator = Taschenrechner
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
# GnuCash's column header for clearing a transaction ("C" → "Abgl").
register-column-clr = Abgl
# Assembled from the heading above and the en-US pattern.
register-clr = Abgl { $payee }, { $date }, { $amount }
# GnuCash's "Not Cleared", "Cleared" and "Reconciled". The marks are GnuCash's cleared
# flag ("c" → "b") and its column header for 'Reconciled' ("R" → "A").
register-clr-uncleared = Unbestätigt
register-clr-cleared = Bestätigt
    .mark = b
register-clr-reconciled = Abgeglichen
    .mark = A
# GnuCash's "Save", "_Add" and "Cancel".
register-row-save = Speichern
register-row-add = Hinzufügen
register-row-cancel = Abbrechen
# GnuCash's "Split" as it names a multi-split transaction, "Mehrteilig". An adjective
# where en-US has a verb; a native speaker should check it.
register-row-split = Mehrteilig
# Coined: GnuCash has no "More actions". .a11y follows GnuCash's "Actions",
# "Aktionen".
register-row-more = Mehr
    .a11y = Weitere Aktionen
register-new-transaction = + Buchung
register-current-balance = Aktuell { $amount }
register-ending-balance = Endsaldo { $amount }
# GnuCash's "Cleared", as the Cleared status above.
register-cleared-balance = Bestätigt { $amount }
register-transaction-count =
    { $count ->
        [one] { $count } Buchung
       *[other] { $count } Buchungen
    }

## Dialogs

dialog-cancel = Abbrechen
dialog-close = Dialog schließen
dialog-ok = OK
dialog-go-to-date-title = Zu Datum gehen
dialog-go-to-date-date = Datum
# Coined from GnuCash's "Springen" for its register's Jump.
dialog-go-to-date-go = Springen
dialog-delete-reconciled-transaction-message = Diese Buchung wurde abgeglichen. Trotzdem löschen? { $payee }, { $date }, { $amount }
dialog-delete-transaction-delete = Löschen
dialog-reconciled-amount-message = Diese Buchung wurde abgeglichen. Betrag auf { $new } ändern? { $payee }, { $date }, { $amount }
dialog-reconciled-amount-change = Betrag ändern
# Coined. "Abgleich" and "Bestätigt" follow the Clr column's GnuCash terms.
dialog-unreconcile-message = Abgleich dieser Buchung aufheben? Die Buchung wurde mit einem Kontoauszug abgeglichen und wird auf „Bestätigt“ gesetzt. { $payee }, { $date }, { $amount }
dialog-unreconcile-accept = Auf „Bestätigt“ setzen
dialog-leave-row-message = Geänderte Buchung speichern?
# Coined from GnuCash's "Hinzufügen" and "Buchung".
dialog-leave-row-draft-message = Neue Buchung hinzufügen?
dialog-leave-row-save = Speichern
dialog-leave-row-add = Hinzufügen
# Coined.
dialog-leave-row-continue = Weiter bearbeiten
dialog-leave-row-discard = Verwerfen
# Coined after LibreOffice's "Beim Schließen immer eine neue Version speichern".
dialog-leave-row-always = Immer ohne Nachfrage speichern
# Coined from GnuCash's "Scheckformular kann nicht gespeichert werden." and "Buchung".
dialog-cant-save-title = Diese Buchung kann nicht gespeichert werden
# Coined, as dialog-leave-row-continue.
dialog-cant-save-continue = Weiter bearbeiten
dialog-cant-save-discard = Änderungen verwerfen
# GnuCash's "Split Transaction".
dialog-split-title = Mehrteilige Buchung
# Qt's "Done".
dialog-split-done = Fertig
# GnuCash's "Amount".
dialog-split-amount = Betrag
# Coined from GnuCash's "Line" (Zeile), with register-column-category,
# register-column-memo and dialog-split-amount.
dialog-split-line-category = Kategorie, Zeile { $line }
dialog-split-line-memo = Notiz, Zeile { $line }
dialog-split-line-amount = Betrag, Zeile { $line }
# Coined from GnuCash's "_Add" and "Line".
dialog-split-add-line = Zeile hinzufügen
# GnuCash's "_Remove"; .a11y coined from it and "Line".
dialog-split-remove-line = Entfernen
    .a11y = Zeile { $line } entfernen
# GnuCash's "Total".
dialog-split-total = Summe
# Coined, with GnuCash's "Difference" and "Total".
dialog-split-mismatch = Die Zeilen ergeben { $lines }, nicht die Summe von { $total }. Differenz: { $difference }
# Coined.
dialog-split-no-category = Zeile { $line } hat keine Kategorie.
dialog-split-unknown-category = Zeile { $line }: Keine Kategorie und kein Konto heißt „{ $name }“.
dialog-split-line-error = Zeile { $line }: { $error }
dialog-split-total-error = Summe: { $error }
dialog-split-no-lines = Geben Sie mindestens einer Zeile eine Kategorie.
# Coined from GnuCash's "Geänderte Buchung speichern?".
dialog-preferences-save-without-asking = Geänderte Buchung ohne Nachfrage speichern
dialog-close-account-close = Schließen
dialog-delete-account-delete = Löschen
# Coined from GnuCash's "Neu", "Bearbeiten", "Zusammenführen", "Ausblenden" and
# "Löschen".
dialog-category-list-new = Neu …
dialog-category-list-edit = Bearbeiten …
dialog-category-list-merge = Zusammenführen …
dialog-category-list-hide = Ausblenden
dialog-category-list-delete = Löschen …
# Coined.
dialog-category-list-status = Änderungen gelten sofort nach der Bestätigung.
# Coined from GnuCash's "Löschen" and "Buchung".
dialog-delete-category-message = { $name } löschen? Keine Buchung verwendet diese Kategorie.
dialog-delete-category-delete = Löschen
dialog-new-account-back = Zurück
dialog-new-account-next = Weiter
dialog-new-account-finish = Fertigstellen
# Coined from GnuCash's "Mit »_Eingabe« gehen Sie zur leeren Buchung" and "Zeile".
dialog-preferences-enter-moves-to-next-row = Mit »Eingabe« zur nächsten Zeile gehen

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
# Coined.
dialog-data-file-backup-failed = { -brand-name } konnte vor der Aktualisierung keine Sicherungskopie von „{ $file }“ anlegen, deshalb wurde die Datei nicht verändert. Prüfen Sie, ob im Ordner genug Platz ist und Sie dort Dateien speichern dürfen, und versuchen Sie es erneut.
# Coined.
dialog-data-file-upgrade-failed = { -brand-name } konnte „{ $file }“ nicht aktualisieren, deshalb wurde die Datei nicht verändert. Daneben liegt eine vor der Aktualisierung angelegte Sicherungskopie namens „{ $backup }“.
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

# Coined from "Datendatei", above.
dialog-welcome-message = { -brand-name } speichert Ihre Finanzen in einer Datendatei.
# Coined from "Neue Datendatei", above, and "Erstellen".
dialog-welcome-create = Neue Datendatei erstellen …
# Coined from "Datendatei öffnen", above, and "Vorhanden".
dialog-welcome-open = Vorhandene Datendatei öffnen …
dialog-welcome-continue = Weiter
dialog-welcome-quit = Beenden

## Keypad

field-amount-keypad = Rechner
    .tooltip = Rechner (Alt+Herunter)
keypad-add = Addieren
keypad-subtract = Subtrahieren
keypad-multiply = Multiplizieren
keypad-divide = Dividieren
keypad-decimal = Dezimaltrennzeichen
keypad-clear = Löschen
    .key = C
keypad-backspace = Rücktaste
keypad-ok = OK
keypad-waiting = …
keypad-not-an-amount = Kein Betrag
keypad-equals = Gleich

## Calculator

calculator = Taschenrechner
calculator-paste = Einfügen
calculator-close = Taschenrechner schließen
status-no-paste-target = Klicken Sie zuerst auf eine Zelle für Zahlung oder Einzahlung, dann fügt Einfügen das Ergebnis dort ein.
status-paste-not-an-amount = Die Rechnung im Taschenrechner ist kein Betrag, daher gibt es nichts einzufügen.

## Date field

field-date-today = Heute
