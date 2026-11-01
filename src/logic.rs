use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

pub const CURRENCIES: [&str; 3] = ["RUB", "USD", "EUR"];
pub const PERIODS: [&str; 4] = ["weekly", "monthly", "quarterly", "yearly"];
pub const CATEGORIES: [&str; 7] = [
    "Видео", "Музыка", "Софт", "Облако", "Игры", "Связь", "Другое",
];

const MONTHS: [&str; 12] = [
    "января",
    "февраля",
    "марта",
    "апреля",
    "мая",
    "июня",
    "июля",
    "августа",
    "сентября",
    "октября",
    "ноября",
    "декабря",
];

const DUE_WINDOW_DAYS: i64 = 30;
const TIMELINE_LIMIT: usize = 8;
const BALANCE_HORIZON_DAYS: i64 = 365 * 8;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ListFilter {
    All,
    Active,
    Paused,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Date,
    Amount,
    Name,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub amount: f64,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default = "default_period")]
    pub period: String,
    pub next_charge: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub note: String,
    #[serde(default = "default_true")]
    pub active: bool,
    /// `card` — списание с карты. `balance` — со своего остатка на этом сервисе.
    #[serde(default = "default_source")]
    pub source: String,
    /// Остаток на этом сервисе, в его валюте. Учитывается только при `source == balance`.
    #[serde(default)]
    pub balance: f64,
    #[serde(default)]
    pub created_at: String,
    /// Отрицательное значение — цвет значка берётся из названия. Иначе свой цвет, отдельно от категории.
    #[serde(default = "unset_channel")]
    pub icon_r: i16,
    #[serde(default = "unset_channel")]
    pub icon_g: i16,
    #[serde(default = "unset_channel")]
    pub icon_b: i16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_true")]
    pub dark: bool,
    #[serde(default = "default_currency")]
    pub base: String,
    #[serde(default = "default_usd")]
    pub rub_per_usd: f64,
    #[serde(default = "default_eur")]
    pub rub_per_eur: f64,
    #[serde(default)]
    pub custom_categories: Vec<NamedColor>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dark: true,
            base: default_currency(),
            rub_per_usd: default_usd(),
            rub_per_eur: default_eur(),
            custom_categories: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Rates {
    pub rub_per_usd: f64,
    pub rub_per_eur: f64,
}

impl Settings {
    pub fn rates(&self) -> Rates {
        Rates {
            rub_per_usd: self.rub_per_usd,
            rub_per_eur: self.rub_per_eur,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedColor {
    pub name: String,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub struct Loaded<T> {
    pub value: T,
    pub warning: String,
}

pub struct FormDraft {
    pub name: String,
    pub amount: String,
    pub currency_index: i32,
    pub period_index: i32,
    pub category_index: i32,
    pub date: String,
    pub note: String,
    pub active: bool,
    pub source_index: i32,
    pub balance: String,
    pub icon_index: i32,
}

pub struct FormCheck {
    pub name_error: String,
    pub amount_error: String,
    pub date_error: String,
    pub note_error: String,
    pub balance_error: String,
    pub valid: bool,
}

pub struct ViewQuery {
    pub text: String,
    pub filter: ListFilter,
    pub sort: SortMode,
    pub category: Option<String>,
    pub horizon_days: i64,
}

pub struct RowView {
    pub id: String,
    pub name: String,
    pub detail: String,
    pub when: String,
    pub date: String,
    pub price: String,
    pub letter: String,
    pub pause_label: String,
    pub balance: String,
    pub balance_short: bool,
    pub badge: String,
    pub can_topup: bool,
    pub rgb: (u8, u8, u8),
    pub category_rgb: (u8, u8, u8),
    pub paused: bool,
}

#[allow(dead_code)]
pub struct TimelineItem {
    pub header: bool,
    pub label: String,
    pub price: String,
}

pub struct BalanceLine {
    pub id: String,
    pub name: String,
    pub amount: String,
    pub until: String,
    pub detail: String,
    pub short: bool,
}

pub struct DayCell {
    pub label: String,
    pub marked: bool,
    pub today: bool,
    pub dim: bool,
}

pub struct CategoryShare {
    pub name: String,
    pub amount: String,
    pub share: f32,
    pub percent: String,
    pub rgb: (u8, u8, u8),
}

pub struct Dashboard {
    pub monthly: String,
    pub yearly: String,
    #[allow(dead_code)]
    pub due: String,
    #[allow(dead_code)]
    pub month_left: String,
    pub horizon_total: String,
    pub active_count: String,
    pub rows: Vec<RowView>,
    #[allow(dead_code)]
    pub timeline: Vec<TimelineItem>,
    pub categories: Vec<CategoryShare>,
    pub month_title: String,
    pub month_weeks: Vec<Vec<DayCell>>,
    pub empty: bool,
    pub no_results: bool,
    pub hint: String,
    pub balances: Vec<BalanceLine>,
}

fn default_true() -> bool {
    true
}

fn unset_channel() -> i16 {
    -1
}

pub const ICON_CHOICES: [(u8, u8, u8); 8] = [
    (61, 126, 255),
    (62, 207, 142),
    (255, 93, 99),
    (224, 138, 60),
    (168, 120, 255),
    (58, 160, 168),
    (212, 83, 126),
    (196, 163, 90),
];

fn default_currency() -> String {
    "RUB".into()
}

fn default_period() -> String {
    "monthly".into()
}

fn default_category() -> String {
    "Другое".into()
}

fn default_usd() -> f64 {
    90.0
}

fn default_eur() -> f64 {
    98.0
}

fn default_source() -> String {
    "card".into()
}

pub fn app_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TrackPurchase")
}

pub fn subscriptions_path() -> PathBuf {
    app_dir().join("subscriptions.json")
}

pub fn settings_path() -> PathBuf {
    app_dir().join("settings.json")
}

pub fn export_json_path() -> PathBuf {
    app_dir().join("export.json")
}

pub fn export_csv_path() -> PathBuf {
    app_dir().join("export.csv")
}

pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

pub fn parse_date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").ok()
}

pub fn parse_user_date(text: &str) -> Option<NaiveDate> {
    let text = text.trim();
    NaiveDate::parse_from_str(text, "%d.%m.%Y")
        .ok()
        .or_else(|| parse_date(text))
}

pub fn format_input_date(date: NaiveDate) -> String {
    format!("{:02}.{:02}.{}", date.day(), date.month(), date.year())
}

pub fn index_of(options: &[&str], value: &str, fallback: i32) -> i32 {
    options
        .iter()
        .position(|item| *item == value)
        .map(|index| index as i32)
        .unwrap_or(fallback)
}

pub fn amount_field(amount: f64) -> String {
    let cents = (amount * 100.0).round() as i64;
    let whole = cents / 100;
    let fraction = (cents % 100).unsigned_abs();
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole},{fraction:02}")
    }
}

pub fn category_names(settings: &Settings) -> Vec<String> {
    let mut names: Vec<String> = CATEGORIES.iter().map(|name| (*name).to_string()).collect();
    names.extend(settings.custom_categories.iter().map(|item| item.name.clone()));
    names
}

pub fn category_rgb(name: &str, settings: &Settings) -> (u8, u8, u8) {
    if let Some(custom) = settings.custom_categories.iter().find(|item| item.name == name) {
        return (custom.r, custom.g, custom.b);
    }
    match name {
        "Видео" => (124, 92, 191),
        "Музыка" => (212, 83, 126),
        "Софт" => (61, 139, 253),
        "Облако" => (58, 160, 168),
        "Игры" => (224, 138, 60),
        "Связь" => (196, 163, 90),
        _ => (139, 147, 161),
    }
}

pub fn default_category_index(settings: &Settings) -> i32 {
    category_names(settings)
        .iter()
        .position(|name| name == "Другое")
        .unwrap_or(0) as i32
}

/// Добавляет категорию и запоминает случайный цвет. Повтор того же имени не создаёт вторую.
pub fn add_custom_category(settings: &mut Settings, raw: &str) -> Result<usize, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("Введите название категории".into());
    }
    if name.chars().count() > 24 {
        return Err("Не длиннее 24 символов".into());
    }
    let names = category_names(settings);
    if names.iter().any(|item| same_name(item, name)) {
        return Err("Такая категория уже есть".into());
    }
    let used = names.iter().map(|item| category_rgb(item, settings)).collect::<Vec<_>>();
    let (r, g, b) = random_swatch(&used);
    settings.custom_categories.push(NamedColor { name: name.to_string(), r, g, b });
    Ok(category_names(settings).len() - 1)
}

