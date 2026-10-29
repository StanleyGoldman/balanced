//! PROTOTYPE (#374): the reconcile scenario's fixed-seed stub data, written into a
//! real ledger in a scratch data file ("PROTOTYPE, wipe me"), with its undo history
//! emptied so Undo starts greyed out. Throwaway.

use std::collections::HashMap;
use std::path::Path;

use balanced_core::{
    AccountId, AccountKind, CategoryId, CategoryType, ClearedStatus, CurrencyCode, DomainCommand,
    Ledger, Money, NewTransaction, TransactionEdit, TransactionId,
};
use icu_calendar::{Date, Iso};
use rust_decimal::Decimal;

/// Days since 1970-01-01, as the generator counts them.
pub type Day = i64;

/// Howard Hinnant's `civil_from_days`: (year, month, day).
pub fn civil(days: Day) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn days_from_civil(y: i64, m: i64, d: i64) -> Day {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn iso(day: Day) -> Date<Iso> {
    let (y, m, d) = civil(day);
    Date::try_new_iso(y as i32, m as u8, d as u8).unwrap()
}

pub fn day_of(date: Date<Iso>) -> Day {
    days_from_civil(
        i64::from(date.era_year().year),
        i64::from(date.month().ordinal),
        i64::from(date.day_of_month().0),
    )
}

pub fn usd() -> CurrencyCode {
    CurrencyCode::new("USD").unwrap()
}

pub fn money(cents: i64) -> Money {
    Money::new(Decimal::new(cents, 2), usd()).unwrap()
}

pub fn cents(money: Money) -> i64 {
    use rust_decimal::prelude::ToPrimitive;
    (money.amount() * Decimal::new(100, 0)).to_i64().unwrap()
}

/// The data's today. The App reads the system clock, so the Today line follows the
/// real date; the data stops here.
pub fn today() -> Day {
    days_from_civil(2026, 10, 27)
}

const SEED: u64 = 0x5EED_0374;

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// A typical amount, give or take 40%, in cents.
    fn around(&mut self, typical: i64) -> i64 {
        let spread = typical * 4 / 10;
        typical - spread + (self.below((2 * spread + 1) as u64) as i64)
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }
}

#[derive(Clone)]
struct Txn {
    day: Day,
    num: Option<u32>,
    payee: String,
    cat: String,
    /// Signed in the account's own sense: money in is positive; a card charge is negative.
    cents: i64,
    clr: ClearedStatus,
    /// The amount the paper statement shows for it, when the statement lists it.
    on_paper: Option<i64>,
    /// A transfer to the named account rather than a category.
    transfer: Option<&'static str>,
}

fn txn(day: Day, payee: &str, cat: &str, cents: i64) -> Txn {
    Txn {
        day,
        num: None,
        payee: payee.to_owned(),
        cat: cat.to_owned(),
        cents,
        clr: ClearedStatus::Uncleared,
        on_paper: None,
        transfer: None,
    }
}

/// The paper statement the user holds for an account, shown in the dock.
#[derive(Debug, Clone)]
pub struct Paper {
    pub date: Day,
    /// As printed: positive owed on a card.
    pub ending: i64,
    pub note: String,
    /// The transactions it lists, with the amount it lists each at.
    pub items: Vec<(TransactionId, i64)>,
}

/// What the scenario sets up beside the ledger.
#[derive(Debug, Clone)]
pub struct Scenario {
    pub paper: HashMap<AccountId, Paper>,
    /// Statements in progress at the start: (date, ending as typed).
    pub statements: HashMap<AccountId, (Day, i64)>,
    pub checking: AccountId,
    pub visa: AccountId,
}

