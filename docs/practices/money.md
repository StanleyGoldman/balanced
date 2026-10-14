# Money

Amounts are exact decimals that always carry their currency's decimal places, and the user sees and types them in their own formatting locale, whatever the currency. The decisions behind this are in issue #14, "How the GUI represents money".

## Types

- **A posted amount is a `balanced_core::Money`**: a `rust_decimal::Decimal` plus a `CurrencyCode`, always at the currency's decimal places (`5` USD is held as `5.00`). Unit prices, share quantities and rates are plain `Decimal`, because they need more places than the currency has.
- **`CurrencyCode` carries the decimal places**, from ISO 4217 (USD 2, JPY 0, BHD 3, CLF 4). `iso_currency` stays inside `CurrencyCode`, so non-ISO currencies can be added later without touching its users.
- **Combine amounts with `checked_add` / `checked_sub`.** Mixing currencies returns `MoneyError::CurrencyMismatch`; converting between currencies is a domain operation with its own rate, never an implicit step.

## Rounding

Every rounding names its `RoundingPolicy` (`BankersEven` or `HalfAwayFromZero`), chosen for that operation.

- `Money::new` rejects an amount with more decimal places than the currency allows. Use `Money::rounded(amount, currency, policy)` when rounding is intended.
- On a money `Decimal`, round only with `round_dp_with_strategy`. `Decimal::rescale` and `round_dp` round with a built-in default; use `rescale` only to pad, where it can't round.

## Floats

Amounts are built from text or integers. `clippy.toml` bans `Decimal`'s float conversions. `Decimal::try_from(f64)` and `f64::try_from(Decimal)` go through `TryFrom` and slip past the ban, so review flags them. `rust_decimal`'s `serde-float` feature stays off; serialised amounts are strings.

## Showing amounts

- **`Localizer::format_amount(amount, currency, style)` is the only way an amount reaches the screen.** It formats in the formatting locale (`1.234,50 $` for USD in `de-DE`) with the locale's standard minus sign. It never rounds: an amount with more decimal places than its currency shows all of them. `Money` has no `Display`, so `{}` on it fails to compile.
- **Style:** `AmountStyle::Bare` in Register cells, `AmountStyle::Symbol` in the Account bar, totals, and anywhere currencies mix.
- **`balanced_i18n` doesn't know `Money`.** `balanced_ui` converts at the call: `Currency::new(code.as_str(), code.decimal_places())` and `money.amount()`.
- **In messages**, pass the formatted string as a Fluent argument (see [i18n](i18n.md)).
- **Cache formatted strings** in the window's view model, rebuilt when the snapshot or the formatting locale changes (ADR 0001). Each call builds an ICU4X formatter.
- Negative amounts in parentheses (accounting style) are deferred. Red negatives belong to the theme.

## Reading typed amounts

- **Parse on commit** (Enter, Tab or focus loss) with `Localizer::parse_amount(text, decimal_places)`. While editing, the field keeps the raw text; after a successful commit it shows the reformatted amount.
- **On `Err`, the field stays in edit mode** with an inline error and no Command is sent. Map each `AmountParseError` variant to a Fluent message, naming the currency for `TooManyDecimalPlaces` ("USD amounts have 2 decimal places").
- **What the parser accepts:** the locale's decimal mark and group separators in the locale's positions (`1.23` is rejected in `de-DE`), a typed space for a no-break-space separator, a leading or trailing minus, and any currency symbol or code at either end, which it ignores. Extra decimal places are rejected unless they are zeros.
- Calculator input (`+ - * /` in the field) is left to "Prototype money, date and category input fields".

## Upgrading `icu_experimental`

`format_amount` is built on `icu_experimental::CurrencyFormatter`, whose API breaks with every release. It is pinned with `=` in the workspace `Cargo.toml`, and `crates/balanced_i18n/src/money.rs` is the only file that names it.

To upgrade: bump the pin, fix `money.rs`, and read the diff of the `en-US` / `de-DE` snapshot tests there. Every changed line is a visible change for users; accept it in the PR deliberately.

Keep `IsoDigits` in that file. `CurrencyFormatter` rounds to CLDR's display digits, which differ from ISO 4217's for some currencies (IQD is 3 in ISO, 0 in CLDR); `IsoDigits` feeds it the currency's own decimal places so no posted digit is hidden.

## Tests

- Test formatting and parsing with explicit locales. Add a locale the app ships to the round-trip test, `formatted_amounts_parse_back`.
- Money logic lives in `balanced_core` and `balanced_i18n`, so mutation testing covers it: run `cargo xtask mutants` before opening the PR.