pub fn remember_category(settings: &mut Settings, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() || category_names(settings).iter().any(|item| same_name(item, name)) {
        return false;
    }
    add_custom_category(settings, name).is_ok()
}

pub fn initial(name: &str) -> String {
    let Some(ch) = name.trim().chars().next() else {
        return "•".into();
    };
    match ch.to_uppercase().next() {
        Some(upper) => upper.to_string(),
        None => "•".into(),
    }
}

pub fn load_subscriptions(path: &Path) -> Loaded<Vec<Subscription>> {
    if !path.exists() {
        return Loaded { value: Vec::new(), warning: String::new() };
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            return Loaded {
                value: Vec::new(),
                warning: format!("Не удалось прочитать подписки: {error}"),
            };
        }
    };
    match serde_json::from_str::<Vec<Subscription>>(&text) {
        Ok(mut items) => {
            for item in &mut items {
                item.balance = finite_balance(item.balance);
                if item.source != "balance" {
                    item.source = default_source();
                }
            }
            Loaded { value: items, warning: String::new() }
        }
        Err(error) => Loaded {
            value: Vec::new(),
            warning: backup_and_warn(path, &format!("Файл подписок повреждён ({error})")),
        },
    }
}

pub fn load_settings(path: &Path) -> Loaded<Settings> {
    if !path.exists() {
        return Loaded { value: Settings::default(), warning: String::new() };
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            return Loaded {
                value: Settings::default(),
                warning: format!("Не удалось прочитать настройки: {error}"),
            };
        }
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(settings) => Loaded { value: sanitize_settings(settings), warning: String::new() },
        Err(error) => Loaded {
            value: Settings::default(),
            warning: backup_and_warn(path, &format!("Файл настроек повреждён ({error})")),
        },
    }
}

pub fn save_subscriptions(path: &Path, items: &[Subscription]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(items).map_err(|error| error.to_string())?;
    atomic_write(path, &format!("{json}\n"))
}

pub fn save_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    let json = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    atomic_write(path, &format!("{json}\n"))
}

pub fn export_json(path: &Path, items: &[Subscription]) -> Result<(), String> {
    save_subscriptions(path, items)
}

pub fn export_csv(path: &Path, items: &[Subscription]) -> Result<(), String> {
    let mut text = String::from("id,название,сумма,валюта,период,дата,категория,заметка,статус,создана,источник,баланс\n");
    for item in items {
        let amount_text = amount_field(item.amount);
        let balance_text = amount_field(item.balance);
        let cells = [
            item.id.as_str(),
            item.name.as_str(),
            amount_text.as_str(),
            item.currency.as_str(),
            item.period.as_str(),
            item.next_charge.as_str(),
            item.category.as_str(),
            item.note.as_str(),
            if item.active { "активна" } else { "пауза" },
            item.created_at.as_str(),
            if item.source == "balance" { "баланс" } else { "карта" },
            balance_text.as_str(),
        ];
        text.push_str(&cells.map(csv_cell).join(","));
        text.push('\n');
    }
    atomic_write(path, &text)
}

pub fn import_subscriptions(path: &Path) -> Result<Vec<Subscription>, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("Не удалось прочитать файл: {error}"))?;
    let trimmed = text.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err("Файл пуст".into());
    }
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        let items = serde_json::from_str::<Vec<Subscription>>(trimmed)
            .map_err(|error| format!("Это не список подписок в JSON: {error}"))?;
        if items.is_empty() {
            return Err("В файле нет подписок".into());
        }
        return Ok(items);
    }
    parse_csv(trimmed)
}

pub fn merge_imported(existing: &mut Vec<Subscription>, incoming: Vec<Subscription>) -> usize {
    let mut count = 0;
    for mut item in incoming {
        if item.name.trim().is_empty() || !item.amount.is_finite() || item.amount <= 0.0 {
            continue;
        }
        item.name = item.name.trim().to_string();
        item.currency = normalize_currency(&item.currency);
        item.period = normalize_period(&item.period);
        item.category = normalize_category(&item.category);
        item.note = item.note.trim().to_string();
        if let Some(slot) = existing.iter_mut().find(|current| !item.id.is_empty() && current.id == item.id) {
            *slot = item;
            count += 1;
            continue;
        }
        item.id = new_id();
        if item.created_at.is_empty() {
            item.created_at = chrono::Local::now().to_rfc3339();
        }
        existing.push(item);
        count += 1;
    }
    count
}

pub fn validate(draft: &FormDraft, show_required: bool) -> FormCheck {
    let name = draft.name.trim();
    let name_error = if name.is_empty() {
        if show_required { "Введите название" } else { "" }
    } else if name.chars().count() > 80 {
        "Не длиннее 80 символов"
    } else {
        ""
    };

    let amount_text = draft.amount.trim();
    let amount_error = if amount_text.is_empty() {
        if show_required { "Введите сумму" } else { "" }
    } else {
        match parse_positive(amount_text) {
            Err(message) => message,
            Ok(value) if value > 1_000_000_000.0 => "Слишком большая сумма",
            Ok(_) => "",
        }
    };

    let date_text = draft.date.trim();
    let date_error = if date_text.is_empty() {
        if show_required { "Укажите дату" } else { "" }
    } else if parse_user_date(date_text).is_none() {
        "Формат ДД.ММ.ГГГГ, например 31.10.2026"
    } else {
        ""
    };

    let note_error = if draft.note.trim().chars().count() > 200 {
        "Не длиннее 200 символов"
    } else {
        ""
    };

    let from_balance = draft.source_index == 1;
    let balance_text = draft.balance.trim();
    let balance_error = if !from_balance {
        String::new()
    } else if balance_text.is_empty() {
        if show_required { "Укажите остаток на сервисе".into() } else { String::new() }
    } else {
        match parse_balance(balance_text) {
            Ok(_) => String::new(),
            Err(message) => message,
        }
    };

    let valid = name_error.is_empty()
        && amount_error.is_empty()
        && date_error.is_empty()
        && note_error.is_empty()
        && balance_error.is_empty()
        && !name.is_empty()
        && !amount_text.is_empty()
        && !date_text.is_empty()
        && (!from_balance || !balance_text.is_empty());

    FormCheck {
        name_error: name_error.into(),
        amount_error: amount_error.into(),
        date_error: date_error.into(),
        note_error: note_error.into(),
        balance_error,
        valid,
    }
}

pub fn subscription_from_form(
    draft: FormDraft,
    existing: Option<&Subscription>,
    categories: &[String],
) -> Result<Subscription, String> {
    let check = validate(&draft, true);
    if !check.valid {
        let message = [&check.name_error, &check.amount_error, &check.date_error, &check.note_error, &check.balance_error]
            .into_iter()
            .find(|item| !item.is_empty())
            .map(|item| (*item).clone())
            .unwrap_or_else(|| "Проверьте поля".into());
        return Err(message);
    }
    let amount = parse_positive(draft.amount.trim()).map_err(str::to_string)?;
    let date = parse_user_date(draft.date.trim()).ok_or("Формат ДД.ММ.ГГГГ, например 31.10.2026")?;
    let from_balance = draft.source_index == 1;
    let balance = if from_balance {
        parse_balance(draft.balance.trim())?
    } else {
        parse_balance(draft.balance.trim()).unwrap_or(0.0)
    };
    Ok(Subscription {
        id: existing.map(|item| item.id.clone()).unwrap_or_else(new_id),
        name: draft.name.trim().to_string(),
        amount,
        currency: pick(&CURRENCIES, draft.currency_index, "RUB").to_string(),
        period: pick(&PERIODS, draft.period_index, "monthly").to_string(),
        next_charge: date.format("%Y-%m-%d").to_string(),
        category: usize::try_from(draft.category_index)
            .ok()
            .and_then(|index| categories.get(index))
            .cloned()
            .unwrap_or_else(|| "Другое".into()),
        note: draft.note.trim().to_string(),
        active: draft.active,
        source: if from_balance { "balance" } else { "card" }.to_string(),
        balance,
        created_at: existing
            .map(|item| item.created_at.clone())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| chrono::Local::now().to_rfc3339()),
        icon_r: icon_channels(draft.icon_index).0,
        icon_g: icon_channels(draft.icon_index).1,
        icon_b: icon_channels(draft.icon_index).2,
    })
}

