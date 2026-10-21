// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Category List: a heavy Dialog showing every category as a tree, with how many
//! transactions and split lines use each and their total, and every account a
//! transaction can transfer to.

use std::collections::HashMap;

use balanced_core::{AccountId, Category, CategoryId, Generation, LedgerView, Usage};
use balanced_i18n::{AmountStyle, Localizer};
use eframe::egui::accesskit::Role;
use eframe::egui::style::ScrollStyle;
use eframe::egui::{self, Align, Id, Layout, RichText, Sense, vec2};
use egui_extras::{Column, TableBuilder};
use icu_locale::Locale;

use crate::dialog::{DialogOutcome, HeavyDialog, OpenDialog};
use crate::formatted_amount::FormattedAmount;
use crate::i18n::fl;
use crate::register_model::transfer_text;
use crate::theme::Theme;

/// What the Category List holds while it is open.
#[derive(Debug, Default)]
pub(crate) struct CategoryListForm {
    /// The rows of the last snapshot shown, rebuilt when it is out of date.
    model: Option<ListModel>,
}

/// Which row of the Category List a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RowKey {
    Category(CategoryId),
    /// An account, which a transaction names as `[Account]` to transfer to it.
    Transfer(AccountId),
}

/// One row of the Category List, formatted for drawing.
#[derive(Debug)]
struct ListRow {
    key: RowKey,
    /// How many categories it sits under: 0 at the top of the tree.
    depth: usize,
    name: String,
    /// The Txns cell's text.
    count: String,
    /// The Total cell's amount, or `None` when nothing uses the row or the total
    /// overflows.
    total: Option<FormattedAmount>,
    /// The count and total, as a screen reader reads them after the name.
    description: String,
}

/// Every row of the Category List for one snapshot: the categories in tree order, each
/// category's subcategories after it sorted by name, then the accounts in Account bar
/// order.
#[derive(Debug)]
struct ListModel {
    source: Source,
    rows: Vec<ListRow>,
}

/// What a [`ListModel`] was built from: a snapshot, formatted in a locale.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    generation: Generation,
    formatting_locale: Locale,
}

impl Source {
    fn of(view: &LedgerView, i18n: &Localizer) -> Self {
        Self {
            generation: view.generation(),
            formatting_locale: i18n.formatting_locale().clone(),
        }
    }
}

impl ListModel {
    fn new(view: &LedgerView, i18n: &Localizer) -> Self {
        let mut children: HashMap<Option<CategoryId>, Vec<&Category>> = HashMap::new();
        for category in view.categories() {
            children
                .entry(category.parent())
                .or_default()
                .push(category);
        }
        for siblings in children.values_mut() {
            siblings.sort_by(|a, b| i18n.compare_text(a.name(), b.name()));
        }
        let mut rows = Vec::with_capacity(view.categories().len() + view.accounts().len());
        // Depth first, each category before its subcategories.
        let mut pending: Vec<(&Category, usize)> = children
            .get(&None)
            .into_iter()
            .flatten()
            .rev()
            .map(|&category| (category, 0))
            .collect();
        while let Some((category, depth)) = pending.pop() {
            let key = RowKey::Category(category.id());
            let usage = view.category_usage(category.id());
            rows.push(ListRow::new(
                i18n,
                key,
                depth,
                category.name().to_owned(),
                usage,
            ));
            let under = children.get(&Some(category.id())).into_iter().flatten();
            pending.extend(under.rev().map(|&child| (child, depth + 1)));
        }
        for summary in view.accounts() {
            let account = summary.account();
            let usage = view.transfer_usage(account.id());
            let name = transfer_text(account.name());
            rows.push(ListRow::new(
                i18n,
                RowKey::Transfer(account.id()),
                0,
                name,
                usage,
            ));
        }
        Self {
            source: Source::of(view, i18n),
            rows,
        }
    }
}

impl ListRow {
    fn new(i18n: &Localizer, key: RowKey, depth: usize, name: String, usage: &Usage) -> Self {
        let count = usage.count();
        let total = match usage.total() {
            Ok(total) if count > 0 => Some(FormattedAmount::new(i18n, total, AmountStyle::Symbol)),
            Ok(_) => None,
            Err(error) => {
                tracing::warn!(?error, ?key, "a Category List total can't be summed");
                None
            }
        };
        let description = match &total {
            Some(total) => fl!(
                i18n,
                "dialog-category-list-usage",
                count = count,
                total = total.text().to_owned()
            ),
            None => fl!(i18n, "dialog-category-list-usage-count", count = count),
        };
        Self {
            key,
            depth,
            name,
            count: i18n.format_number(i64::try_from(count).unwrap_or(i64::MAX)),
            total,
            description,
        }
    }
}

