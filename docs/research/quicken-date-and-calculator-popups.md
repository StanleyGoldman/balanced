# Quicken's calendar and calculator popups

Research for [#360](https://github.com/StanleyGoldman/balanced/issues/360). It feeds the prototype in #214, which compares Balanced variants against Quicken's behaviour. The question: what do Quicken's calendar popup (date fields) and calculator popup (amount fields) do, in how they open, their layout, their keyboard flow and how a value returns to the field?

Gathered 2026-10-20 from Quicken's own help pages (`info.quicken.com`, the current home of the old `help.quicken.com` articles). Community-forum posts are not primary, so they appear only under [Unconfirmed](#unconfirmed-and-from-secondary-sources), marked as such. No Quicken install was available, so nothing here was observed in the product.

## Answer in brief

- **The calendar opens from a button at the right of the Date field, or from Alt+Down Arrow.** Clicking a day picks it. ([Enter a spending transaction](https://info.quicken.com/win/enter-a-spending-transaction), [Keyboard shortcuts](https://info.quicken.com/win/keyboard-shortcuts-in-quicken))
- **The calculator opens from a calculator icon in any amount field, or from Alt+Down Arrow in the Charge or Payment field.** You work it with its buttons or the number keys, and Enter puts the result in the field. ([How do I manually enter a transaction?](https://info.quicken.com/win/how-do-i-manually-enter-a-transaction), [Keyboard shortcuts](https://info.quicken.com/win/keyboard-shortcuts-in-quicken))
- **Quicken documents no keyboard flow inside either popup, no Esc behaviour, no calendar layout, and no Today button for Windows.** Those gaps stay open (see [Unconfirmed](#unconfirmed-and-from-secondary-sources)); only the Quicken Mac help mentions a Today button, and only for the Portfolio as-of date.
- **There is a second, separate calculator: Tools > Calculator.** It works "from any window or field", and its result goes into the field you select by clicking Paste. It is a floating window, not the field popup. ([How do I use the calculator?](https://info.quicken.com/win/how-do-i-use-the-calculator))
- **Balanced already covers the calendar's open paths, the date shortcut keys and arithmetic typed in the field. It has no calculator popup.** See [What Balanced covers](#what-balanced-covers).

## Calendar popup (Date fields)

### Opening

- A click on the "pop-up calendar icon at the right of the Date field" opens the calendar, and then you click a date. The same article notes you need not type slashes or the year. Source: [How do I manually enter a transaction?](https://info.quicken.com/win/how-do-i-manually-enter-a-transaction); the spending-transaction article says the same: "click the pop-up calendar at the right of the Date field, and then click a date" ([Enter a spending transaction](https://info.quicken.com/win/enter-a-spending-transaction)).
- Alt+Down Arrow "open[s] the calendar in the Date field" (Windows shortcut list, [Keyboard shortcuts in Quicken](https://info.quicken.com/win/keyboard-shortcuts-in-quicken)). The same shortcut opens the Payee, Tag and Category lists and the calculator, so it is Quicken's general "drop this field's popup" key.

### Keys in the Date field (no popup needed)

From the same shortcut list ([Keyboard shortcuts in Quicken](https://info.quicken.com/win/keyboard-shortcuts-in-quicken)):

| Key | Does |
|---|---|
| `t` | Today |
| `m` / `h` | First / last day of this month |
| `y` / `r` | First / last day of this year |
| `+` / `-` | Increase / decrease the date (the same keys step a check number) |
| Ctrl+G | Go to date |

The Mac list gives `t`, `+`/`-` and `m`/`h` ([Keyboard Shortcuts, Mac](https://info.quicken.com/mac/keyboard-shortcuts)); I did not fetch its full text, so treat the Mac `y`/`r` as unchecked.

Ctrl+K "go to the Calendar" is Quicken's separate Calendar view (bills and reminders), not the date popup.

### Layout, navigation, Today, keyboard inside the popup, Esc

Not stated in any Quicken help page I could reach. Not confirmed: month and year header, previous/next controls, weekday columns, a Today button, arrow-key or PgUp/PgDn movement inside the popup, whether Enter or Esc closes it, whether picking a day returns focus to the Date field.

## Calculator popup (amount fields)

### Opening

- "Click the calculator icon in any field in which you enter an amount" opens a pop-up calculator ([Enter a spending transaction](https://info.quicken.com/win/enter-a-spending-transaction); [How do I manually enter a transaction?](https://info.quicken.com/win/how-do-i-manually-enter-a-transaction)).
- Alt+Down Arrow opens it "in the Charge or Payment field" ([Keyboard shortcuts in Quicken](https://info.quicken.com/win/keyboard-shortcuts-in-quicken)). The list names only Charge and Payment, though the icon is said to be in any amount field.
- No shortcut list entry covers amount fields otherwise.

### Layout and keyboard flow

- It has on-screen buttons, and the number keys on the keyboard also work: "use the calculator buttons or the number keys" ([How do I manually enter a transaction?](https://info.quicken.com/win/how-do-i-manually-enter-a-transaction)).
- Not confirmed: the button grid, whether operator keys (`+ - * /`, `=`) and Backspace work from the keyboard, any memory or percent keys, tab order.

### Returning the value

- Enter "insert[s] the result" into the field ([How do I manually enter a transaction?](https://info.quicken.com/win/how-do-i-manually-enter-a-transaction)); the help text says to click Enter, so the popup has an Enter button as well as responding to the key (the article does not say whether the key works). 
- Not confirmed: what Esc or a click outside does (cancel and leave the field unchanged is the likely behaviour but no page says so), and which field gets focus afterwards.

### A number already in the field

No Quicken help page says whether the calculator starts from the field's current value, replaces it, or starts at zero. Not confirmed.

### Where else the calculator is used

- Any amount field, by the icon ([Enter a spending transaction](https://info.quicken.com/win/enter-a-spending-transaction)). The register's Charge and Payment columns are the ones the shortcut list names.
- Tools > Calculator opens the Quicken Calculator window, usable "from any window or field in Quicken". You click the target field, then Paste in the calculator window to place the amount; you can use its buttons or the number keys ([How do I use the calculator?](https://info.quicken.com/win/how-do-i-use-the-calculator)). That page does not say how Enter or Esc behave.

## Unconfirmed and from secondary sources

Community-forum posts, from Quicken's own forum but written by users, so not primary. They show what to test rather than what is true:

- Typing arithmetic straight into the amount field and pressing Enter stores the result; the thread warns that double-clicking the field opens a "mini calculator" ([community: Idea for Quicken](https://community.quicken.com/discussion/7973430/idea-for-quicken)).
- Mac: an operator key typed in an amount field brings up the calculator; `=` shows the total and Return sends it to the field ([Calculator in Quicken Mac](https://community.quicken.com/discussion/7946548/calculator-in-quicken-mac)). Users there report that typing `+50` into a filled field replaces the value rather than adding to it, and that `%` divides by 100. These are user reports from one platform.
- Mac calendar: a Today button exists and sets the field to today, and some installs closed the popup on it; `t` was the suggested keyboard workaround ([Calendar pop-up defaults to Dec 31, 2999](https://community.quicken.com/discussion/7871694/calendar-pop-up-defaults-to-dec-31-2999-or-1899-or-1900-q-mac)). A Today button in a Quicken Mac calendar is also named in the Portfolio help ([Portfolio time machine](https://info.quicken.com/mac/portfolio-time-machine-as-of-date)), not in a Date-field article.
- The date shortcut keys reportedly stop working when the system short date format changes ([community: Keyboard shortcuts in Date fields not working](https://community.quicken.com/discussion/7974578/keyboard-shortcuts-in-date-fields-not-working)).

Pages I could not read: `help.quicken.com` pages redirect to the `info.quicken.com` home page, and the `quickendocumentation.atlassian.net` pages returned no content. The #214 prototype will need to observe the real product to settle the gaps above.

## What Balanced covers

From the repo at the time of writing.

| Quicken behaviour | Balanced |
|---|---|
| Calendar icon at the right of the Date field | Covered: the 📅 button (`calendar_button` in `crates/balanced_ui/src/date_field.rs`), which never takes focus |
| Alt+Down Arrow opens the calendar | Covered: `handle_input` toggles the calendar on Alt+Down |
| Click a day to pick | Covered: a day picks on pointer press and closes the calendar (`pick`) |
| Date keys `t m h y r + -` | Covered (while nothing has been typed), and `=` also steps a day. Balanced adds ↑/↓ and PgUp/PgDn |
| Calendar layout | Balanced's `Calendar` shows one month, with the month and year between ◀ and ▶, weekday columns, today marked and the field's date marked. Quicken's layout is unconfirmed, so no comparison |
| Ctrl+G go to date | Not covered; `Key::G` with the command modifier is bound to `GoToOtherSide` in `crates/balanced_ui/src/keymap.rs` |
| Typing arithmetic in an amount field | Covered: `AmountReader::read` in `crates/balanced_i18n/src/calculator.rs` evaluates `+ - * /` with precedence and rounds half away from zero; `amount_field.rs` shows a live `= $33.33` (or `~` when rounded) preview and refuses characters the reader does not accept |
| Calculator popup with buttons, opened by icon or Alt+Down | Not covered: no calculator widget exists |
| Tools > Calculator window with Paste | Not covered |

Practice docs that bind a popup built for the prototype: `docs/practices/egui.md` ("Popups and layout") says a popup under a focused field must pick on `is_pointer_button_down_on()`, not `clicked()`, and must keep the field focused across a press on the popup or a button beside the field (the 📅 button and calendar already do this). `docs/practices/money.md` says arithmetic multiplies and divides first, treats a leading minus as a sign, and rounds `HalfAwayFromZero` to the currency's places, which a calculator popup would reuse by feeding `AmountReader` rather than evaluating on its own.