fn icon_channels(index: i32) -> (i16, i16, i16) {
    usize::try_from(index - 1)
        .ok()
        .and_then(|index| ICON_CHOICES.get(index))
        .map(|color| (i16::from(color.0), i16::from(color.1), i16::from(color.2)))
        .unwrap_or((-1, -1, -1))
}

pub fn icon_index(item: &Subscription) -> i32 {
    ICON_CHOICES
        .iter()
        .position(|color| i16::from(color.0) == item.icon_r && i16::from(color.1) == item.icon_g && i16::from(color.2) == item.icon_b)
        .map(|index| index as i32 + 1)
        .unwrap_or(0)
}

pub fn icon_rgb(item: &Subscription) -> (u8, u8, u8) {
    if item.icon_r >= 0 && item.icon_g >= 0 && item.icon_b >= 0 {
        return (item.icon_r as u8, item.icon_g as u8, item.icon_b as u8);
    }
    let mut hash: u32 = 2_166_136_261;
    for byte in item.name.trim().to_lowercase().bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hsl_to_rgb((hash % 360) as f32, 0.58, 0.56)
}

pub fn parse_rate(raw: &str) -> Result<f64, String> {
    match parse_positive(raw.trim()) {
        Ok(value) if value <= 1_000_000.0 => Ok(value),
        Ok(_) => Err("Слишком большой курс".into()),
        Err(_) => Err("Курс должен быть больше нуля".into()),
    }
}

/// Сдвигает прошедшие даты списания на целое число периодов, пока дата не станет сегодня или позже.
pub fn catch_up(items: &mut [Subscription], today: NaiveDate) -> bool {
    let mut changed = false;
    for item in items.iter_mut() {
        let Some(date) = parse_date(&item.next_charge) else {
            continue;
        };
        let next = advance_until(date, &item.period, today);
        let formatted = next.format("%Y-%m-%d").to_string();
        if formatted != item.next_charge {
            item.next_charge = formatted;
            changed = true;
        }
    }
    changed
}

pub fn to_base(amount: f64, currency: &str, base: &str, rates: &Rates) -> f64 {
    let rub = match currency {
        "USD" => amount * rates.rub_per_usd,
        "EUR" => amount * rates.rub_per_eur,
        _ => amount,
    };
    match base {
        "USD" if rates.rub_per_usd > 0.0 => rub / rates.rub_per_usd,
        "EUR" if rates.rub_per_eur > 0.0 => rub / rates.rub_per_eur,
        _ => rub,
    }
}

pub fn compose(items: &[Subscription], view: &ViewQuery, today: NaiveDate, settings: &Settings) -> Dashboard {
    let rates = settings.rates();
    let base = settings.base.as_str();
    let active_n = items.iter().filter(|item| item.active).count();
    let mut monthly = 0.0;
    let mut due = 0.0;
    let mut by_category: BTreeMap<String, f64> = BTreeMap::new();
    let mut events = Vec::new();

    for item in items.iter().filter(|item| item.active) {
        let monthly_amount = to_base(item.amount * monthly_factor(&item.period), &item.currency, base, &rates);
        monthly += monthly_amount;
        *by_category.entry(item.category.clone()).or_insert(0.0) += monthly_amount;
        for date in charge_dates(&item.next_charge, &item.period, today) {
            due += to_base(item.amount, &item.currency, base, &rates);
            events.push((date, item.name.clone(), format_money(item.amount, &item.currency)));
        }
    }

    events.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    events.truncate(TIMELINE_LIMIT);
    let timeline = group_timeline(events, today);

    let mut categories: Vec<CategoryShare> = by_category
        .into_iter()
        .filter(|(_, amount)| *amount > 0.0)
        .map(|(name, amount)| CategoryShare {
            share: if monthly > 0.0 { (amount / monthly) as f32 } else { 0.0 },
            percent: format!("{}%", if monthly > 0.0 { (amount / monthly * 100.0).round() as i32 } else { 0 }),
            rgb: category_rgb(&name, settings),
            amount: format_money(amount, base),
            name,
        })
        .collect();
    categories.sort_by(|left, right| right.share.total_cmp(&left.share).then(left.name.cmp(&right.name)));

    let mut visible: Vec<&Subscription> = items
        .iter()
        .filter(|item| matches_filter(item, view.filter) && matches_query(item, &view.text) && matches_category(item, view.category.as_deref()))
        .collect();
    visible.sort_by(|left, right| compare_items(left, right, view.sort, settings));

    let balances = balance_lines(items, today).into_iter().filter(|line| line.short).collect::<Vec<_>>();
    let month_weeks = month_weeks(items, today);
    let rows = visible.iter().map(|item| row_view(item, today, settings)).collect::<Vec<_>>();
    let empty = items.is_empty();
    let no_results = !empty && rows.is_empty() && !view.text.trim().is_empty();
    let hint = if !rows.is_empty() {
        String::new()
    } else if empty {
        "Укажите название, сумму и дату следующего списания.".into()
    } else if no_results {
        "Попробуйте другое название или снимите фильтр.".into()
    } else if view.filter == ListFilter::Paused {
        "Нет подписок на паузе.".into()
    } else if view.filter == ListFilter::Active {
        "Нет активных подписок.".into()
    } else {
        "Список пуст.".into()
    };

    Dashboard {
        monthly: format_money(monthly, base),
        yearly: format_money(monthly * 12.0, base),
        due: format_money(due, base),
        month_left: format_money(month_remaining(items, today, settings), base),
        horizon_total: format_money(window_total(items, today, settings, view.horizon_days), base),
        active_count: active_n.to_string(),
        rows,
        timeline,
        categories,
        month_title: month_title(today),
        month_weeks,
        empty,
        no_results,
        hint,
        balances,
    }
}

/// Остаток сервиса тратится только его списаниями, в его валюте.
fn service_forecast(item: &Subscription, today: NaiveDate) -> ServiceForecast {
    let currency = item.currency.as_str();
    let balance = money_cents(item.balance).max(0);
    let charge = money_cents(item.amount).max(0);
    if !item.active {
        return ServiceForecast {
            until: "На паузе".into(),
            detail: format!("Осталось {}", format_money(cents_to_amount(balance), currency)),
            sort_on: NaiveDate::MAX,
        };
    }
    let Some(start) = parse_date(&item.next_charge) else {
        return ServiceForecast {
            until: "Проверьте дату".into(),
            detail: String::new(),
            sort_on: NaiveDate::MAX,
        };
    };
    if charge <= 0 {
        return ServiceForecast {
            until: "Нет суммы списания".into(),
            detail: String::new(),
            sort_on: NaiveDate::MAX,
        };
    }
    let mut cursor = advance_until(start, &item.period, today);
    if balance == 0 {
        return ServiceForecast {
            until: "Баланс пуст".into(),
            detail: format!(
                "Ближайшее списание {} · {}",
                format_day(cursor, today),
                format_money(cents_to_amount(charge), currency)
            ),
            sort_on: cursor,
        };
    }

    let horizon = today + chrono::Duration::days(BALANCE_HORIZON_DAYS);
    let mut left = balance;
    let mut covered: Option<NaiveDate> = None;
    for _ in 0..500 {
        if cursor > horizon {
            return ServiceForecast {
                until: "Хватит дольше 8 лет".into(),
                detail: format!("Останется {}", format_money(cents_to_amount(left), currency)),
                sort_on: NaiveDate::MAX,
            };
        }
        if left < charge {
            let until = match covered {
                Some(last) => format!("Хватит до {}", format_day(last, today)),
                None => "Не хватает на ближайшее списание".into(),
            };
            return ServiceForecast {
                until,
                detail: format!(
                    "{} не хватит {}",
                    format_day(cursor, today),
                    format_money(cents_to_amount(charge - left), currency)
                ),
                sort_on: covered.unwrap_or(cursor),
            };
        }
        left -= charge;
        covered = Some(cursor);
        if left == 0 {
            return ServiceForecast {
                until: format!("Закончится {}", format_day(cursor, today)),
                detail: when_label(cursor, today),
                sort_on: cursor,
            };
        }
        let next = add_period(cursor, &item.period);
        if next <= cursor {
            break;
        }
        cursor = next;
    }
    ServiceForecast {
        until: "Хватит дольше 8 лет".into(),
        detail: format!("Останется {}", format_money(cents_to_amount(left), currency)),
        sort_on: NaiveDate::MAX,
    }
}

