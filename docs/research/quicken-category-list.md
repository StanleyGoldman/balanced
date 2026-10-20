# Quicken's Category List window

Research for [#362](https://github.com/StanleyGoldman/balanced/issues/362). It feeds #361, "Prototype: Category List window", which blocks #215, "Category List window".

Researched 2026-10-20 from Quicken's own help and support pages for Quicken for Windows. Where two of those pages disagree, this note says so. Nothing here was checked against a running copy of Quicken.

## Question

How does Quicken's Category List window behave: its columns, operations, what deleting a used category does, how merging works, and what happens to subcategories, transfers and system categories?

## Answer in brief

- **Opened** from Tools → Category List, or Ctrl+Shift+C.
- **Operations:** New Category, Edit, Delete, Merge, Hide/Unhide. Edit, Delete, Merge and Hide are reached by right-clicking a row; New Category and Delete also appear as buttons.
- **Deleting a used category:** one help page says Quicken prompts you to reassign its transactions to another category. Another page on the same operation never mentions a prompt and only advises merging first. The sources do not settle whether leaving the transactions uncategorized is offered.
- **Merge** moves the transactions of the selected categories into one chosen category. Only categories that have transactions can be merged. An optional "Delete unused selected categories" box removes the emptied categories.
- **Hide** is Quicken's non-destructive alternative to Delete: a hidden category leaves the transaction Category field but stays in the file and in reports.
- **Columns** are not described on any page found, so the prototype cannot copy them from these sources.

## Opening and layout

- Tools → Category List, or Ctrl+Shift+C. ([Managing categories with the Category List](https://info.quicken.com/win/how-do-i-open-the-category-list-and-view-my-catego))
- The list is sorted alphabetically by default. Subcategories sit indented under their parent, and transfer accounts appear at the bottom under Transfers and Payments, as `[Account]` names. A **Show** dropdown filters the list and a **Search** field finds a category. ([Working with Categories in Quicken for Windows](https://www.quicken.com/support/working-categories-quicken/))
- Account names in square brackets appear on a Transfers tab; picking an account as a category makes a transfer between the accounts. ([Managing categories](https://info.quicken.com/win/how-do-i-open-the-category-list-and-view-my-catego))
- The usage of a category is checked through a "minireport" icon. ([Merge Category](https://info.quicken.com/win/merge-category))

## Operations

**Add.** New Category opens a dialog with a name, a description and a choice of Income, Expense, or Subcategory of a parent. A Tax Reporting tab holds a "Tax related category" checkbox, the Standard or Extended line item list, and the tax line item. The two pages that describe it disagree on the confirm button's label (OK or Save). ([Working with Categories](https://www.quicken.com/support/working-categories-quicken/), [Managing categories](https://info.quicken.com/win/how-do-i-open-the-category-list-and-view-my-catego))

**Edit.** Right-click → Edit changes the name, the parent, the description and the tax information. Renaming a category that transactions use updates all of them: "Quicken automatically updates the name on all associated transactions." ([Working with Categories](https://www.quicken.com/support/working-categories-quicken/))

**Delete.** Select, Delete, confirm. ([Managing categories](https://info.quicken.com/win/how-do-i-open-the-category-list-and-view-my-catego))
- If the category is used: "Quicken prompts you to reassign those transactions to a different category. This change affects reports and historical data." (same page)
- The other page says only: "might first want to merge it with another category" before deleting, and mentions no reassign prompt. ([Working with Categories](https://www.quicken.com/support/working-categories-quicken/))
- A 2020 community answer says deleting a used category leaves its transactions Uncategorized. This is a forum post, not Quicken documentation, and the search summary calls it older behaviour. ([Quicken Community](https://community.quicken.com/discussion/7883392/is-it-possible-to-delete-or-hide-categories))

**Merge.** Select one or more categories (Ctrl+click or Shift+click), right-click → Merge with another category. ([Merge Category](https://info.quicken.com/win/merge-category))
1. Choose the surviving category in "Merge transactions into".
2. Optionally tick "Delete unused selected categories".
3. OK.
- Restriction: "You can merge only categories that are in use—that is, categories that have transactions associated with them."
- Community answers say a merge can only combine two main categories or two subcategories of the same parent; to merge subcategories under different parents, drag one under the other's parent first. This is not in Quicken's own pages. (summary of [Quicken Community](https://community.quicken.com/discussion/7883392/is-it-possible-to-delete-or-hide-categories))
- Quicken warns the result of a merge cannot be undone and suggests backing up the data file first (summary of the help page found by search; the fetched page itself is silent on undo).

**Hide.** Right-click → Hide/Unhide (one page says "Hide"). ([Hiding a category in Quicken](https://info.quicken.com/win/how-do-i-hide-a-category))
- Hiding a parent hides all its subcategories.
- "Hidden categories don't appear in the Category field when entering a transaction." They stay in the file and in reports where they were used.
- A **Show hidden categories** checkbox at the bottom-left of the window brings them back into the list, where Hide/Unhide reverses it.

## System categories

Names beginning with an underscore are reserved for investment transactions. They are hidden by default and cannot be edited or deleted. ([Managing categories](https://info.quicken.com/win/how-do-i-open-the-category-list-and-view-my-catego)) Quicken's Simplifi product also protects Transfer, Credit Card Payment, Balance Adjustment and Investment from deletion, and deleting a main category there removes its subcategories ([Simplifi help](https://support.simplifi.quicken.com/en/articles/4378958-managing-categories-and-subcategories)); that is a different product and not evidence for Quicken for Windows.

## Gaps

- **Columns.** No page describes the Category List's columns, so what the window shows per row (type, tax flag, usage count, budget) is unknown from these sources.
- **Groups.** No page found covers category groups in the Category List, though the ticket that started this mentions them.
- **Delete with subcategories.** The Windows pages do not say what deleting a parent does to its subcategories or their transactions.
- **Delete: reassign vs uncategorized.** See Delete above; the pages disagree, and a running copy of Quicken would settle it.
- **Undo.** Whether a delete or hide can be undone is not stated.
