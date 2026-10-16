# Theme

Balanced has one look, in light and dark; the decisions behind it are in issue #16, "Prototype theming and density". `balanced_ui`'s `theme` module builds it from a `ThemeSettings`, the view of the Preferences that shape the look (`ThemeSettings::from(&preferences)`). The App rebuilds the `Theme` whenever the Preferences it shows change, and applies its style and fonts with `apply_theme`.

## Fonts

`Theme::fonts` puts the chosen face (Noto Sans or Liberation Sans, bundled in `crates/balanced_ui/fonts/` with their OFL licences) first in the proportional family, with egui's fonts behind it for the glyphs it lacks. Monospace money uses egui's Hack. A new face goes into `FontFace` (its id in `preferences.toml`), `Theme::fonts`, and the font list's labels.

## How a widget gets its colours and sizes

- **Build a `Theme` when the settings change, and apply `theme.style()` to the context.** Standard egui widgets then take their colours, text sizes and spacing from `ui.visuals()` and `ui.spacing()`.
- **Where egui's `Visuals` has no slot, ask the `Theme`:** the chrome and footer frames, the Register header fill, amounts, stripes and gridlines each have a method.
- **A colour the theme lacks becomes a new `Palette` field**, set for both schemes, so every colour stays in the theme module (see [egui](egui.md#colours-and-theme)).
- **Sizes come from the theme or the style:** `row_height()`, `Theme::EDGE_INSET` and `ui.spacing()`.

## Density

Dense rows are a Preference, off by default. `Sizes::of` lists what density changes: row height, the vertical gap, button padding and menu margins. Text size and the gap between columns stay fixed. Layout that reads `row_height()` and `ui.spacing()` follows density with no branch of its own.

## Amounts

Draw every amount on screen with `theme.amount(&money, formatted)`, which applies the monospace-money and coloured-negatives Preferences. The text comes from `Localizer::format_amount`, as [money](money.md#showing-amounts) describes.

## Register stripes and gridlines

These follow the painting notes on issue #17, "Prototype the Register grid", which hold for `egui_extras` 0.36.

- **Stripes:** only the Register stripes. Its `TableBuilder` takes `.striped(theme.alternating_rows())`, and egui_extras paints the stripe colour from the style. A list laid out by hand paints `theme.stripe(row)` behind each row.
- **Header band:** fill it with `theme.header_fill()` across the cell and half the gap around it, through the layer painter clipped to that rect intersected with the scroll viewport, so the band stays inside the Register.
- **Gridlines:** in each cell, after its content, paint `theme.cell_rules(ui.max_rect(), column, ctx.pixels_per_point())` with `ctx.layer_painter(ui.layer_id())`, clipped to `theme.rule_clip(ui.clip_rect(), viewport)`, where `viewport` is the Register's scroll viewport.
- **Keep `TableBuilder::resizable` off.** It draws a drag line at every column boundary, which reads as a gridline the Preference can't turn off.