struct ServiceForecast {
    until: String,
    detail: String,
    sort_on: NaiveDate,
}

fn balance_lines(items: &[Subscription], today: NaiveDate) -> Vec<BalanceLine> {
    let mut lines: Vec<(NaiveDate, BalanceLine)> = items
        .iter()
        .filter(|item| item.source == "balance")
        .map(|item| {
            let forecast = service_forecast(item, today);
            let line = BalanceLine {
                id: item.id.clone(),
                name: item.name.clone(),
                amount: format_money(finite_balance(item.balance), &item.currency),
                until: forecast.until,
                detail: forecast.detail,
                short: item.active && shortage_cents(item) > 0,
            };
            (forecast.sort_on, line)
        })
        .collect();
    lines.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.name.cmp(&right.1.name)));
    lines.into_iter().map(|(_, line)| line).take(8).collect()
}

fn shortage_cents(item: &Subscription) -> i64 {
    let charge = money_cents(item.amount).max(0);
    let balance = money_cents(finite_balance(item.balance)).max(0);
    (charge - balance).max(0)
}

fn money_cents(amount: f64) -> i64 {
    if !amount.is_finite() {
        return 0;
    }
    (amount * 100.0).round() as i64
}

fn cents_to_amount(cents: i64) -> f64 {
    cents as f64 / 100.0
}

fn month_remaining(items: &[Subscription], today: NaiveDate, settings: &Settings) -> f64 {
    let last = NaiveDate::from_ymd_opt(today.year(), today.month(), last_day_of_month(today.year(), today.month())).unwrap_or(today);
    let rates = settings.rates();
    items
        .iter()
        .filter(|item| item.active)
        .map(|item| {
            charges_through(&item.next_charge, &item.period, today, last).len() as f64
                * to_base(item.amount, &item.currency, &settings.base, &rates)
        })
        .sum()
}

fn window_total(items: &[Subscription], today: NaiveDate, settings: &Settings, days: i64) -> f64 {
    let days = match days {
        7 => 7,
        360 => 360,
        _ => DUE_WINDOW_DAYS,
    };
    let rates = settings.rates();
    items
        .iter()
        .filter(|item| item.active)
        .map(|item| {
            charge_dates_for(&item.next_charge, &item.period, today, days).len() as f64
                * to_base(item.amount, &item.currency, &settings.base, &rates)
        })
        .sum()
}

fn charges_through(next_charge: &str, period: &str, today: NaiveDate, last: NaiveDate) -> Vec<NaiveDate> {
    let Some(start) = parse_date(next_charge) else {
        return Vec::new();
    };
    let mut cursor = advance_until(start, period, today);
    let mut dates = Vec::new();
    while cursor <= last && dates.len() < 80 {
        dates.push(cursor);
        let next = add_period(cursor, period);
        if next <= cursor {
            break;
        }
        cursor = next;
    }
    dates
}

pub fn charge_dates(next_charge: &str, period: &str, today: NaiveDate) -> Vec<NaiveDate> {
    charge_dates_for(next_charge, period, today, DUE_WINDOW_DAYS)
}

fn charge_dates_for(next_charge: &str, period: &str, today: NaiveDate, days: i64) -> Vec<NaiveDate> {
    let Some(start) = parse_date(next_charge) else {
        return Vec::new();
    };
    // Окно начинается сегодня и не включает день ровно через N суток.
    // Для 30 дней: 1 ноября + 30 дней = 1 декабря, месячная подписка не считается дважды.
    let horizon = today + chrono::Duration::days(days.max(1));
    let mut cursor = advance_until(start, period, today);
    let mut dates = Vec::new();
    while cursor < horizon && dates.len() < 60 {
        dates.push(cursor);
        let next = add_period(cursor, period);
        if next <= cursor {
            break;
        }
        cursor = next;
    }
    dates
}

#[allow(dead_code)]
pub fn mark_paid(item: &mut Subscription, today: NaiveDate) {
    let start = parse_date(&item.next_charge).unwrap_or(today);
    let base = if start < today { advance_until(start, &item.period, today) } else { start };
    item.next_charge = add_period(base, &item.period).format("%Y-%m-%d").to_string();
    if item.source == "balance" {
        let left = finite_balance(item.balance) - finite_balance(item.amount);
        item.balance = if left.is_finite() && left > 0.0 { (left * 100.0).round() / 100.0 } else { 0.0 };
    }
}

pub fn add_period(date: NaiveDate, period: &str) -> NaiveDate {
    match period {
        "weekly" => date + chrono::Duration::days(7),
        "quarterly" => add_months(date, 3),
        "yearly" => add_months(date, 12),
        _ => add_months(date, 1),
    }
}

/// 31 января плюс месяц становится 28 или 29 февраля, без паники на коротком месяце.
pub fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let month0 = date.month0() as i32 + months;
    let year = date.year() + month0.div_euclid(12);
    let month = month0.rem_euclid(12) as u32 + 1;
    let day = date.day().min(last_day_of_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).unwrap_or(date)
}

pub fn format_money(amount: f64, currency: &str) -> String {
    let cents = (amount * 100.0).round() as i64;
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.abs();
    let whole = group_digits(cents / 100);
    let fraction = cents % 100;
    let number = if fraction == 0 {
        whole
    } else {
        format!("{whole},{fraction:02}")
    };
    format!("{sign}{number} {}", currency_suffix(currency))
}

fn currency_suffix(currency: &str) -> &str {
    match currency {
        "RUB" => "₽",
        "USD" => "$",
        "EUR" => "€",
        other => other,
    }
}

fn group_digits(value: i64) -> String {
    let digits = value.abs().to_string();
    let mut reversed = String::new();
    for (index, character) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            reversed.push(' ');
        }
        reversed.push(character);
    }
    reversed.chars().rev().collect()
}

fn advance_until(mut date: NaiveDate, period: &str, today: NaiveDate) -> NaiveDate {
    for _ in 0..500 {
        if date >= today {
            return date;
        }
        let next = add_period(date, period);
        if next <= date {
            return date;
        }
        date = next;
    }
    date
}

fn monthly_factor(period: &str) -> f64 {
    match period {
        "weekly" => 52.0 / 12.0,
        "quarterly" => 1.0 / 3.0,
        "yearly" => 1.0 / 12.0,
        _ => 1.0,
    }
}

fn period_label(period: &str) -> &'static str {
    match period {
        "weekly" => "Неделя",
        "quarterly" => "Квартал",
        "yearly" => "Год",
        _ => "Месяц",
    }
}

fn row_view(item: &Subscription, today: NaiveDate, settings: &Settings) -> RowView {
    let (when, date_label) = match parse_date(&item.next_charge) {
        Some(date) if item.active => (title_when(date, today), format_day(date, today)),
        Some(date) => ("На паузе".into(), format_day(date, today)),
        None => ("Проверьте дату".into(), String::new()),
    };
    let mut detail = format!("{} · {}", period_label(&item.period), item.category);
    if item.currency != settings.base {
        detail.push_str(" · ");
        detail.push_str(&format_money(item.amount, &item.currency));
    }
    let (balance, balance_short, badge, can_topup) = if item.source == "balance" {
        let short = shortage_cents(item);
        (
            format!("Баланс {}", format_money(finite_balance(item.balance), &item.currency)),
            short > 0,
            if item.active && short > 0 {
                format!("Не хватает {}", format_money(cents_to_amount(short), &item.currency))
            } else {
                String::new()
            },
            true,
        )
    } else {
        (String::new(), false, String::new(), false)
    };
    if !item.note.trim().is_empty() {
        detail.push_str(" · ");
        detail.push_str(&short(item.note.trim(), 42));
    }
    let price = format_money(
        to_base(item.amount, &item.currency, &settings.base, &settings.rates()),
        &settings.base,
    );
    RowView {
        id: item.id.clone(),
        name: item.name.clone(),
        detail,
        when,
        date: date_label,
        price,
        letter: initial(&item.name),
        pause_label: if item.active { "Пауза" } else { "Включить" }.into(),
        balance,
        balance_short,
        badge,
        can_topup,
        rgb: icon_rgb(item),
        category_rgb: category_rgb(&item.category, settings),
        paused: !item.active,
    }
}