/// Builds the ledger into a new data file at `path`, replacing any file there.
pub fn build(path: &Path) -> (Ledger, Scenario) {
    let _ = std::fs::remove_file(path);
    let today = today();
    let start = today - 1_095;
    let ck_last = days_from_civil(2026, 9, 15);
    let ck_stmt = days_from_civil(2026, 10, 15);
    let vs_last = days_from_civil(2026, 9, 5);
    let vs_stmt = days_from_civil(2026, 10, 5);
    let mut rng = Lcg(SEED);

    // Visa first: Checking pays its statements.
    const MERCHANTS: &[(&str, &str, i64)] = &[
        ("Amazon", "Shopping", 3_800),
        ("Whole Foods", "Groceries", 6_400),
        ("Shell", "Auto:Fuel", 4_500),
        ("Starbucks", "Dining", 650),
        ("Target", "Household", 5_200),
        ("Uber", "Travel", 2_300),
        ("Chipotle", "Dining", 1_400),
        ("Home Depot", "Home:Repairs", 7_800),
        ("CVS Pharmacy", "Medical", 2_100),
    ];
    let mut visa = Vec::new();
    let mut payments = Vec::new();
    let mut bal = 0_i64;
    let mut stmt_bal = 0_i64;
    for day in start..=today {
        let (_, _, d) = civil(day);
        if d == 25 && stmt_bal < 0 {
            payments.push((day, -stmt_bal));
            bal += -stmt_bal;
        }
        if d == 3 {
            visa.push(txn(day, "Netflix", "Entertainment", -1_549));
            bal -= 1_549;
        }
        if d == 11 {
            visa.push(txn(day, "Spotify", "Entertainment", -1_199));
            bal -= 1_199;
        }
        for _ in 0..(u64::from(rng.below(100) < 70) + u64::from(rng.below(100) < 25)) {
            let (p, c, typical) = rng.pick(MERCHANTS);
            let cents = -rng.around(typical);
            visa.push(txn(day, p, c, cents));
            bal += cents;
        }
        if d == 5 {
            stmt_bal = bal;
        }
    }

    // Checking.
    const DINING: &[&str] = &["Olive Garden", "Panera Bread", "Thai Basil", "Five Guys"];
    const MISC: &[(&str, &str, i64)] = &[
        ("Walgreens", "Medical", 2_500),
        ("Petco", "Pets", 3_500),
        ("USPS", "Postage", 900),
        ("Barnes & Noble", "Books", 2_800),
        ("Ace Hardware", "Home:Repairs", 3_100),
    ];
    const CHECKS: &[(&str, &str, i64)] = &[
        ("Lincoln Elementary PTA", "Education", 6_000),
        ("Dr. Patel Dental", "Medical:Dental", 15_000),
        ("Green Lawn Care", "Home:Yard", 7_500),
    ];
    let mut ck = Vec::new();
    let mut is_check = Vec::new();
    for day in start..=today {
        let (_, _, d) = civil(day);
        if (day - start) % 14 == 3 {
            ck.push(txn(day, "Acme Corp Payroll", "Salary", 285_000));
        }
        if d == 1 {
            is_check.push(ck.len());
            ck.push(txn(day, "Parkside Apartments", "Housing:Rent", -195_000));
        }
        if d == 12 {
            let c = -rng.around(11_000);
            ck.push(txn(day, "City Power & Light", "Utilities:Electric", c));
        }
        if d == 18 {
            ck.push(txn(day, "Comcast", "Utilities:Internet", -8_999));
        }
        if d == 22 {
            let c = -rng.around(4_500);
            ck.push(txn(day, "City Water Dept", "Utilities:Water", c));
        }
        if (day - start) % 7 == 5 {
            ck.push(txn(day, "ATM Withdrawal", "Cash", -6_000));
        }
        if rng.below(7) < 3 {
            let c = -rng.around(8_500);
            ck.push(txn(day, "Safeway", "Groceries", c));
        }
        if rng.below(7) == 0 {
            let c = -rng.around(4_800);
            ck.push(txn(day, "Chevron", "Auto:Fuel", c));
        }
        if rng.below(7) < 2 {
            let p = rng.pick(DINING);
            let c = -rng.around(4_000);
            ck.push(txn(day, p, "Dining", c));
        }
        if rng.below(10) < 4 {
            let (p, cat, typical) = rng.pick(MISC);
            let c = -rng.around(typical);
            ck.push(txn(day, p, cat, c));
        }
        if rng.below(30) == 0 && !(ck_last..=ck_stmt).contains(&day) {
            let (p, cat, typical) = rng.pick(CHECKS);
            is_check.push(ck.len());
            ck.push(txn(day, p, cat, -rng.around(typical)));
        }
    }
    for (day, p) in payments {
        let mut t = txn(day, "Visa Payment", "", -p);
        t.transfer = Some("Visa");
        ck.push(t);
    }
    // The scenario's own rows: an old check never cashed, the transposed check, a later
    // check, a later debit already marked Cleared, and next month's rent written early.
    let outstanding = ck.len();
    is_check.push(outstanding);
    ck.push(txn(
        days_from_civil(2026, 8, 28),
        "Lincoln Elementary PTA",
        "Education",
        -4_500,
    ));
    let dental = ck.len();
    is_check.push(dental);
    ck.push(txn(
        days_from_civil(2026, 9, 24),
        "Dr. Patel Dental",
        "Medical:Dental",
        -14_500,
    ));
    is_check.push(ck.len());
    ck.push(txn(
        days_from_civil(2026, 10, 10),
        "Green Lawn Care",
        "Home:Yard",
        -7_500,
    ));
    let coffee = ck.len();
    ck.push(txn(
        days_from_civil(2026, 10, 17),
        "Blue Bottle Coffee",
        "Dining",
        -675,
    ));
    is_check.push(ck.len());
    ck.push(txn(
        days_from_civil(2026, 11, 1),
        "Parkside Apartments",
        "Housing:Rent",
        -195_000,
    ));

    let mut order: Vec<usize> = (0..ck.len()).collect();
    order.sort_by_key(|&i| (ck[i].day, i));
    let mut num = 1001;
    for &i in &order {
        if is_check.contains(&i) {
            ck[i].num = Some(num);
            num += 1;
        }
    }
    let late_checks = days_from_civil(2026, 10, 8);
    let late_all = days_from_civil(2026, 10, 13);
    for (i, t) in ck.iter_mut().enumerate() {
        if t.day <= ck_last && i != outstanding {
            t.clr = ClearedStatus::Reconciled;
        } else if t.day <= ck_stmt && i != outstanding {
            // The bank hasn't seen checks written from 10/8, or anything from 10/13.
            let late = (t.num.is_some() && t.day >= late_checks) || t.day >= late_all;
            if !late {
                t.on_paper = Some(if i == dental { -15_400 } else { t.cents });
            }
        }
        if i == coffee {
            t.clr = ClearedStatus::Cleared;
        }
    }
    let dental_num = ck[dental].num.unwrap_or_default();
    let fee = -1_200;
    let opening_ck = 420_000;

    let half = days_from_civil(2026, 9, 20);
    for t in &mut visa {
        if t.day <= vs_last {
            t.clr = ClearedStatus::Reconciled;
        } else if t.day <= vs_stmt - 2 {
            t.on_paper = Some(t.cents);
            // Started before: the first half is already ticked.
            if t.day <= half {
                t.clr = ClearedStatus::Cleared;
            }
        }
    }

    // Write it.
    let mut ledger = Ledger::create(path, usd()).expect("create the scratch data file");
    let opened = iso(start);
    let mut accounts = vec![
        ("Checking", AccountKind::Checking, opening_ck),
        ("Visa", AccountKind::CreditCard, 0),
        ("Savings", AccountKind::Savings, 1_250_000),
        ("Brokerage", AccountKind::Investment, 2_531_055),
        ("Mortgage", AccountKind::Liability, -31_050_000),
    ];
    let mut cmds = Vec::new();
    for (name, kind, opening) in accounts.drain(..) {
        cmds.push(DomainCommand::CreateAccount {
            name: name.to_owned(),
            kind,
            opened,
            opening_balance: money(opening),
        });
    }
    let mut cats: Vec<String> = ck
        .iter()
        .chain(&visa)
        .map(|t| t.cat.clone())
        .chain(
            [
                "Bank Fees",
                "Interest Income",
                "Miscellaneous",
                "Uncategorized",
            ]
            .map(String::from),
        )
        .filter(|c| !c.is_empty())
        .collect();
    cats.sort();
    cats.dedup();
    let mut parents: Vec<String> = cats
        .iter()
        .filter_map(|c| c.split_once(':').map(|(p, _)| p.to_owned()))
        .collect();
    parents.sort();
    parents.dedup();
    for p in &parents {
        cmds.push(DomainCommand::CreateCategory {
            name: p.clone(),
            parent: None,
            category_type: CategoryType::Expense,
        });
    }
    for c in cats
        .iter()
        .filter(|c| !c.contains(':') && !parents.contains(c))
    {
        let income = matches!(c.as_str(), "Salary" | "Interest Income");
        cmds.push(DomainCommand::CreateCategory {
            name: c.clone(),
            parent: None,
            category_type: if income {
                CategoryType::Income
            } else {
                CategoryType::Expense
            },
        });
    }
    ledger
        .apply(DomainCommand::Compound(cmds))
        .expect("accounts and top-level categories");
    let view = ledger.snapshot();
    let account = |name: &str| {
        view.accounts()
            .iter()
            .find(|a| a.account().name() == name)
            .unwrap()
            .account()
            .id()
    };
    let (ck_id, vs_id) = (account("Checking"), account("Visa"));
    let top = |name: &str| -> CategoryId {
        view.categories()
            .iter()
            .find(|c| c.name() == name && c.parent().is_none())
            .unwrap_or_else(|| panic!("category {name}"))
            .id()
    };
    let mut cmds = Vec::new();
    for c in cats.iter().filter(|c| c.contains(':')) {
        let (p, n) = c.split_once(':').unwrap();
        cmds.push(DomainCommand::CreateCategory {
            name: n.to_owned(),
            parent: Some(top(p)),
            category_type: CategoryType::Expense,
        });
    }
    ledger
        .apply(DomainCommand::Compound(cmds))
        .expect("subcategories");
    let view = ledger.snapshot();
    let cat_id = |path: &str| -> CategoryId {
        view.categories()
            .iter()
            .find(|c| view.category_path(c.id()).unwrap().join(":") == path)
            .unwrap_or_else(|| panic!("category {path}"))
            .id()
    };
    let cat_ids: HashMap<String, CategoryId> =
        cats.iter().map(|c| (c.clone(), cat_id(c))).collect();

    let post = |account: AccountId, t: &Txn| {
        let mut new = match t.transfer {
            Some(_) => NewTransaction::transfer(iso(t.day), &t.payee, vs_id, money(t.cents)),
            None => NewTransaction::new(iso(t.day), &t.payee, cat_ids[&t.cat], money(t.cents)),
        };
        if let Some(n) = t.num {
            new = new.number(&n.to_string());
        }
        DomainCommand::PostTransaction {
            account,
            transaction: new,
        }
    };
    let mut cmds: Vec<DomainCommand> = Vec::new();
    for &i in &order {
        cmds.push(post(ck_id, &ck[i]));
    }
    let mut vs_order: Vec<usize> = (0..visa.len()).collect();
    vs_order.sort_by_key(|&i| (visa[i].day, i));
    for &i in &vs_order {
        cmds.push(post(vs_id, &visa[i]));
    }
    ledger
        .apply(DomainCommand::Compound(cmds))
        .expect("the transactions");

    // Match the posted transactions back to the generated ones, to set each one's
    // Cleared status and find the statement's items. The Visa side of each payment
    // is generated here, Cleared like the charges around it.
    let view = ledger.snapshot();
    let mut edits = Vec::new();
    let mut paper_items: HashMap<AccountId, Vec<(TransactionId, i64)>> = HashMap::new();
    for (account, list) in [(ck_id, &ck), (vs_id, &visa)] {
        let mut queue: HashMap<(Day, i64, String), Vec<&Txn>> = HashMap::new();
        for t in list.iter().rev() {
            queue
                .entry((t.day, t.cents, t.payee.clone()))
                .or_default()
                .push(t);
        }
        let summary = view.account(account).unwrap();
        for entry in summary.register().iter() {
            let tx = entry.transaction();
            let key = (day_of(tx.date()), cents(tx.amount()), tx.payee().to_owned());
            let generated = queue.get_mut(&key).and_then(Vec::pop);
            let (clr, paper) = match generated {
                Some(t) => (t.clr, t.on_paper),
                // The Visa side of a payment from Checking.
                None if account == vs_id && tx.payee() == "Visa Payment" => {
                    let day = key.0;
                    if day <= vs_last {
                        (ClearedStatus::Reconciled, None)
                    } else if day <= vs_stmt - 2 {
                        let clr = if day <= half {
                            ClearedStatus::Cleared
                        } else {
                            ClearedStatus::Uncleared
                        };
                        (clr, Some(key.1))
                    } else {
                        (ClearedStatus::Uncleared, None)
                    }
                }
                // The opening balance.
                None => (
                    if key.0 <= vs_last.min(ck_last) {
                        ClearedStatus::Reconciled
                    } else {
                        ClearedStatus::Uncleared
                    },
                    None,
                ),
            };
            if clr != ClearedStatus::Uncleared {
                edits.push(DomainCommand::EditTransaction {
                    transaction: tx.id(),
                    edit: TransactionEdit::ClearedStatus(clr),
                });
            }
            if let Some(p) = paper {
                paper_items.entry(account).or_default().push((tx.id(), p));
            }
        }
    }
    ledger
        .apply(DomainCommand::Compound(edits))
        .expect("the cleared statuses");
    let view = ledger.snapshot();

    let reconciled = |account: AccountId| -> i64 {
        view.account(account)
            .unwrap()
            .register()
            .iter()
            .filter(|e| e.transaction().cleared_status() == ClearedStatus::Reconciled)
            .map(|e| cents(e.transaction().amount()))
            .sum()
    };
    let items_sum = |account: AccountId| -> i64 {
        paper_items
            .get(&account)
            .map_or(0, |v| v.iter().map(|(_, c)| c).sum())
    };
    let ck_paper = reconciled(ck_id) + items_sum(ck_id) + fee;
    let vs_paper = -(reconciled(vs_id) + items_sum(vs_id));
    let mut paper = HashMap::new();
    paper.insert(
        ck_id,
        Paper {
            date: ck_stmt,
            ending: ck_paper,
            note: format!(
                "It lists a 12.00 monthly service fee never entered in Balanced, and check {dental_num} to Dr. Patel Dental at 154.00 (entered as 145.00). Checks written from 10/8 and anything from 10/13 aren't on it."
            ),
            items: paper_items.remove(&ck_id).unwrap_or_default(),
        },
    );
    paper.insert(
        vs_id,
        Paper {
            date: vs_stmt,
            ending: vs_paper,
            note: "Matches Balanced once every charge and payment it lists is ticked. Charges from 10/4 and 10/5 post on the next statement.".to_owned(),
            items: paper_items.remove(&vs_id).unwrap_or_default(),
        },
    );
    let mut statements = HashMap::new();
    statements.insert(vs_id, (vs_stmt, vs_paper));
    drop(view);
    drop(ledger);

    // Empty the undo history, so the user starts with nothing to undo.
    let store = balanced_store::Store::open(path).expect("reopen the scratch file");
    store
        .clear_stack(balanced_store::Stack::Undo)
        .expect("clear the undo stack");
    drop(store);
    let ledger = Ledger::open(path).expect("open the scratch file");
    (
        ledger,
        Scenario {
            paper,
            statements,
            checking: ck_id,
            visa: vs_id,
        },
    )
}
