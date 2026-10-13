# Internationalisation in Rust/egui

Research for [#9](https://github.com/StanleyGoldman/balanced/issues/9) (child of the wayfinder map #5). It feeds #13, "Choose the internationalisation approach".

Researched 2026-10-13. Crate metadata comes from the crates.io API (`https://crates.io/api/v1/crates/<name>`) on that date; "last release" is the publish date of the newest version. Source claims were checked against each repo's default branch on that date: egui `b25d2d45b7`, fluent-rs `0f87afbbc3`, cargo-i18n `10e345470d`, rust-i18n `5f3ea5ba03`. Where egui behaviour matters, it was also checked against the released 0.36.2 source.

Money formatting is covered by the sibling note `docs/research/rust-money-representation.md` (branch `research/rust-money-representation`, #10). This note builds on its conclusion (ICU4X: `icu_decimal` 2.x stable, `icu_experimental` `CurrencyFormatter` unstable, Balanced-owned `format_money` / `parse_money` seams) and does not repeat it.

## Question

Which internationalisation stack fits a Rust/egui app that needs translatable text (with plurals and text expansion) and locale formatting (numbers, currency placement, dates, first day of week)? Locale must stay independent of an account's currency. Already decided in #5: fallback chain exact locale → base language → `en-US`; formatting follows the user's locale even when text falls back; missing keys log in debug builds and fail a CI completeness test; proof target `en-US`, `de-DE` and a pseudo-locale; RTL and CJK deferred but assessed.

## Answer in brief

- **Text: Project Fluent, through `i18n-embed` + `i18n-embed-fl`.** Fluent is the only candidate with real CLDR plural selection and translator-controlled variants. `i18n-embed-fl`'s `fl!` macro checks message ids and argument names **at compile time** against the `en-US` files. `rust-i18n` has no plurals and no compile-time key check. gettext bindings add a C/LGPL dependency and process-global locale state.
- **Formatting: ICU4X, separately from Fluent.** Fluent's own number formatting is not locale-aware (`f64::to_string`). Balanced formats numbers, money and dates with ICU4X (`icu_decimal`, `icu_datetime`, `icu_calendar::week`) using the user's **formatting locale**, and passes the formatted strings into messages. Fluent's `set_formatter` hook can route plain `{ $count }` numbers through the same ICU4X code.
- **Fallback chain:** i18n-embed already walks a language list and fills missing messages from the fallback bundle. The chain Balanced wants is easy to build explicitly (`de-AT` → `de` → `en-US`). The fill-in is **silent**, so Balanced has to add the debug-build log itself.
- **CI completeness:** compile-time `fl!` guarantees every used id exists in `en-US`. A small test using `fluent-syntax` compares every other locale's message ids, attributes and variables against `en-US`.
- **Pseudo-locale:** `fluent-pseudo` plugs into `FluentBundle::set_transform`. **egui's default font lacks 15 of the accented pseudo glyphs**, so Balanced either bundles a broader font or ships its own transform map limited to covered glyphs.
- **egui wiring:** immediate mode means a locale switch shows on the next frame with no invalidation step. Grids and tables re-measure over one or two passes. Released egui (0.36.2) cannot shape RTL text correctly and only covers Latin, Greek and Cyrillic in its built-in font. The unreleased 0.37 adds system-font fallback for CJK and other scripts. RTL/bidi remains an open egui issue.

## 1. Translatable text: the candidates

| Crate | Latest (date) | Licence | Downloads total / recent | Repo activity |
|---|---|---|---|---|
| [`fluent-bundle`](https://docs.rs/fluent-bundle/0.16.0/fluent_bundle/) | 0.16.0 (2025-05-22) | Apache-2.0 OR MIT | 20.9M / 7.3M | [projectfluent/fluent-rs](https://github.com/projectfluent/fluent-rs), 1.4k stars, pushed 2026-09-27 |
| [`fluent`](https://docs.rs/fluent/0.17.0/fluent/) (umbrella) | 0.17.0 (2025-05-23) | Apache-2.0 OR MIT | 15.5M / 6.2M | same |
| [`fluent-syntax`](https://docs.rs/fluent-syntax/0.12.0/fluent_syntax/) | 0.12.0 (2025-05-22) | Apache-2.0 OR MIT | 21.0M / 7.4M | same |
| [`fluent-pseudo`](https://docs.rs/fluent-pseudo/0.3.3/fluent_pseudo/) | 0.3.3 (2025-05-22) | Apache-2.0 OR MIT | 572k / 229k | same |
| [`fluent-langneg`](https://docs.rs/fluent-langneg/0.14.2/fluent_langneg/) | 0.14.2 (2025-10-24) | Apache-2.0 OR MIT | 21.0M / 7.3M | [projectfluent/fluent-langneg-rs](https://github.com/projectfluent/fluent-langneg-rs) |
| [`i18n-embed`](https://docs.rs/i18n-embed/0.16.0/i18n_embed/) | 0.16.0 (2025-07-09) | MIT | 7.9M / 3.9M | [kellpossible/cargo-i18n](https://github.com/kellpossible/cargo-i18n), 147 stars, pushed 2026-09-26 |
| [`i18n-embed-fl`](https://docs.rs/i18n-embed-fl/0.10.1/i18n_embed_fl/) | 0.10.1 (2026-07-23) | MIT | 7.7M / 3.9M | same |
| [`fluent-templates`](https://docs.rs/fluent-templates/0.15.1/fluent_templates/) | 0.15.1 (2026-08-05) | MIT/Apache-2.0 | 1.2M / 218k | [XAMPPRocky/fluent-templates](https://github.com/XAMPPRocky/fluent-templates), 168 stars |
| [`rust-i18n`](https://docs.rs/rust-i18n/4.2.4/rust_i18n/) | 4.2.4 (2026-09-29) | MIT | 4.0M / 1.4M | [longbridge/rust-i18n](https://github.com/longbridge/rust-i18n), 678 stars, pushed 2026-09-29 |
| [`gettext-rs`](https://docs.rs/gettext-rs/0.8.0/gettextrs/) | 0.8.0 (2026-08-05) | MIT (bundles LGPL GNU gettext) | 4.9M / 634k | [gettext-rs/gettext-rs](https://github.com/gettext-rs/gettext-rs), 66 stars |
| [`gettext`](https://crates.io/crates/gettext) (pure Rust `.mo` reader) | 0.4.0 (2019-06-04) | MIT | 1.3M / 365k | stale |
| [`tr`](https://crates.io/crates/tr) | 0.1.11 (2025-06-23) | MIT | 1.7M / 203k | [woboq/tr](https://github.com/woboq/tr) |

Maintenance: fluent-rs releases are slow (the last crate releases were in May 2025), but the repo is active and the crates are used by Firefox. i18n-embed-fl had a release in July 2026. rust-i18n releases often.

### 1a. Project Fluent (`fluent-bundle` and friends)

- **Plurals and variants.** Fluent selectors pick a variant by CLDR plural category: `{ $count -> [one] … *[other] … }`. In `FluentValue::matches`, a string key such as `one` against a `FluentValue::Number` is resolved through plural rules, cardinal or ordinal ([`fluent-bundle/src/types/mod.rs`](https://github.com/projectfluent/fluent-rs/blob/0f87afbbc3/fluent-bundle/src/types/mod.rs#L177)). The plural data comes from [`intl_pluralrules`](https://crates.io/crates/intl_pluralrules) 7.0 (last release 2022-10-19), not from ICU4X. That is separate CLDR data, but plural rules for `en` and `de` are stable.
- **Number formatting is not locale-aware.** `FluentNumber::as_string` is `self.value.to_string()` plus `minimumFractionDigits` padding ([`types/number.rs`](https://github.com/projectfluent/fluent-rs/blob/0f87afbbc3/fluent-bundle/src/types/number.rs#L148)). `currency`, `useGrouping` and similar options are parsed but not applied. A `de-DE` message with `{ $count }` = 1234.5 would print `1234.5`. The values are `f64`, so they must never carry money.
- **Hooks Balanced needs** ([`bundle.rs`](https://github.com/projectfluent/fluent-rs/blob/0f87afbbc3/fluent-bundle/src/bundle.rs#L320-L354)):
  - `set_formatter(fn(&FluentValue, &M) -> Option<String>)` is "particularly useful for plugging in an external formatter for `FluentValue::Number`". It is called whenever a value is written out ([`types/mod.rs#L223`](https://github.com/projectfluent/fluent-rs/blob/0f87afbbc3/fluent-bundle/src/types/mod.rs#L223)). Selection still sees the raw number, so plurals keep working. It is a plain `fn` pointer and only gets the *bundle's* language. To honour "format in the user's locale even when text falls back", it has to read Balanced's current formatting locale from a process-wide cell.
  - `set_transform(fn(&str) -> Cow<str>)` runs on text fragments, not on placeables. It is "primarily used for pseudolocalization".
  - `set_use_isolating(bool)` (default `true`) wraps placeables in U+2068/U+2069 isolation marks.
  - `add_function(id, closure)` lets Balanced register custom functions, for example `DATE()` backed by ICU4X.
- **Fallback.** `fluent-bundle` is single-locale. Chaining is provided by `fluent-fallback`, by i18n-embed, or by fluent-templates.

### 1b. `i18n-embed` + `i18n-embed-fl` (Fluent loader and compile-checked macro)

- **Loader.** `FluentLanguageLoader::new(domain, fallback_language)` loads `.ftl` files embedded with `rust-embed` (default feature), or from the filesystem (`filesystem-assets`, plus `autoreload` through `notify`). It keeps one `FluentBundle` per selected language ([`i18n-embed/Cargo.toml`](https://github.com/kellpossible/cargo-i18n/blob/10e345470d/i18n-embed/Cargo.toml)). The optional `system-requester` feature reads the OS locale through `sys-locale`.
- **Fallback lookup.** `get_args_fluent` walks the current languages in order and returns the first bundle that has the message. If none does, it logs `log::error!` and returns `No localization for id: "…"` ([`i18n-embed/src/fluent.rs#L145-L186`](https://github.com/kellpossible/cargo-i18n/blob/10e345470d/i18n-embed/src/fluent.rs#L145-L186)). `select_languages` appends the fallback language when it is missing from the list (L443–L475). Falling back to `en-US` for one message is **silent**, so Balanced adds its own debug-build check (§3).
- **Building the chain.** `select_languages(&[de-AT, de, en-US])` gives the chain from #5 exactly. Alternatively, `select_languages_negotiate` uses `fluent-langneg`. Its Filtering strategy is "based on the BCP47 3.3.2 Extended Filtering algorithm, with several modifications". It also matches *sibling regions* (`en-GB` matches `en-AU`) ([docs](https://docs.rs/fluent-langneg/0.14.2/fluent_langneg/negotiate/index.html)), so `de-AT` could pick `de-DE` before `de`. That is broader than #5's chain. Build the list explicitly, or use ICU4X `LocaleFallbacker` (§2d).
- **Compile-time checks.** `fl!(loader, "message-id", name = value)` loads the **fallback-language** files at compile time (configured in `i18n.toml`). It aborts compilation if the message id or attribute does not exist, or if a named argument is not in the message ([`i18n-embed-fl/src/lib.rs#L227-L332`](https://github.com/kellpossible/cargo-i18n/blob/10e345470d/i18n-embed-fl/src/lib.rs#L227)). The hash-map argument form is **not** checked. Other locales are not checked at all.
- `set_use_isolating`, and `with_bundles_mut(|b| …)` for applying `set_transform`/`set_formatter`/`add_function` to every bundle ([`fluent.rs#L405-L440`](https://github.com/kellpossible/cargo-i18n/blob/10e345470d/i18n-embed/src/fluent.rs#L405)).
- MSRV 1.85.1 (workspace `Cargo.toml`), well below egui's 1.95.

### 1c. `fluent-templates`

`static_loader!` embeds `.ftl` files at compile time with a `fallback_language` and a shared `core_locales` file ([README](https://github.com/XAMPPRocky/fluent-templates)). It suits web templating (Tera, Handlebars). Its README does not advertise compile-time message-id checks, so it gives Balanced less than `i18n-embed-fl`.

### 1d. `rust-i18n`

- YAML/JSON/TOML key → string files, compiled in by `i18n!("locales", fallback = "en")`, with a global `t!("key", name = value)` and `set_locale` ([README](https://github.com/longbridge/rust-i18n)). It supports a fallback chain and automatic `zh-CN` → `zh` territory fallback (since 2.4). The `log-miss-tr` feature warns on misses, and `cargo i18n` (rust-i18n-cli) extracts untranslated keys.
- **No plurals.** Issue [#65](https://github.com/longbridge/rust-i18n/issues/65) ("plans to implement pluralization like rails?") has been open since 2023-10. Interpolation is plain `%{name}` substitution.
- **No compile-time key check.** The generated code looks keys up at runtime and returns the key or the source text when a key is missing ([`crates/macro/src/tr.rs#L439-L470`](https://github.com/longbridge/rust-i18n/blob/5f3ea5ba03/crates/macro/src/tr.rs#L439)).
- Verdict: it fails the plural requirement.

### 1e. gettext

- `gettext-rs` wraps GNU gettext through FFI. Off glibc/musl it "compiles GNU gettext, which is licensed under LGPL. This means **you have to abide by LGPL**" unless it links dynamically ([gettext-sys README](https://github.com/gettext-rs/gettext-rs/blob/master/gettext-sys/README.md)). That matters on Windows and macOS. gettext selects the language through process-global locale state, which is awkward for switching locales inside the app. Plurals use `ngettext` with C-expression plural forms in the `.po` header rather than CLDR categories.
- The pure-Rust `gettext` crate has had no release since 2019. `tr` (from the Slint authors) offers gettext-style macros over `gettext`/`gettext-rs`.
- Verdict: workable with mature tooling (Poedit, Weblate), but there is no compile-time id checking (the source string *is* the id), it adds a C build, and it duplicates locale logic that ICU4X already provides.

### 1f. Others

`es-fluent` 0.21.0 (2026-10-03, about 12k downloads, MSRV 1.99) and `l10n` 0.1.3 have too few users to bet on.

## 2. Locale formatting (ICU4X)

All ICU4X 2.x components are at 2.3.x (2026-08-13/20), Unicode-3.0 licence, MSRV 1.88, repo [unicode-org/icu4x](https://github.com/unicode-org/icu4x) (pushed 2026-10-13).

### 2a. Numbers and money

See the money note. `icu_decimal::DecimalFormatter` handles separators and grouping. `format_money(&Locale, &Money)` sits on `icu_decimal`, or on `icu_experimental::CurrencyFormatter` behind a seam. Inside Fluent, money is always passed as an **already-formatted string argument**: `{ $amount }` where `amount` = `format_money(user_locale, &money)`. Locale and currency stay independent because `format_money` takes them as separate inputs.

### 2b. Dates

[`icu_datetime`](https://docs.rs/icu_datetime/2.3.0/icu_datetime/) 2.3.0: `DateTimeFormatter::try_new(locale.into(), fieldsets::YMD::medium())`, then `.format(&date)`. The docs example renders `es-AR` as `"15 de ene de 2025, 4:09 p. m."`. Binary-size guidance: static field sets (`YMD`) let unused pattern data be dropped, and `FixedCalendarDateTimeFormatter<Gregorian>` avoids linking every calendar's data. A Gregorian-only finance app should use it. ICU4X documents formatters only; date **input** parsing is Balanced's own, as with amounts.

### 2c. First day of week

[`icu_calendar::week::WeekInformation`](https://docs.rs/icu_calendar/2.3.0/icu_calendar/week/struct.WeekInformation.html)`::try_new(prefs)` returns `first_weekday` and `weekend`. Per the docs example, `en-US` → Sunday and `de-DE` → Monday. `en-US-u-rg-dezzzz` (region override) → Monday, and adding `-u-fw-wed` → Wednesday. A user "first day of week" preference can therefore be stored as a Unicode locale extension on the formatting locale, with no separate code path.

### 2d. Locale fallback data

[`icu_locale::fallback::LocaleFallbacker`](https://docs.rs/icu_locale/2.3.1/icu_locale/fallback/struct.LocaleFallbacker.html) implements the UTS #35 inheritance chain, ending at `und`. The documented example is `hi-Latn-IN` → `hi-Latn` → `en-IN` → `en-001` → `en` → `und`. ICU4X's own formatters use it internally, so a `de-AT` user gets `de-AT` number and date data even when the text is `de` or `en-US`. That is the "formatting follows the user's locale" rule, for free.

### 2e. Two locales, kept apart

The design that falls out:

- **text language list:** for example `[de-AT, de, en-US]` or `[pseudo]`. It drives Fluent.
- **formatting locale:** the user's full locale, for example `de-AT-u-fw-mon`. It drives ICU4X and Fluent's `set_formatter` hook.
- **account currency:** a property of the data, never of either locale.

## 3. Missing keys: debug log and CI test

- **Compile time:** `fl!` with literal ids guarantees every id used in code exists in `en-US`, along with its named arguments.
- **Debug log at runtime:** i18n-embed fills gaps silently from later languages (§1b). In debug builds, after `load_languages`, use `with_message_iter(lang, …)` to compare each selected language's message ids with `en-US`, and `log::warn!` each gap once. Alternatively, wrap lookups in a Balanced `tr()` that checks the first bundle.
- **CI completeness test:** parse every `locales/<lang>/*.ftl` with `fluent-syntax` (`parser::parse` → `ast::Resource`). For each locale except `en-US`, assert:
  - every `en-US` message and attribute id exists;
  - no extra ids exist (catching stale keys);
  - the set of `$variables` referenced matches.

  This is a few dozen lines in a normal `#[test]`. A grep-based check can also flag `en-US` ids that no `fl!` call uses.

## 4. Pseudo-localisation

- `fluent_pseudo::transform(s, flipped, elongate)` maps a–z/A–Z to accented look-alikes. With `elongate`, it doubles a/e/o/u "to emulate ~30% longer text" (`"Hello World"` → `"Ħeeŀŀoo Ẇoořŀḓ"`). `flipped` gives upside-down text for a mock-RTL look ([`fluent-pseudo/src/lib.rs`](https://github.com/projectfluent/fluent-rs/blob/0f87afbbc3/fluent-pseudo/src/lib.rs#L65)). It plugs into `set_transform`, so the pseudo-locale is just the `en-US` resources plus a transform. Placeables (formatted amounts and dates) are not mangled. The plain `transform` adds no `[…]` boundary markers (only `transform_dom` has `with_markers`), so Balanced should add brackets in its own transform fn to make truncation visible.
- **Font gap (checked with `fc-query` against egui's bundled `Ubuntu-Light.ttf` + `NotoEmoji-Regular.ttf` + `emoji-icon-font.ttf`):**
  - The **accented map** is missing 15 glyphs: ḓ ɠ ḿ ṽ ẇ ẋ ẏ ẑ and Ḓ Ḿ Ṽ Ẇ Ẋ Ẏ Ẑ (U+1E12–U+1E91 and U+0260). Released egui draws these as a replacement box.
  - The **flipped map** is missing 26 glyphs.
  - German (äöüßÄÖÜ€) is fully covered.

  Options: bundle a font with Latin Extended Additional coverage (Noto Sans covers it), rely on 0.37 system-font fallback (§5c), or write a Balanced transform whose map only uses covered glyphs. The transform is about 30 lines.
- **Formatting for the pseudo-locale:** ICU4X has no pseudo-locale data. `en-XA`/`ar-XB` support is only under discussion in [icu4x#8398](https://github.com/unicode-org/icu4x/issues/8398) (opened 2026-08-18), so the pseudo-locale formats with a real locale. Making it configurable and defaulting to `de-DE` catches hard-coded `.`/`,` assumptions alongside the text checks.

## 5. egui wiring

### 5a. Locale switch and re-layout

- egui is immediate mode: `fl!(…)` runs every frame, so changing the loader's language (or the formatting-locale cell) takes effect on the next frame. Call `ctx.request_repaint()` after switching. Galleys are cached by text, so no manual invalidation is needed. Do not cache translated `String`s in app state; store message ids instead.
- Size-dependent containers measure from the previous pass. `Grid` "will do a best guess, but it will likely be wrong. Next pass it can read the sizes from the previous pass", and calls `request_discard` to hide the glitch, which is capped by `Options::max_passes` (default 2) ([`context.rs#L2300-L2321`](https://github.com/emilk/egui/blob/b25d2d45b7/crates/egui/src/context.rs#L2300)). Expect one or two frames of settling after a switch. Persisted widths (for example `egui_extras` table columns sized for English) may need resetting. Verify this in the prototype.
- egui has almost no built-in text of its own. Upstream chose to "limit the amount of text in `egui`" rather than localise it ([egui#136](https://github.com/emilk/egui/issues/136), closed 2022-12). So nearly all visible strings will be Balanced's.
- Bidi isolation marks from Fluent (U+2066–U+2069) are in egui's `invisible_char` list, which makes them zero-width ([`epaint/src/text/unicode.rs`](https://github.com/emilk/egui/blob/b25d2d45b7/crates/epaint/src/text/unicode.rs), also in 0.36.2). Leaving `use_isolating` on is harmless on screen. Strings copied to the clipboard and string-equality tests will contain them, so disable isolation in test bundles or strip it in assertions.

### 5b. Spaces CLDR emits

CLDR uses U+00A0 (`1.234,56 €`) and U+202F (`fr` grouping, `en` time "4:09 PM"). egui synthesises width for U+202F and the thin space ([`font_face.rs#L508`](https://github.com/emilk/egui/blob/b25d2d45b7/crates/epaint/src/text/font_face.rs#L508)). However, its line breaker only exempts U+00A0 from break opportunities ([`text_layout.rs#L1415`](https://github.com/emilk/egui/blob/b25d2d45b7/crates/epaint/src/text/text_layout.rs#L1415)), so wrapped labels may break inside `4:09 PM` or a French amount. This is minor, since register cells do not wrap. It is worth an upstream issue if it shows up.

### 5c. Fonts and CJK (fog)

- **egui 0.36.2 (released 2026-09-08):** the bundled fonts are Ubuntu-Light, Hack, NotoEmoji, emoji-icon-font and egui-icons. Per the in-tree `egui_system_fonts` README, "The default egui fonts cover Latin, Greek, and Cyrillic", and other scripts draw `◻`. CJK needs the app to ship a font and add it to the fallback list with `Context::add_font` / `set_fonts(FontDefinitions)`. A Noto Sans CJK subset is large (tens of MB for the full set).
- **egui 0.37 (unreleased on main):** `Context::add_font_provider`. eframe's new default `system_fonts` feature uses [`fontique`](https://crates.io/crates/fontique) to "load system fonts on demand for characters that the installed fonts lack, e.g. CJK, Arabic, or Devanagari", controlled by `NativeOptions::system_font_fallback` (default `true`). 0.37 also makes `default_fonts` non-default ([CHANGELOG](https://github.com/emilk/egui/blob/b25d2d45b7/CHANGELOG.md), #8512, #8583). The crates.io name `egui-system-fonts` is currently a different third-party crate (0.36.0, 2026-08-15, yijehyung/egui-system-fonts).
- Shaping: since 0.35, egui shapes with `harfrust` and renders with `skrifa` + `vello_cpu`. The changelog notes the font refactor is "a step towards using parley". egui already has CJK-aware line breaking (`is_cjk`, `is_cjk_break_allowed`).
- Implication: CJK is a font-and-packaging problem, not an architecture problem. Fluent and ICU4X handle CJK locales already.

### 5d. RTL / bidi (fog)

- There is no bidi algorithm in egui. `invisible_char` carries `// TODO(emilk): heed bidi characters`. Run segmentation "is by font face, not by Unicode script … would need script-aware splitting once RTL/bidi support is added" ([`text_layout.rs#L1500`](https://github.com/emilk/egui/blob/b25d2d45b7/crates/epaint/src/text/text_layout.rs#L1500)).
- [egui#1016](https://github.com/emilk/egui/issues/1016) "Bidirectional text support" has been open since 2021, with Parley and cosmic-text ([#3378](https://github.com/emilk/egui/issues/3378)) suggested. Since harfrust shaping, RTL runs also break `TextEdit` caret and selection ([egui#8576](https://github.com/emilk/egui/issues/8576), open, 2026-10-11).
- egui's `Layout::right_to_left` mirrors widget placement only, not text.
- Implication: RTL languages are **not viable on egui today**. Keep the door open by using Fluent (bidi isolation is built in), avoiding string concatenation, and keeping layout direction in one place. Revisit when egui adopts Parley.

## 6. Locale detection

[`sys-locale`](https://crates.io/crates/sys-locale) 0.3.2 (2024-11-01, MIT OR Apache-2.0, 37M downloads, 1Password) returns the OS locale as a BCP-47 string. i18n-embed's `system-requester` feature wraps it. Balanced stores an explicit user choice and uses the OS value only as the default.

## Recommendation for #13

1. **Text:** Fluent `.ftl` files under `locales/<lang>/balanced.ftl`, loaded by `i18n-embed` (`fluent-system`, `rust-embed`), accessed only through `fl!` with literal ids and named arguments. A thin `balanced::i18n` module owns the loader, the language list and the hooks.
2. **Fallback chain:** compute the list explicitly (exact → base language → `en-US`; use `LocaleFallbacker` if parents beyond the base language are wanted), and call `select_languages`.
3. **Formatting:** ICU4X with the user's formatting locale, behind Balanced seams: `format_money`/`parse_money` (from #10), `format_date`, `first_weekday`. Install `set_formatter` on every bundle so stray `FluentValue::Number`s also format through `icu_decimal`. Pass money and dates to Fluent as preformatted strings.
4. **Missing keys:** use `fl!` compile checks for `en-US`, a debug-build gap log at language load, and a `fluent-syntax`-based CI test that compares ids, attributes and variables per locale.
5. **Pseudo-locale:** use `en-US` resources plus a Balanced transform (accent and elongate, with bracket markers, glyphs limited to the shipped font) and a configurable formatting locale (default `de-DE`).
6. **Fonts:** for the proof target, egui's default font suffices for `en-US` and `de-DE`. Decide on bundling a Noto font (pseudo glyphs, future CJK) versus relying on 0.37 system fallback when 0.37 ships.

## Open questions for #13

- Should the pseudo-locale use a Balanced-owned transform (glyphs guaranteed) or `fluent-pseudo` plus a bundled broader font?
- Is a user-selectable formatting locale (separate from UI language) exposed in settings, or always derived from the OS?
- Is RTL officially out of scope until egui supports bidi, or does #13 want a spike on Parley-based text in egui?
- Should translators get a TMS-friendly workflow (Pontoon and Weblate support Fluent), or are `.ftl` files edited in the repo?