fn month_title(today: NaiveDate) -> String {
    const NAMES: [&str; 12] = [
        "Январь", "Февраль", "Март", "Апрель", "Май", "Июнь", "Июль", "Август", "Сентябрь", "Октябрь", "Ноябрь", "Декабрь",
    ];
    format!("{} {}", NAMES[today.month0() as usize], today.year())
}

fn month_weeks(items: &[Subscription], today: NaiveDate) -> Vec<Vec<DayCell>> {
    let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    let last = NaiveDate::from_ymd_opt(today.year(), today.month(), last_day_of_month(today.year(), today.month())).unwrap_or(today);
    let mut marked = std::collections::BTreeSet::new();
    for item in items.iter().filter(|item| item.active) {
        let Some(start) = parse_date(&item.next_charge) else {
            continue;
        };
        let mut cursor = advance_until(start, &item.period, today);
        for _ in 0..40 {
            if cursor > last {
                break;
            }
            if cursor >= first {
                marked.insert(cursor);
            }
            let next = add_period(cursor, &item.period);
            if next <= cursor {
                break;
            }
            cursor = next;
        }
    }
    let mut cursor = first - chrono::Duration::days(first.weekday().num_days_from_monday() as i64);
    let mut weeks = Vec::new();
    for _ in 0..6 {
        let mut week = Vec::new();
        for _ in 0..7 {
            let in_month = cursor.year() == today.year() && cursor.month() == today.month();
            week.push(DayCell {
                label: if in_month { cursor.day().to_string() } else { String::new() },
                marked: marked.contains(&cursor),
                today: cursor == today,
                dim: !in_month,
            });
            cursor += chrono::Duration::days(1);
        }
        let finished = cursor > last;
        weeks.push(week);
        if finished {
            break;
        }
    }
    weeks
}

fn group_timeline(events: Vec<(NaiveDate, String, String)>, today: NaiveDate) -> Vec<TimelineItem> {
    let mut timeline = Vec::new();
    let mut last = None;
    for (date, name, price) in events {
        if last != Some(date) {
            timeline.push(TimelineItem {
                header: true,
                label: format!("{} · {}", format_day(date, today), when_label(date, today)),
                price: String::new(),
            });
            last = Some(date);
        }
        timeline.push(TimelineItem { header: false, label: name, price });
    }
    timeline
}

fn compare_items(left: &Subscription, right: &Subscription, sort: SortMode, settings: &Settings) -> std::cmp::Ordering {
    let name = left.name.to_lowercase().cmp(&right.name.to_lowercase());
    match sort {
        SortMode::Name => name,
        SortMode::Amount => {
            let amount = |item: &Subscription| {
                to_base(item.amount * monthly_factor(&item.period), &item.currency, &settings.base, &settings.rates())
            };
            amount(right).total_cmp(&amount(left)).then(name)
        }
        SortMode::Date => {
            let day = |item: &Subscription| parse_date(&item.next_charge).unwrap_or(NaiveDate::MAX);
            day(left).cmp(&day(right)).then(name)
        }
    }
    .then_with(|| left.id.cmp(&right.id))
}

fn matches_filter(item: &Subscription, filter: ListFilter) -> bool {
    match filter {
        ListFilter::All => true,
        ListFilter::Active => item.active,
        ListFilter::Paused => !item.active,
    }
}

fn matches_category(item: &Subscription, category: Option<&str>) -> bool {
    category.is_none_or(|value| item.category == value)
}

fn matches_query(item: &Subscription, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    item.name.to_lowercase().contains(&query)
        || item.category.to_lowercase().contains(&query)
        || item.note.to_lowercase().contains(&query)
}

fn format_day(date: NaiveDate, today: NaiveDate) -> String {
    let month = MONTHS[date.month0() as usize];
    if date.year() == today.year() {
        format!("{} {month}", date.day())
    } else {
        format!("{} {month} {}", date.day(), date.year())
    }
}

fn title_when(date: NaiveDate, today: NaiveDate) -> String {
    let label = when_label(date, today);
    let mut chars = label.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn when_label(date: NaiveDate, today: NaiveDate) -> String {
    let days = (date - today).num_days();
    match days {
        0 => "сегодня".into(),
        1 => "завтра".into(),
        n if n > 1 => format!("через {n} {}", ru_word(n, "день", "дня", "дней")),
        n => format!("{} {} назад", -n, ru_word(-n, "день", "дня", "дней")),
    }
}

fn ru_word<'a>(n: i64, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let n = n.abs() % 100;
    if (11..20).contains(&n) {
        return many;
    }
    match n % 10 {
        1 => one,
        2..=4 => few,
        _ => many,
    }
}

fn short(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let trimmed: String = text.chars().take(max).collect();
    format!("{trimmed}…")
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    for day in (28..=31).rev() {
        if NaiveDate::from_ymd_opt(year, month, day).is_some() {
            return day;
        }
    }
    28
}

fn parse_positive(raw: &str) -> Result<f64, &'static str> {
    let compact: String = raw.chars().filter(|character| !character.is_whitespace()).collect();
    if compact.is_empty() {
        return Err("Введите число");
    }
    if compact.contains('.') && compact.contains(',') {
        return Err("Оставьте либо запятую, либо точку");
    }
    let value: f64 = compact.replace(',', ".").parse().map_err(|_| "Введите число")?;
    if !value.is_finite() || value <= 0.0 {
        return Err("Число должно быть больше нуля");
    }
    Ok((value * 100.0).round() / 100.0)
}

fn pick<'a>(options: &'a [&str], index: i32, fallback: &'a str) -> &'a str {
    usize::try_from(index)
        .ok()
        .and_then(|index| options.get(index).copied())
        .unwrap_or(fallback)
}

fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let number = COUNTER.fetch_add(1, Ordering::Relaxed);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{millis:x}{number:x}")
}

fn sanitize_settings(settings: Settings) -> Settings {
    Settings {
        dark: settings.dark,
        base: if CURRENCIES.contains(&settings.base.as_str()) {
            settings.base
        } else {
            default_currency()
        },
        rub_per_usd: finite_rate(settings.rub_per_usd, default_usd()),
        rub_per_eur: finite_rate(settings.rub_per_eur, default_eur()),
        custom_categories: sanitize_categories(settings.custom_categories),
    }
}

fn sanitize_categories(items: Vec<NamedColor>) -> Vec<NamedColor> {
    let mut kept = Vec::new();
    for item in items {
        let name = item.name.trim();
        if name.is_empty() || name.chars().count() > 24 {
            continue;
        }
        if category_names_of(&kept).iter().any(|current| same_name(current, name)) {
            continue;
        }
        kept.push(NamedColor { name: name.to_string(), r: item.r, g: item.g, b: item.b });
        if kept.len() == 24 {
            break;
        }
    }
    kept
}

fn category_names_of(custom: &[NamedColor]) -> Vec<String> {
    let mut names: Vec<String> = CATEGORIES.iter().map(|name| (*name).to_string()).collect();
    names.extend(custom.iter().map(|item| item.name.clone()));
    names
}

fn same_name(left: &str, right: &str) -> bool {
    left.to_lowercase() == right.to_lowercase()
}

fn random_swatch(used: &[(u8, u8, u8)]) -> (u8, u8, u8) {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u32)
        .unwrap_or(1);
    let mut hue = tick.wrapping_mul(2_654_435_761);
    for _ in 0..24 {
        let color = hsl_to_rgb((hue % 360) as f32, 0.62, 0.58);
        let distinct = used.iter().all(|current| color_gap(*current, color) > 48.0);
        if distinct {
            return color;
        }
        hue = hue.wrapping_add(47);
    }
    hsl_to_rgb((hue % 360) as f32, 0.62, 0.58)
}