impl CategoryListForm {
    /// Draws the tree and its usage, filling `ui`.
    fn show(&mut self, ui: &mut egui::Ui, i18n: &Localizer, theme: &Theme, view: &LedgerView) {
        let model = match self.model.take() {
            Some(model) if model.source == Source::of(view, i18n) => model,
            _ => ListModel::new(view, i18n),
        };
        // A solid scroll bar takes its own width beside the columns, so it never covers
        // Total.
        ui.style_mut().spacing.scroll = ScrollStyle::solid();
        let row_height = theme.row_height();
        TableBuilder::new(ui)
            .id_salt("category_list_table")
            .striped(true)
            .resizable(false)
            .auto_shrink(false)
            .max_scroll_height(f32::INFINITY)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::remainder().clip(true))
            .column(Column::exact(Theme::CATEGORY_LIST_COUNT_WIDTH))
            .column(Column::exact(Theme::CATEGORY_LIST_TOTAL_WIDTH))
            .header(row_height, |mut header| {
                header.col(|ui| {
                    ui.strong(fl!(i18n, "dialog-category-list-category"));
                });
                header.col(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.strong(fl!(i18n, "dialog-category-list-txns"));
                    });
                });
                header.col(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.strong(fl!(i18n, "dialog-category-list-total"));
                    });
                });
            })
            .body(|body| {
                body.rows(row_height, model.rows.len(), |mut table_row| {
                    let row = &model.rows[table_row.index()];
                    table_row.col(|ui| {
                        #[expect(
                            clippy::cast_precision_loss,
                            reason = "a tree is never deep enough to lose precision"
                        )]
                        let indent = row.depth as f32 * Theme::CATEGORY_LIST_INDENT;
                        ui.add_space(indent);
                        name_cell(ui, row);
                    });
                    table_row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(&row.count);
                        });
                    });
                    table_row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if let Some(total) = &row.total {
                                ui.label(total.styled(theme));
                            }
                        });
                    });
                });
            });
        self.model = Some(model);
    }
}

/// The Category cell of `row`: its name, as one tree item a screen reader reads with
/// its level and usage.
fn name_cell(ui: &mut egui::Ui, row: &ListRow) {
    let text = match row.key {
        RowKey::Category(_) => RichText::new(&row.name),
        RowKey::Transfer(_) => RichText::new(&row.name).weak(),
    };
    let size = vec2(ui.available_width(), ui.available_height());
    let response = ui.allocate_ui(size, |ui| ui.label(text)).response;
    let id = Id::new(("category_list_row", row.key));
    let response = ui.interact(response.rect, id, Sense::click());
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(Role::TreeItem);
        node.set_label(row.name.clone());
        node.set_description(row.description.clone());
        node.set_level(row.depth + 1);
    });
}

/// What became of the Category List this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CategoryListOutcome {
    Open,
    Closed,
}

fn dialog_id() -> Id {
    Id::new("category_list_dialog")
}

/// Draws the Category List from `view`.
pub(crate) fn show(
    ctx: &egui::Context,
    i18n: &Localizer,
    theme: &Theme,
    dialog: &mut OpenDialog<CategoryListForm>,
    view: &LedgerView,
) -> CategoryListOutcome {
    let outcome = HeavyDialog::closing(
        dialog_id(),
        fl!(i18n, "dialog-category-list-title"),
        fl!(i18n, "dialog-category-list-close"),
        fl!(i18n, "dialog-close"),
        theme,
    )
    .with_size(Theme::CATEGORY_LIST_SIZE)
    .body_fills()
    .show(ctx, dialog, |ui, form| form.show(ui, i18n, theme, view));
    match outcome {
        DialogOutcome::Open => CategoryListOutcome::Open,
        DialogOutcome::Accepted | DialogOutcome::Cancelled => CategoryListOutcome::Closed,
        DialogOutcome::Back | DialogOutcome::Discarded => {
            unreachable!("a heavy Dialog has no Back or Discard button")
        }
    }
}
