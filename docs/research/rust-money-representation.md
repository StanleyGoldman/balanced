# How Rust represents money

Research for [#10](https://github.com/StanleyGoldman/balanced/issues/10) (child of the wayfinder map #5). It feeds #14, "How the GUI represents money".

Researched 2026-10-13. Crate metadata comes from the crates.io API (`https://crates.io/api/v1/crates/<name>`) on that date. Where this note gives a "last release" date, it is the publish date of the newest version, not the crate's `updated_at`.

## Question

What are the established options for representing money in Rust, and what are their trade-offs for a personal-finance GUI? The answer must cover `rust_decimal`, integer minor units and the crates built on them, ISO 4217 codes and exponents, rounding, parsing and formatting, and serde. Balanced has one extra constraint from #5: **locale formatting (separators, symbol placement) is independent of an account's currency**. A `de-DE` user looking at a USD account sees `1.234,56 $`, not `$1,234.56`.

## Answer in brief

- **Amounts: use `rust_decimal::Decimal`.** It is the de facto standard: about 157M downloads, released 2026-09-02, and used by every Rust finance codebase checked (tackler, rustledger, beancount-parser-lima, Wealthfolio). Pair it with a Balanced-owned `Money { amount: Decimal, currency: CurrencyCode }` newtype. Integer minor units (`i64` cents) are a valid alternative and are what Actual Budget's API uses. The cost is that every value needs its exponent carried alongside it, and prices, rates and splits need a second type anyway.
- **Do not adopt `rusty-money` as the money type.** Its display and parse locale is a field of the *currency* (USD → `EnUs`), with only 4 hard-coded locales. That breaks Balanced's "locale is independent of currency" rule. It also borrows its currency (`Money<'a, T>`), which is awkward to store in egui state.
- **Currency metadata: `iso_currency`** (released 2026-09-06). It is an enum of ISO 4217 codes with `exponent() -> Option<u16>`, symbol and numeric code, and has a `with-serde` feature. Alternatively, generate a small table from the official SIX list.
- **Formatting: ICU4X.** `icu_decimal` 2.3 (stable) formats numbers with locale separators and grouping. `icu_experimental` 0.6 `CurrencyFormatter` does locale-correct symbol placement from CLDR, but its API is explicitly unstable. No Rust crate does **locale-aware parsing** of user input, so Balanced writes its own small parser driven by the locale's decimal and group symbols.
- **Rounding:** use `Decimal::round_dp_with_strategy(exponent, RoundingStrategy::MidpointNearestEven | MidpointAwayFromZero)`. Make the strategy an explicit, named policy and never rely on an implicit default.

## 1. Representation options

### 1a. `rust_decimal::Decimal` (decimal floating point, fixed 128 bits)

- Crate: [`rust_decimal`](https://docs.rs/rust_decimal/1.43.0/rust_decimal/) 1.43.0 (2026-09-02), MIT, MSRV 1.67.1. About 157.6M total downloads and about 38.5M recent (90-day). Repo: <https://github.com/paupino/rust-decimal>. Actively maintained, with releases in 2026-05, 2026-06 and 2026-09.
- Representation: a 96-bit integer mantissa, a sign and a scale `e` with "e is an integer between 0 and 28 inclusive". That gives "roughly 28 base-10 digits (29 in some cases)". `Decimal::MAX` is `79_228_162_514_264_337_593_543_950_335`. The type is `Copy`, and `serialize()` yields 16 bytes. Trailing zeros are preserved (`1.50` keeps scale 2), and `normalize`/`round_dp` remove them. ([docs](https://docs.rs/rust_decimal/1.43.0/rust_decimal/), [`Decimal`](https://docs.rs/rust_decimal/1.43.0/rust_decimal/struct.Decimal.html))
- Construction: `Decimal::new(i64, scale)`, `from_i128_with_scale`, `mantissa() -> i128`, `scale() -> u32`, and the `dec!(12.34)` compile-time macro (crate `rust_decimal_macros` 1.40.0). `FromStr` parses `"1234.56"`. `from_str_exact` "will return an error" if the value underflows and cannot be represented at the given scale. Use it for user input so that nothing is silently lost. (The docs do not describe how plain `FromStr` handles excess digits.)
- Arithmetic: operators plus `checked_add`/`checked_mul`, which return `None` on overflow. The `maths` feature adds `pow`, `ln` and `exp`, which are useful for loan and interest calculations.
- Rounding: `round_dp_with_strategy(dp, RoundingStrategy)` ([enum](https://docs.rs/rust_decimal/1.43.0/rust_decimal/enum.RoundingStrategy.html)) with `MidpointNearestEven` (banker's), `MidpointAwayFromZero`, `MidpointTowardZero`, `ToZero`, `AwayFromZero`, `ToNegativeInfinity` and `ToPositiveInfinity`. The old names (`BankersRounding`, `RoundHalfUp`, and so on) have been deprecated since 1.11. Caution: `rescale` rounds with `MidpointAwayFromZero` implicitly.
- serde ([features](https://docs.rs/rust_decimal/1.43.0/rust_decimal/)): `serde-str` serialises as a string, which the docs recommend for `bincode`/`csv`-like formats. `serde-float` is lossy for money. `serde-arbitrary-precision` works with `serde_json/arbitrary_precision`. Per-field variants are `serde-with-str`, `serde-with-float` and `serde-with-arbitrary-precision`, used through `#[serde(with = ...)]`. DB features cover Postgres (`db-postgres`, `db-tokio-postgres`, `db-diesel-postgres`) and `db-diesel-mysql`. There is **no SQLite feature**, so with SQLite it would be stored as TEXT or as integer mantissa plus scale.
- Fit: exact decimal arithmetic, a scale that is a property of the value rather than the type (prices like `0.0125`, share quantities like `12.3456`, FX rates), and an ecosystem default. Downsides: 16 bytes rather than 8, and slower than `i64`, which is irrelevant at personal-finance volumes.

### 1b. Integer minor units (`i64` cents)

- Store `amount_minor: i64` alongside the currency, so that `$120.30` becomes `12030`. Actual Budget's API defines it this way: "A currency amount is an integer representing the value without any decimal places … a USD amount of `$120.30` would be `12030`." ([Actual API reference](https://actualbudget.org/docs/api/reference)) Actual is TypeScript, so this is a design reference rather than a Rust one.
- Pros: 8 bytes, `Copy`, `Ord`, trivially exact for addition and subtraction, and stored as SQLite INTEGER with no ceremony. The range of ±9.2×10¹⁸ minor units is ample.
- Cons:
  - The exponent lives outside the number. A value means nothing without its currency's exponent, which varies: 0 for JPY, 2 for USD, 3 for BHD and KWD, 4 for CLF and UYW (see §2).
  - Anything finer than the minor unit needs a second type. That includes unit prices, share quantities, FX rates, interest accrual and split-before-rounding.
  - Multiplication and division need explicit rounding back to minor units at every step.
  - Changing the exponent (redenomination, or a crypto with 8 to 18 decimals) means rewriting stored data.
- GnuCash, a C reference point for "integers, but exact", uses a **rational** `gnc_numeric` with "a 64-bit integer as numerator and denominator", where the denominator is usually the smallest currency unit. It has explicit rounding policies, including `GNC_HOW_RND_NEVER`, which errors on any remainder. ([GnuCash Numeric docs](https://code.gnucash.org/docs/STABLE/group__Numeric.html))

### 1c. Money crates

| Crate | Latest (date) | Downloads total / recent | Internals | Verdict |
|---|---|---|---|---|
| [`rusty-money`](https://docs.rs/rusty-money/0.5.1/rusty_money/) | 0.5.1 (2026-09-25), MIT | 1.02M / 200k | `Money<'a, T>` = `rust_decimal::Decimal` + `&'a Currency`. Optional `FastMoney` (`fast` feature) = `i64` minor units, 16 bytes | Active again in 2026 (0.4.2, 0.5.0, 0.5.1); 107 stars, 15 open issues. Locale model conflicts with Balanced (see below) |
| [`money2`](https://crates.io/crates/money2) | 1.4.0 (2023-11-04), **GPL-3.0-only** | 29k / 197 | Decimal-based | Stale and copyleft; avoid |
| [`steel-cent`](https://crates.io/crates/steel-cent) | 0.2.3 (2019-07-12) | 92k / 404 | `i64` minor units | Unmaintained |
| [`currency_rs`](https://crates.io/crates/currency_rs) | 1.3.1 (2026-08-10), non-standard licence | 28k / 1.9k | Port of currency.js | Niche |
| [`doubloon`](https://crates.io/crates/doubloon) | 3.2.0 (2026-06-30), MIT | 3.4k / 146 | Decimal + currency | Tiny user base |
| [`moneta`](https://crates.io/crates/moneta) | 0.7.1 (2026-07-31) | 13k / 143 | | Tiny user base |
| [`currencies`](https://crates.io/crates/currencies) | 0.4.1 (2024-06-21), MIT | 33k / 938 | | Quiet since 2024 |

**What `rusty-money` gets right:** ISO currencies built in, plus `define_currency_set!` for custom ones. It has `from_minor`/`from_major`/`from_decimal`, `round(dp, Round::HalfUp|HalfDown|HalfEven)`, `split(n)` and `allocate(shares)` (remainder-correct splitting), currency-mismatch errors on `add`/`sub`, and an `exchange` module. ([docs](https://docs.rs/rusty-money/0.5.1/rusty_money/)) The `serde` feature turns on `rust_decimal/serde-str` (crates.io feature list).

**Why it does not fit Balanced:**

1. **Locale belongs to the currency.** The ISO `Currency` struct has a `locale: Locale` field, for example `USD { exponent: 2, locale: EnUs, symbol: "$", symbol_first: true, … }`. Both `Display for Money` and `Money::from_str` call `LocalFormat::from_locale(currency.locale())`. Sources: [`src/money.rs`](https://github.com/varunsrin/rusty_money/blob/master/src/money.rs) (lines ~48, ~721) and [`src/currency/iso_currencies.rs`](https://github.com/varunsrin/rusty_money/blob/master/src/currency/iso_currencies.rs). A German user would therefore see USD formatted US-style. Symbol placement (`symbol_first`) is also per currency, whereas CLDR places the symbol per locale.
2. **Only four locales exist:** `enum Locale { EnUs, EnIn, EnEu, EnBy }`, each a hand-coded separator, grouping pattern and decimal mark ([`src/locale.rs`](https://github.com/varunsrin/rusty_money/blob/master/src/locale.rs)). There is no CLDR data, no locale digits, and no pseudo-locale.
3. **The borrowed currency `&'a T`** puts a lifetime on every struct that holds money. That is workable with `&'static` ISO currencies, but noisy in GUI state and in serde, where deserialising needs a `Findable` lookup.

Its `allocate`/`split` algorithms and currency-mismatch checks are worth copying into Balanced's own `Money` type.

### 1d. Other numeric types (for completeness)

- [`bigdecimal`](https://crates.io/crates/bigdecimal) 0.4.11 (2026-09-28), 128M downloads. This is arbitrary precision on the heap and is not `Copy`. rustledger uses it only where totals exceed `Decimal`'s range.
- [`fastnum`](https://crates.io/crates/fastnum) 0.7.5 (2026-06-11), 9.2M downloads, MSRV 1.94. Fixed-width decimals (D64/D128, …). Younger, and the MSRV is very recent.
- [`fixed`](https://crates.io/crates/fixed) is binary fixed-point, so `0.1` is not exact. It is unsuitable for money.
- `f64` is never acceptable for stored amounts. rusty-money itself names its conversion `to_f64_lossy`.

## 2. ISO 4217 currency codes and minor units

- The authoritative source is the ISO 4217 maintenance agency (SIX), [List One XML](https://www.six-group.com/dam/download/financial-information/data-center/iso-currrency/lists/list-one.xml), published 2026-09-17. Its 178 distinct codes break down by minor units (`CcyMnrUnts`) as follows:
  - **2**: 139 codes
  - **0**: 17 codes (JPY, KRW, VND, CLP, ISK, XOF, XAF, XPF, BIF, DJF, GNF, KMF, PYG, RWF, UGX, UYI, VUV)
  - **3**: 7 codes (BHD, IQD, JOD, KWD, LYD, OMR, TND)
  - **4**: 2 codes (CLF, UYW)
  - **N.A.**: 13 codes (XAU, XAG, XPT, XPD, XDR, XTS, XXX, XBA–XBD, XSU, XUA)

  The exponent is therefore not "always 2", and some codes have none at all.
- [`iso_currency`](https://docs.rs/iso_currency/0.7.1/iso_currency/) 0.7.1 (2026-09-06), MIT, 2.38M downloads (578k recent), with releases in 2026-08 and 2026-09. `Currency` is an enum. It offers `code()`, `numeric()`, `name()`, `symbol()`, `exponent() -> Option<_>` (EUR `Some(2)`, JPY `Some(0)`, `None` where ISO says N.A.), `subunit_fraction()`, `from_code()`, `from_numeric()`, `used_by()`/`from_country()`, `is_fund()`/`is_special()`, and `is_superseded()`/`latest()` (VES → VED). Features: `with-serde`, `iterator` (strum), `with-schemars`, `with-specta`, `with-sqlx-{sqlite,postgres,mysql}`. The docs say the data is taken from the English Wikipedia ISO 4217 article, not the SIX file, so it should be spot-checked against List One.
- [`iso4217`](https://crates.io/crates/iso4217) 0.3.2 has not been released since 2018 and is stale.
- CLDR adds what ISO lacks: per-currency display `digits`, a `rounding` increment, and `cashDigits`/`cashRounding`. For example, CHF with `rounding="5"` rounds "to the nearest 0.05 units in formatting". ([UTS #35 Numbers, Supplemental Currency Data](https://www.unicode.org/reports/tr35/tr35-numbers.html), v48.2) This is relevant if Balanced ever shows cash-rounded amounts. ISO's minor unit remains the storage and accounting precision.
- Custom or non-ISO currencies (crypto, loyalty points, a security's share "currency") need an escape hatch. ISO-only enums cannot express them. A `CurrencyCode` newtype over a 3–8 char string, with metadata looked up from a table that ISO seeds, keeps that door open.

## 3. Rounding

- Banker's rounding (`MidpointNearestEven`) is what GnuCash recommends for financial quantities ("`GNC_HOW_RND_ROUND` … recommended"). Many banks and statements, however, use half-up (`MidpointAwayFromZero`), so this is a policy choice. Name it per operation (interest, split, FX), and leave no implicit global default.
- Round only at boundaries: after multiplication or division, and when converting to the currency's exponent for posting. Addition and subtraction of same-scale `Decimal`s is exact.
- When splitting a total (split transactions, budget allocation), allocate the remainder so that the parts sum back exactly, as rusty-money's `allocate` does. Do not round each part independently.

## 4. Formatting for display (locale-aware, currency-independent)

- **ICU4X `icu_decimal`** [2.3.0](https://docs.rs/icu_decimal/2.3.0/icu_decimal/) (2026-08-13), Unicode-3.0 licence, 14.8M downloads, MSRV 1.88. This is stable 2.x. `DecimalFormatter::try_new(locale, options).format(&decimal)` uses CLDR separators, grouping (Indian lakh grouping for `bn`/`hi`) and native digits (`th-u-nu-thai`). Its input is `fixed_decimal::Decimal` ([`fixed_decimal`](https://crates.io/crates/fixed_decimal) 0.7.2). The docs example builds `2,000.50` from `Decimal::from(200050)` plus `multiply_pow10(-2)`, which is exactly the shape of a `rust_decimal` mantissa and scale, so bridging is cheap.
- **ICU4X `icu_experimental`** [0.6.0](https://docs.rs/icu_experimental/0.6.0/icu_experimental/dimension/currency/formatter/struct.CurrencyFormatter.html) (2026-08-13), with `dimension::currency::formatter::CurrencyFormatter`. It provides `try_new_code`, `try_new_symbol`, `try_new_no_currency`, `try_new_name` (giving "12,345.67 US dollars") and compact variants, then `format_fixed_decimal(&value)`. The docs example formats `en-US` + `USD` as `"USD\u{a0}12,345.67"`. The **locale and currency are separate inputs**, which is exactly Balanced's model. Caveat: the crate states "all code in this crate is unstable". It is versioned 0.x and has broken API in each release (0.4 → 0.5 → 0.6 in 2025-10, 2026-04 and 2026-08). The `compiled_data` feature bundles CLDR data.
- CLDR places the symbol per locale through the `¤` position in `currencyFormat` patterns, with `alt="alphaNextToNumber"` spacing variants since CLDR 42 ([UTS #35](https://www.unicode.org/reports/tr35/tr35-numbers.html)). That is why a per-currency `symbol_first` flag, as in rusty-money, is the wrong model.
- [`num-format`](https://crates.io/crates/num-format) 0.4.4 has had no release since 2022-12-03, though it still has 74M downloads. It formats integers only, with no decimals or currency. Do not adopt it.
- Other bridging crates (seen in rustledger): `format_num_pattern` with a `rust_decimal` feature. It is pattern-based rather than locale-based.

**Practical options for Balanced:**

- **(a)** Use `icu_decimal` for the number, and put the currency symbol, its position and the negative style in Balanced's own small table per locale. This is stable, but duplicates some CLDR.
- **(b)** Use `icu_experimental::CurrencyFormatter` behind a thin Balanced `format_money(locale, &Money)` seam, pinned to an exact version, so API churn stays in one file.

Either way, the egui layer calls one Balanced function and never the crates directly.

## 5. Parsing user input

- No crate surveyed offers **locale-aware number parsing** decoupled from currency. `icu_decimal` documents only formatters (`DecimalFormatter`, `CompactDecimalFormatter`), with no parser. `rust_decimal::FromStr` accepts only `.` as the decimal mark, with no grouping. rusty-money's `Money::from_str` is locale-aware, but uses the currency's locale (see §1c).
- Balanced therefore needs its own parser. Take the user locale's decimal and grouping symbols (from ICU4X data, or from the same table as the formatter), strip grouping, map the decimal mark to `.`, accept a leading or trailing sign, accept parentheses or a trailing minus if the register UI supports them, and optionally ignore the currency symbol. Then call `Decimal::from_str_exact`. Finally, reject input with more fraction digits than the currency's exponent, or round it under the named policy. This is a few dozen lines and an obvious target for property and mutation tests (format → parse round-trip per locale, including the pseudo-locale).

## 6. Serde and persistence

- `rust_decimal`: prefer string serialisation (`serde-str` or `serde-with-str`) for any persisted or JSON form, so precision and scale survive. Avoid `serde-float`. Note that Wealthfolio enables `serde-float` for its JS frontend, while tackler uses `serde-with-arbitrary-precision`.
- `iso_currency`: `with-serde` serialises the code. For SQLite, `with-sqlx-sqlite` exists if sqlx is chosen. Persistence is out of scope for #5.
- Integer minor units serialise trivially, but the exponent must be stored or derived from the currency.

## 7. What real Rust finance apps chose

All four use **`rust_decimal`**, and none uses `rusty-money` or integer minor units:

- **tackler** (plain-text accounting; [tackler-ng/tackler](https://github.com/tackler-ng/tackler), 168 stars, pushed 2026-10-08) declares `rust_decimal = { version = "1.43.0", features = ["serde-with-arbitrary-precision"] }` plus `rust_decimal_macros` in its workspace `Cargo.toml`.
- **rustledger** (Beancount-compatible; [rustledger/rustledger](https://github.com/rustledger/rustledger), 408 stars, pushed 2026-10-13) declares `rust_decimal = { version = "1.42", features = ["serde"] }`, with `maths` in core. It also uses `bigdecimal = "0.4"` for totals past `Decimal`'s range, and `format_num_pattern` (with its `rust_decimal` feature) for display.
- **beancount-parser-lima** ([tesujimath/beancount-parser-lima](https://github.com/tesujimath/beancount-parser-lima)) declares `rust_decimal = "1.40"`.
- **Wealthfolio** (Tauri desktop personal-finance tracker; [wealthfolio/wealthfolio](https://github.com/wealthfolio/wealthfolio), 9.2k stars, pushed 2026-10-13) declares `rust_decimal = { version = "1.39", features = ["maths", "serde-float", "serde-with-str"] }` across its crates.

These come from `gh search code --filename Cargo.toml decimal` on each repo, 2026-10-13.

For contrast, outside Rust: Actual Budget (TypeScript) uses integer minor units, and GnuCash (C) uses 64/64-bit rationals.

## Recommendation for #14

1. `Money { amount: rust_decimal::Decimal, currency: CurrencyCode }`, owned and `Copy`-able, with no lifetimes. Same-currency arithmetic is checked, and a currency mismatch is a type-level or `Result` error.
2. `CurrencyCode` is a newtype. ISO metadata (exponent, symbol) comes from `iso_currency` or a table generated from SIX List One, with room for non-ISO codes.
3. Amounts posted to a register are normalised to the currency's exponent through one named rounding policy. Prices, quantities and rates stay as raw `Decimal`.
4. Display goes through one seam, `format_money(&Locale, &Money)`, built on ICU4X. Start with `icu_decimal` (stable), and decide (a) versus (b) in §4 when #14 settles the symbol and negative-style requirements.
5. Input goes through one seam, `parse_money(&Locale, CurrencyCode, &str)`, written in-house and round-trip tested against the formatter.

## Open questions for #14

- Should negative display (`-$5.00` vs `($5.00)` vs `5,00 $-`) follow CLDR's accounting pattern, or be a user preference?
- Should Balanced accept more precision than the exponent at input time, for example fuel prices like `1.239` USD?
- Is `icu_experimental` churn acceptable behind a seam, or do we keep a hand-maintained symbol table on top of `icu_decimal`?