fn color_gap(left: (u8, u8, u8), right: (u8, u8, u8)) -> f32 {
    let channel = |a: u8, b: u8| (i16::from(a) - i16::from(b)).abs() as f32;
    channel(left.0, right.0) + channel(left.1, right.1) + channel(left.2, right.2)
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> (u8, u8, u8) {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let section = (hue % 360.0) / 60.0;
    let x = chroma * (1.0 - (section % 2.0 - 1.0).abs());
    let (red, green, blue) = match section as i32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let shift = lightness - chroma / 2.0;
    let byte = |channel: f32| ((channel + shift).clamp(0.0, 1.0) * 255.0).round() as u8;
    (byte(red), byte(green), byte(blue))
}

fn finite_balance(value: f64) -> f64 {
    if (0.0..=1_000_000_000.0).contains(&value) {
        (value * 100.0).round() / 100.0
    } else {
        0.0
    }
}

pub fn apply_topup(balance: f64, raw: &str) -> Result<f64, String> {
    let added = parse_positive(raw.trim()).map_err(str::to_string)?;
    let next = finite_balance(balance) + added;
    if next > 1_000_000_000.0 {
        return Err("Слишком большая сумма".into());
    }
    Ok((next * 100.0).round() / 100.0)
}

pub fn parse_balance(raw: &str) -> Result<f64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(0.0);
    }
    let compact: String = trimmed.chars().filter(|character| !character.is_whitespace()).collect();
    if compact.contains('.') && compact.contains(',') {
        return Err("Оставьте либо запятую, либо точку".into());
    }
    let value: f64 = compact.replace(',', ".").parse().map_err(|_| "Введите число".to_string())?;
    if !value.is_finite() || value < 0.0 {
        return Err("Баланс не может быть отрицательным".into());
    }
    if value > 1_000_000_000.0 {
        return Err("Слишком большая сумма".into());
    }
    Ok((value * 100.0).round() / 100.0)
}

fn finite_rate(value: f64, fallback: f64) -> f64 {
    if value.is_finite() && value > 0.0 { value } else { fallback }
}

fn backup_and_warn(path: &Path, reason: &str) -> String {
    let backup = backup_path(path);
    match fs::copy(path, &backup) {
        Ok(_) => format!("{reason}. Копия: {}. Запуск с пустыми данными.", backup.display()),
        Err(error) => format!("{reason}. Не удалось сделать копию: {error}"),
    }
}

fn backup_path(path: &Path) -> PathBuf {
    match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => path.with_file_name(format!("{name}.bak")),
        None => path.with_extension("bak"),
    }
}

/// Windows не перезаписывает файл через rename, поэтому сначала уводим старый в .bak и возвращаем его, если замена не удалась.
fn atomic_write(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
    }
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("data");
    let temporary = path.with_file_name(format!("{file_name}.tmp"));
    fs::write(&temporary, contents).map_err(|error| error.to_string())?;
    if !path.exists() {
        return fs::rename(&temporary, path).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            error.to_string()
        });
    }
    let backup = backup_path(path);
    let _ = fs::remove_file(&backup);
    if let Err(error) = fs::rename(path, &backup) {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::rename(&backup, path);
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    let _ = fs::remove_file(&backup);
    Ok(())
}

fn csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn parse_csv(text: &str) -> Result<Vec<Subscription>, String> {
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let Some(first) = lines.next() else {
        return Err("Файл пуст".into());
    };
    let header = split_csv_line(first);
    let known = header.iter().any(|cell| {
        let lower = cell.to_lowercase();
        lower == "id" || lower == "name" || lower == "название" || lower == "сумма"
    });
    let (header, mut rows) = if known {
        (header, Vec::new())
    } else {
        (default_csv_header(), vec![header])
    };
    rows.extend(lines.map(split_csv_line));
    let id = column(&header, &["id"]);
    let name = column(&header, &["название", "name"]).ok_or("В CSV нет колонки «название»")?;
    let amount = column(&header, &["сумма", "amount"]).ok_or("В CSV нет колонки «сумма»")?;
    let currency = column(&header, &["валюта", "currency"]);
    let period = column(&header, &["период", "period"]);
    let date = column(&header, &["дата", "next_charge", "date"]);
    let category = column(&header, &["категория", "category"]);
    let note = column(&header, &["заметка", "note"]);
    let status = column(&header, &["статус", "active"]);
    let created = column(&header, &["создана", "created_at"]);
    let source = column(&header, &["источник", "source"]);
    let wallet = column(&header, &["баланс", "balance"]);

    let mut items = Vec::new();
    for row in rows {
        let Some(title) = cell(&row, name) else { continue };
        let Ok(value) = cell(&row, amount).map(parse_positive).transpose() else { continue };
        let Some(value) = value else { continue };
        let Some(raw_date) = cell(&row, date.unwrap_or(usize::MAX)) else {
            continue;
        };
        let Some(when) = parse_user_date(raw_date) else {
            continue;
        };
        items.push(Subscription {
            id: cell(&row, id.unwrap_or(usize::MAX)).unwrap_or("").to_string(),
            name: title.to_string(),
            amount: value,
            currency: normalize_currency(cell(&row, currency.unwrap_or(usize::MAX)).unwrap_or("RUB")),
            period: normalize_period(cell(&row, period.unwrap_or(usize::MAX)).unwrap_or("monthly")),
            next_charge: when.format("%Y-%m-%d").to_string(),
            category: normalize_category(cell(&row, category.unwrap_or(usize::MAX)).unwrap_or("Другое")),
            note: cell(&row, note.unwrap_or(usize::MAX)).unwrap_or("").to_string(),
            active: parse_active(cell(&row, status.unwrap_or(usize::MAX)).unwrap_or("активна")),
            source: normalize_source(cell(&row, source.unwrap_or(usize::MAX)).unwrap_or("карта")),
            balance: cell(&row, wallet.unwrap_or(usize::MAX))
                .and_then(|raw| parse_balance(raw).ok())
                .unwrap_or(0.0),
            created_at: cell(&row, created.unwrap_or(usize::MAX)).unwrap_or("").to_string(),
            icon_r: -1,
            icon_g: -1,
            icon_b: -1,
        });
    }
    if items.is_empty() {
        return Err("Не нашёл подписок в CSV".into());
    }
    Ok(items)
}

fn default_csv_header() -> Vec<String> {
    ["id", "название", "сумма", "валюта", "период", "дата", "категория", "заметка", "статус", "создана", "источник", "баланс"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn column(header: &[String], names: &[&str]) -> Option<usize> {
    header.iter().position(|cell| {
        let lower = cell.to_lowercase();
        names.iter().any(|name| lower == *name)
    })
}

fn cell(row: &[String], index: usize) -> Option<&str> {
    row.get(index).map(String::as_str).map(str::trim).filter(|value| !value.is_empty())
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    quotes = false;
                }
            } else {
                current.push(ch);
            }
        } else if ch == '"' && current.trim().is_empty() {
            current.clear();
            quotes = true;
        } else if ch == ',' {
            cells.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    cells.push(current.trim().to_string());
    cells
}

fn normalize_currency(value: &str) -> String {
    match value.trim().to_uppercase().as_str() {
        "USD" | "$" => "USD".into(),
        "EUR" | "€" => "EUR".into(),
        _ => "RUB".into(),
    }
}

fn normalize_period(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "weekly" | "неделя" => "weekly".into(),
        "quarterly" | "квартал" => "quarterly".into(),
        "yearly" | "год" => "yearly".into(),
        _ => "monthly".into(),
    }
}

fn normalize_category(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 24 {
        return default_category();
    }
    CATEGORIES
        .iter()
        .find(|item| same_name(item, trimmed))
        .map(|item| (*item).to_string())
        .unwrap_or_else(|| trimmed.to_string())
}

fn normalize_source(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "balance" | "баланс" | "с баланса" => "balance".into(),
        _ => "card".into(),
    }
}

