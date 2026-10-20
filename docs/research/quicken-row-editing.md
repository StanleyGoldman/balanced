# Quicken row editing: inline buttons, entry bar, keyboard, leaving a changed row, Split

Research for [#365](https://github.com/StanleyGoldman/balanced/issues/365). It feeds #356, the row-level editing prototype.

Researched 2026-10-20 from Quicken's own help pages for Quicken for Windows (Classic). Nothing was checked against a running copy of Quicken. Quicken's help covers little of the row-editing detail, so most of the issue's six questions end up unconfirmed.

## Question

How does Quicken behave while a Register row is edited: the inline buttons, the entry bar, the keyboard, the prompt on leaving a changed row, and the Split flow?

## Answer in brief

- **Buttons:** the help pages name a **Save** button on the transaction toolbar and a **Split** icon. Neither page uses the labels "Enter", "Record" or "More actions" for a button. Placement, new-transaction-only buttons, the Edit button on the row and the Classic vs current difference are unconfirmed.
- **Entry bar:** no help page describes an entry-bar mode. Unconfirmed.
- **Keyboard:** the shortcuts page lists **Enter or Ctrl+Enter** to record, **Ctrl+S** for Split, **Ctrl+N** for a new transaction, **Tab / Shift+Tab** between fields. A preference swaps what Enter does. **Esc** undoes an edit.
- **Leaving a changed row:** Notify Preferences has "To save a transaction after changing it", which warns when you change a transaction and forget to save. The prompt's text, choices and any "always save" equivalent are unconfirmed.
- **Split:** opens a dialog from the Split icon; lines are not committed until OK then Save (unless "Automatically Enter Split Data" is on). The remainder shows below the last line with Adjust, Allocate and Edit options.
- **More actions menu:** unconfirmed.

## 1. Buttons on the edited row

- The change-a-transaction steps are: select the transaction, edit the fields in place, "Click **Save** to record the changes." ([Change a spending transaction](https://info.quicken.com/win/change-a-spending-transaction)) That page does not mention Enter, Split or Actions buttons.
- The split steps say "On the transaction toolbar, click the 'Split' icon" and "Click 'Save' on the transaction toolbar to enter the transaction in the register." ([Record a split transaction](https://www.quicken.com/support/record-split-transaction/)) The fetched page does not mention an "Enter" button; the label is Save.
- The Register Action Bar above the register is a separate, customizable row of account-level buttons (add, reorder, remove, reset), reached from Account Actions → Customize Action Bar. It is not the row's buttons. ([Customize the register action bar](https://info.quicken.com/win/how-do-i-customize-the-register-action-bar))
- Search summaries of Quicken Community threads say the per-row buttons (Save, Split, and others) belong to an "Action buttons" setting under the register columns/gear control, and appear in the lower right of the register or on the second line in two-line mode. These are forum reports (see the labelled section below); the threads themselves require sign-in and could not be fetched.

## 2. Entry bar

No Quicken help page found describes an entry-bar mode. Unconfirmed.

## 3. Keyboard

- **Record:** "ENTER or CTRL+ENTER". **Split:** CTRL+S. **New transaction:** CTRL+N. **Tab:** next field or column; Shift+Tab goes back. The page lists no Esc shortcut for the register. ([Keyboard shortcuts in Quicken](https://info.quicken.com/win/keyboard-shortcuts-in-quicken))
- **Enter key preference:** Data entry and QuickFill preferences has an option that "Lets you use the Enter key, in addition to Tab, to move between fields in the register and the Write Checks window." ([Data entry and QuickFill preferences](https://info.quicken.com/win/data-entry-and-quickfill-preferences)) What Enter then does in the last cell, and that Ctrl+Enter saves while it is on, comes from a search summary of Quicken's documentation wiki and community posts, not from the fetched page. Treat as unconfirmed.
- **Esc:** to undo an edit, select the transaction and choose Edit → Transaction → Restore Transaction, "or press **Esc**". ([Change a spending transaction](https://info.quicken.com/win/change-a-spending-transaction))
- **Tab at the last cell** (into the buttons or round to the first cell) and **focus after Record or Cancel**: unconfirmed.

## 4. Leaving a changed row

- Edit → Preferences → Notify has **To save a transaction after changing it**: "Select this preference to be warned if you change a transaction and then forget to save it." ([Notify preferences](https://info.quicken.com/win/notify-preferences))
- The same page lists **Before changing existing transactions**: "This preference helps you avoid inadvertently changing a transaction that you've already entered." (same page)
- The prompt's text, its button labels, an "Always save without asking" equivalent, and the behaviour for an invalid row are not on the page. Unconfirmed.
- Changing a reconciled transaction asks you to confirm. ([Change a spending transaction](https://info.quicken.com/win/change-a-spending-transaction))

## 5. Split

- Open: select the transaction, click the Split icon on the transaction toolbar (Ctrl+S per the shortcuts page). ([Record a split transaction](https://www.quicken.com/support/record-split-transaction/))
- The Split Transaction dialog takes a category, optional tag and amount per line. "Add lines" adds rows (up to 250), "Clear All" resets, "Edit" inserts or deletes a line. (same page)
- Pending: OK closes the dialog, then Save enters the transaction, "You can skip this step if the register preference 'Automatically Enter Split Data' is turned on." (same page) So lines are pending until Save by default.
- Remainder: when the Split Total differs from the Transaction Total, "Quicken shows the leftover amount below the last split line." Fixes: change line amounts; **Adjust** sets the Transaction Total to the Split Total; Edit → "Apply remainder to current line"; Edit → "Allocate this line to other split lines"; **Allocate** spreads the leftover. (same page)
- Esc inside the dialog: unconfirmed.

## 6. More actions menu

No Quicken help page found names a "More actions" menu on the edited row or an "Actions" column menu on an unedited row. Unconfirmed, including whether the two match.

## Secondary sources (not Quicken documentation)

- **Quicken Community, summaries only:** Ctrl+Enter saves in non-investment registers; with the Enter-moves-fields option off, Tab moves and Enter saves, and with it on, Enter moves and Ctrl+Enter saves; one user reports the save warning not appearing even with the Notify box ticked. ([Notify search summary](https://community.quicken.com/discussion/comment/20326483), [shortcut thread](https://community.quicken.com/discussion/comment/20151426)) The community site now needs sign-in, so the threads were not read directly.
- **GnuCash:** a Cancel toolbar button reverts uncommitted edits only while the cursor is still in the same transaction. Per a GnuCash developer, an edit is committed by Enter, tabbing off the end, or clicking another transaction and confirming a message box. A bug report says the save-or-cancel prompt reappears after "don't ask me again" when switching by click. ([GnuCash list: pending edit behavior](https://lists.gnucash.org/pipermail/gnucash-user/2019-June/085396.html), [bug 335437](https://bugzilla.gnome.org/show_bug.cgi?id=335437), [GnuCash 2.4 guide](https://m.aikan.qq.com/read/1034852094/31)) These come from search summaries; the pages were not fetched.
- **Microsoft Money, YNAB:** not researched.

## Gaps and unconfirmed

- Labels, placement and new-transaction-only status of the row buttons (Enter/Record, Edit, Split, More actions); Classic vs current.
- Whether Quicken has an entry-bar mode.
- Tab at the last cell; Enter in each cell and in the last cell (only via summaries); focus after Record and Cancel.
- The leave-a-changed-row prompt: text, choices, "always save" equivalent, invalid-row behaviour.
- Esc inside the Split dialog; whether inline split lines exist in any version.
- The More actions menu and its match with the Actions column menu.
- Everything above is from Quicken for Windows help; the current/Simplifi-era desktop was not covered, and a running copy would settle most gaps.