fn parse_active(value: &str) -> bool {
    !matches!(value.trim().to_lowercase().as_str(), "0" | "false" | "paused" | "пауза" | "нет")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("test date")
    }

    fn sample() -> Subscription {
        Subscription {
            id: "1".into(),
            name: "Облако".into(),
            amount: 1200.0,
            currency: "RUB".into(),
            period: "yearly".into(),
            next_charge: "2026-11-02".into(),
            category: "Облако".into(),
            note: String::new(),
            active: true,
            source: "card".into(),
            balance: 0.0,
            created_at: String::new(),
            icon_r: -1,
            icon_g: -1,
            icon_b: -1,
        }
    }

    #[test]
    fn clamps_month_end_and_leap_day() {
        assert_eq!(add_months(date(2026, 1, 31), 1), date(2026, 2, 28));
        assert_eq!(add_months(date(2024, 1, 31), 1), date(2024, 2, 29));
        assert_eq!(add_months(date(2026, 3, 31), 1), date(2026, 4, 30));
        assert_eq!(add_months(date(2025, 12, 31), 1), date(2026, 1, 31));
    }

    #[test]
    fn missed_date_moves_forward_by_whole_periods() {
        let mut items = vec![Subscription {
            next_charge: "2026-08-03".into(),
            period: "monthly".into(),
            ..sample()
        }];
        assert!(catch_up(&mut items, date(2026, 10, 31)));
        assert_eq!(items[0].next_charge, "2026-11-03");
        assert!(!catch_up(&mut items, date(2026, 10, 31)));
    }

    #[test]
    fn early_payment_skips_one_period_then_catches_up() {
        let mut early = vec![Subscription {
            next_charge: add_period(date(2026, 11, 3), "monthly").format("%Y-%m-%d").to_string(),
            period: "monthly".into(),
            ..sample()
        }];
        catch_up(&mut early, date(2026, 10, 31));
        assert_eq!(early[0].next_charge, "2026-12-03");

        let mut overdue = vec![Subscription {
            next_charge: add_period(date(2026, 8, 3), "monthly").format("%Y-%m-%d").to_string(),
            period: "monthly".into(),
            ..sample()
        }];
        catch_up(&mut overdue, date(2026, 10, 31));
        assert_eq!(overdue[0].next_charge, "2026-11-03");
    }

    #[test]
    fn parses_dotted_date() {
        assert_eq!(parse_user_date("31.10.2026"), Some(date(2026, 10, 31)));
        assert_eq!(parse_user_date("31.02.2026"), None);
        assert_eq!(format_input_date(date(2026, 10, 5)), "05.10.2026");
    }

    #[test]
    fn formats_rubles_with_groups() {
        assert_eq!(format_money(799.0, "RUB"), "799 ₽");
        assert_eq!(format_money(6000.0, "RUB"), "6 000 ₽");
        assert_eq!(format_money(1234.5, "RUB"), "1 234,50 ₽");
    }

    #[test]
    fn monthly_charge_is_counted_once_inside_thirty_days() {
        let today = date(2026, 11, 1);
        let dates = charge_dates("2026-11-01", "monthly", today);
        assert_eq!(dates, vec![today]);
        let items = vec![Subscription {
            amount: 800.0,
            period: "monthly".into(),
            next_charge: "2026-11-01".into(),
            ..sample()
        }];
        let view = ViewQuery {
            text: String::new(),
            filter: ListFilter::Active,
            sort: SortMode::Date,
            category: None,
            horizon_days: 30,
        };
        let summary = compose(&items, &view, today, &Settings::default());
        assert_eq!(summary.monthly, "800 ₽");
        assert_eq!(summary.due, "800 ₽");
    }

    #[test]
    fn weekly_charges_cover_the_next_month_once_each() {
        let dates = charge_dates("2026-10-31", "weekly", date(2026, 10, 31));
        assert_eq!(dates.len(), 5);
        assert_eq!(dates[0], date(2026, 10, 31));
        assert_eq!(*dates.last().expect("dates"), date(2026, 11, 28));
        let overdue = charge_dates("2026-10-01", "weekly", date(2026, 10, 31));
        assert!(overdue.iter().all(|day| *day >= date(2026, 10, 31)));
        assert_eq!(overdue.len(), overdue.iter().collect::<std::collections::HashSet<_>>().len());
    }

    #[test]
    fn converts_currencies_with_user_rates() {
        let rates = Rates { rub_per_usd: 90.0, rub_per_eur: 98.0 };
        assert!((to_base(10.0, "USD", "RUB", &rates) - 900.0).abs() < 0.001);
        assert!((to_base(900.0, "RUB", "USD", &rates) - 10.0).abs() < 0.001);
        assert!((to_base(10.0, "EUR", "RUB", &rates) - 980.0).abs() < 0.001);

        let mut settings = Settings::default();
        settings.rub_per_usd = 90.0;
        let items = vec![Subscription {
            amount: 10.0,
            currency: "USD".into(),
            period: "monthly".into(),
            next_charge: "2026-11-02".into(),
            ..sample()
        }];
        let view = ViewQuery {
            text: String::new(),
            filter: ListFilter::Active,
            sort: SortMode::Date,
            category: None,
            horizon_days: 30,
        };
        let summary = compose(&items, &view, date(2026, 10, 31), &settings);
        assert_eq!(summary.monthly, "900 ₽");
        assert_eq!(summary.rows[0].price, "900 ₽");
    }

    #[test]
    fn totals_use_monthly_and_yearly_equivalents() {
        let items = vec![sample()];
        let view = ViewQuery {
            text: String::new(),
            filter: ListFilter::Active,
            sort: SortMode::Date,
            category: None,
            horizon_days: 30,
        };
        let summary = compose(&items, &view, date(2026, 10, 31), &Settings::default());
        assert_eq!(summary.monthly, "100 ₽");
        assert_eq!(summary.yearly, "1 200 ₽");
        assert_eq!(summary.active_count, "1");
        assert_eq!(summary.rows.len(), 1);
        assert_eq!(summary.timeline.iter().filter(|item| !item.header).count(), 1);
    }

    #[test]
    fn weekly_due_counts_every_charge_in_the_window() {
        let items = vec![Subscription {
            amount: 100.0,
            period: "weekly".into(),
            next_charge: "2026-10-31".into(),
            ..sample()
        }];
        let view = ViewQuery {
            text: String::new(),
            filter: ListFilter::Active,
            sort: SortMode::Date,
            category: None,
            horizon_days: 30,
        };
        let summary = compose(&items, &view, date(2026, 10, 31), &Settings::default());
        assert_eq!(summary.due, "500 ₽");
        assert_eq!(summary.yearly, "5 200 ₽");
    }

    #[test]
    fn month_left_stops_at_the_end_of_the_month() {
        let today = date(2026, 11, 15);
        let items = vec![Subscription {
            amount: 100.0,
            period: "weekly".into(),
            next_charge: "2026-11-15".into(),
            ..sample()
        }];
        let view = ViewQuery {
            text: String::new(),
            filter: ListFilter::Active,
            sort: SortMode::Date,
            category: None,
            horizon_days: 30,
        };
        let summary = compose(&items, &view, today, &Settings::default());
        assert_eq!(summary.month_left, "300 ₽");
        assert_eq!(summary.due, "500 ₽");
        let week = compose(
            &items,
            &ViewQuery { horizon_days: 7, ..view },
            today,
            &Settings::default(),
        );
        assert_eq!(week.horizon_total, "100 ₽");
    }

    #[test]
    fn roundtrip_file_and_corrupt_backup() {
        let path = std::env::temp_dir().join(format!("trackpurchase-test-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup_path(&path));
        let items = vec![Subscription {
            name: "Netflix".into(),
            amount: 799.0,
            period: "monthly".into(),
            next_charge: "2026-11-03".into(),
            category: "Видео".into(),
            created_at: "2026-10-31T00:00:00+03:00".into(),
            ..sample()
        }];
        save_subscriptions(&path, &items).expect("save");
        assert_eq!(load_subscriptions(&path).value, items);

        fs::write(&path, "{not json").expect("corrupt");
        let broken = load_subscriptions(&path);
        assert!(broken.value.is_empty());
        assert!(broken.warning.contains("повреждён"));
        assert!(backup_path(&path).exists());
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup_path(&path));
    }

    #[test]
    fn csv_roundtrip() {
        let path = std::env::temp_dir().join(format!("trackpurchase-test-{}.csv", std::process::id()));
        let items = vec![Subscription {
            name: "Кино, дом".into(),
            amount: 799.0,
            period: "monthly".into(),
            next_charge: "2026-11-03".into(),
            category: "Видео".into(),
            note: "семейный".into(),
            ..sample()
        }];
        export_csv(&path, &items).expect("csv");
        let imported = import_subscriptions(&path).expect("import");
        assert_eq!(imported[0].name, "Кино, дом");
        assert_eq!(imported[0].amount, 799.0);
        assert_eq!(imported[0].category, "Видео");
        assert_eq!(imported[0].note, "семейный");
        assert_eq!(imported[0].source, "card");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn old_files_default_to_card_and_zero_balance() {
        let item: Subscription = serde_json::from_str(
            r#"{"id":"1","name":"Кино","amount":10,"next_charge":"2026-11-01"}"#,
        )
        .expect("json");
        assert_eq!(item.source, "card");
        assert_eq!(item.balance, 0.0);
        assert_eq!(item.icon_r, -1);
        let hashed = icon_rgb(&item);
        let mut painted = item.clone();
        painted.icon_r = 12;
        painted.icon_g = 34;
        painted.icon_b = 56;
        assert_eq!(icon_rgb(&painted), (12, 34, 56));
        assert_ne!(icon_rgb(&painted), hashed);
        let mut other = item.clone();
        other.name = "Другое имя".into();
        assert_ne!(icon_rgb(&item), icon_rgb(&other));
        let settings: Settings = serde_json::from_str(r#"{"dark":false,"balance":500}"#).expect("settings");
        assert!(!sanitize_settings(settings).dark);
        assert_eq!(parse_balance("").expect("empty"), 0.0);
        assert_eq!(parse_balance("1 500,50").expect("grouped"), 1500.50);
        let mut settings = Settings::default();
        let sport = add_custom_category(&mut settings, " Спорт ").expect("category");
        assert_eq!(category_names(&settings)[sport], "Спорт");
        let first = category_rgb("Спорт", &settings);
        assert!(add_custom_category(&mut settings, "спорт").is_err());
        assert!(add_custom_category(&mut settings, "Видео").is_err());
        assert_eq!(category_rgb("Спорт", &settings), first);
        assert!(!remember_category(&mut settings, "Спорт"));
        assert!(remember_category(&mut settings, "Кино"));
        assert_ne!(category_rgb("Кино", &settings), category_rgb("Спорт", &settings));
        assert_eq!(apply_topup(551.0, "449").expect("topup"), 1000.0);
        assert!(apply_topup(551.0, "0").is_err());
    }

    #[test]
    fn each_service_spends_only_its_own_balance() {
        let today = date(2026, 11, 1);
        let items = vec![
            Subscription {
                name: "Облако".into(),
                amount: 100.0,
                period: "weekly".into(),
                next_charge: "2026-11-01".into(),
                source: "balance".into(),
                balance: 250.0,
                ..sample()
            },
            Subscription {
                name: "Музыка".into(),
                amount: 100.0,
                period: "weekly".into(),
                next_charge: "2026-11-01".into(),
                source: "balance".into(),
                balance: 100.0,
                ..sample()
            },
            Subscription {
                name: "Карта".into(),
                amount: 9_000.0,
                period: "weekly".into(),
                next_charge: "2026-11-01".into(),
                source: "card".into(),
                balance: 50_000.0,
                ..sample()
            },
        ];
        let lines = balance_lines(&items, today);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].name, "Музыка");
        assert_eq!(lines[0].until, "Закончится 1 ноября");
        assert_eq!(lines[1].name, "Облако");
        assert_eq!(lines[1].until, "Хватит до 8 ноября");
        assert!(lines[1].detail.contains("15 ноября"), "{}", lines[1].detail);
        assert!(lines[1].detail.contains("50"), "{}", lines[1].detail);
    }

    #[test]
    fn balance_ends_on_the_charge_that_zeroes_it() {
        let item = Subscription {
            amount: 100.0,
            period: "weekly".into(),
            next_charge: "2026-11-01".into(),
            source: "balance".into(),
            balance: 200.0,
            ..sample()
        };
        let forecast = service_forecast(&item, date(2026, 11, 1));
        assert_eq!(forecast.until, "Закончится 8 ноября");
    }

    #[test]
    fn service_balance_stays_in_its_own_currency() {
        let item = Subscription {
            amount: 10.0,
            currency: "USD".into(),
            period: "weekly".into(),
            next_charge: "2026-11-01".into(),
            source: "balance".into(),
            balance: 25.0,
            ..sample()
        };
        let forecast = service_forecast(&item, date(2026, 11, 1));
        assert_eq!(forecast.until, "Хватит до 8 ноября");
        assert!(forecast.detail.contains("15 ноября"), "{}", forecast.detail);
        assert!(forecast.detail.contains("5 $"), "{}", forecast.detail);
    }

    #[test]
    fn mts_and_kinopoisk_keep_separate_balances() {
        let today = date(2026, 11, 1);
        let items = vec![
            Subscription {
                name: "МТС".into(),
                amount: 800.0,
                period: "monthly".into(),
                next_charge: "2026-11-01".into(),
                source: "balance".into(),
                balance: 2_000.0,
                category: "Связь".into(),
                ..sample()
            },
            Subscription {
                name: "Кинопоиск".into(),
                amount: 400.0,
                period: "monthly".into(),
                next_charge: "2026-11-01".into(),
                source: "balance".into(),
                balance: 3_000.0,
                category: "Видео".into(),
                ..sample()
            },
        ];
        let lines = balance_lines(&items, today);
        assert_eq!(lines[0].name, "МТС");
        assert_eq!(lines[0].amount, "2 000 ₽");
        assert_eq!(lines[0].until, "Хватит до 1 декабря");
        assert!(lines[0].detail.contains("1 января"), "{}", lines[0].detail);
        assert!(lines[0].detail.contains("400"), "{}", lines[0].detail);
        assert_eq!(lines[1].name, "Кинопоиск");
        assert_eq!(lines[1].amount, "3 000 ₽");
        assert_eq!(lines[1].until, "Хватит до 1 мая 2027");
        assert!(lines[1].detail.contains("1 июня 2027"), "{}", lines[1].detail);
        assert!(lines[1].detail.contains("200"), "{}", lines[1].detail);
    }

    #[test]
    fn empty_balance_does_not_cover_the_first_charge() {
        let item = Subscription {
            amount: 100.0,
            period: "monthly".into(),
            next_charge: "2026-11-01".into(),
            source: "balance".into(),
            balance: 0.0,
            ..sample()
        };
        let forecast = service_forecast(&item, date(2026, 11, 1));
        assert_eq!(forecast.until, "Баланс пуст");
        assert!(forecast.detail.contains("100"), "{}", forecast.detail);
    }

    #[test]
    fn paid_moves_the_date_and_spends_that_service_balance() {
        let mut card = Subscription {
            amount: 800.0,
            period: "monthly".into(),
            next_charge: "2026-11-01".into(),
            source: "card".into(),
            balance: 500.0,
            ..sample()
        };
        mark_paid(&mut card, date(2026, 11, 1));
        assert_eq!(card.next_charge, "2026-12-01");
        assert_eq!(card.balance, 500.0);

        let mut wallet = Subscription {
            amount: 800.0,
            period: "monthly".into(),
            next_charge: "2026-11-01".into(),
            source: "balance".into(),
            balance: 2_000.0,
            ..sample()
        };
        mark_paid(&mut wallet, date(2026, 11, 1));
        assert_eq!(wallet.next_charge, "2026-12-01");
        assert_eq!(wallet.balance, 1_200.0);

        let mut short = wallet.clone();
        short.balance = 551.0;
        short.next_charge = "2026-12-01".into();
        mark_paid(&mut short, date(2026, 12, 1));
        assert_eq!(short.balance, 0.0);
        assert_eq!(short.next_charge, "2027-01-01");
    }

    #[test]
    fn month_calendar_marks_only_this_month() {
        let today = date(2026, 11, 1);
        let items = vec![Subscription {
            amount: 800.0,
            period: "weekly".into(),
            next_charge: "2026-11-01".into(),
            ..sample()
        }];
        let weeks = month_weeks(&items, today);
        let marked = weeks
            .iter()
            .flatten()
            .filter(|cell| cell.marked)
            .map(|cell| cell.label.clone())
            .collect::<Vec<_>>();
        assert_eq!(marked, ["1", "8", "15", "22", "29"]);
        assert_eq!(month_title(today), "Ноябрь 2026");
        assert!(weeks[0][6].today);
    }
}
