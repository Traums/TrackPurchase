#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod logic;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

slint::include_modules!();

struct AppState {
    items: Vec<logic::Subscription>,
    settings: logic::Settings,
    editing: Option<String>,
    topping: Option<String>,
    attempted: bool,
    subs_path: std::path::PathBuf,
    settings_path: std::path::PathBuf,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Подписки: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let ui = MainWindow::new()?;
    let subs_path = logic::subscriptions_path();
    let settings_path = logic::settings_path();
    let loaded_settings = logic::load_settings(&settings_path);
    let loaded_items = logic::load_subscriptions(&subs_path);
    let mut items = loaded_items.value;
    let mut settings = loaded_settings.value;
    let mut remembered = false;
    for item in &items {
        if logic::remember_category(&mut settings, &item.category) {
            remembered = true;
        }
    }
    if remembered {
        let _ = logic::save_settings(&settings_path, &settings);
    }
    if logic::catch_up(&mut items, logic::today()) {
        if let Err(error) = logic::save_subscriptions(&subs_path, &items) {
            eprintln!("Не удалось обновить даты: {error}");
        }
    }
    let warning = [loaded_items.warning, loaded_settings.warning]
        .into_iter()
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let state = Rc::new(RefCell::new(AppState {
        items,
        settings: settings.clone(),
        editing: None,
        topping: None,
        attempted: false,
        subs_path,
        settings_path,
    }));

    ui.global::<Theme>().set_dark(settings.dark);
    ui.invoke_apply_theme();
    ui.set_category_choices(category_choices(&settings));
    ui.set_data_path(logic::app_dir().display().to_string().into());
    wire(&ui, &state);
    sync(&ui, &state);
    if !warning.is_empty() {
        ui.set_status(warning.into());
    }
    ui.window().set_position(slint::LogicalPosition::new(32.0, 32.0));
    ui.window().set_size(slint::LogicalSize::new(1200.0, 800.0));
    let wide_timer = slint::Timer::default();
    let weak = ui.as_weak();
    let enlarge_attempts = Rc::new(Cell::new(0_i32));
    wide_timer.start(slint::TimerMode::Repeated, Duration::from_millis(200), move || {
        let Some(ui) = weak.upgrade() else { return };
        let logical = ui.window().size().to_logical(ui.window().scale_factor());
        if logical.width < 1.0 {
            return;
        }
        // Первые кадры иногда открываются уже минимума и прячут правую колонку.
        if logical.width < 1100.0 && enlarge_attempts.get() < 6 {
            enlarge_attempts.set(enlarge_attempts.get() + 1);
            ui.window().set_size(slint::LogicalSize::new(1200.0, 800.0));
            ui.set_wide(true);
            return;
        }
        update_wide(&ui);
    });
    ui.run()?;
    Ok(())
}

fn update_wide(ui: &MainWindow) {
    let logical = ui.window().size().to_logical(ui.window().scale_factor());
    if logical.width < 1.0 {
        return;
    }
    let wide = logical.width >= 880.0;
    if ui.get_wide() != wide {
        ui.set_wide(wide);
    }
}

fn wire(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    ui.on_view_changed({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                sync(&ui, &state);
            }
        }
    });
    ui.on_validate_form({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                revalidate(&ui, &state);
            }
        }
    });
    ui.on_submit_form({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                submit(&ui, &state);
            }
        }
    });
    ui.on_open_form({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                open_form(&ui, &state, None);
            }
        }
    });
    ui.on_close_form({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                close_form(&ui, &state);
            }
        }
    });
    ui.on_open_topup({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move |id| {
            if let Some(ui) = weak.upgrade() {
                open_topup(&ui, &state, id.to_string());
            }
        }
    });
    ui.on_close_topup({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                close_topup(&ui, &state);
            }
        }
    });
    ui.on_add_category({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                add_category(&ui, &state);
            }
        }
    });
    ui.on_validate_topup({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                refresh_topup(&ui, &state);
            }
        }
    });
    ui.on_submit_topup({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                submit_topup(&ui, &state);
            }
        }
    });
    ui.on_edit_item({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move |id| {
            if let Some(ui) = weak.upgrade() {
                let id = id.to_string();
                let item = state.borrow().items.iter().find(|item| item.id == id).cloned();
                if let Some(item) = item {
                    open_form(&ui, &state, Some(item));
                }
            }
        }
    });
    ui.on_toggle_pause({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move |id| {
            if let Some(ui) = weak.upgrade() {
                toggle_pause(&ui, &state, id.to_string());
            }
        }
    });
    ui.on_remove_item({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move |id| {
            if let Some(ui) = weak.upgrade() {
                remove_item(&ui, &state, id.to_string());
            }
        }
    });
    ui.on_open_settings({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                open_settings(&ui, &state);
            }
        }
    });
    ui.on_close_settings({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_settings_open(false);
            }
        }
    });
    ui.on_apply_settings({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                apply_settings(&ui, &state);
            }
        }
    });
    ui.on_theme_changed({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                save_theme(&ui, &state);
            }
        }
    });
    ui.on_export_json({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                export_copy(&ui, &state, true);
            }
        }
    });
    ui.on_export_csv({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                export_copy(&ui, &state, false);
            }
        }
    });
    ui.on_import_file({
        let weak = ui.as_weak();
        let state = Rc::clone(state);
        move || {
            if let Some(ui) = weak.upgrade() {
                import_file(&ui, &state);
            }
        }
    });
}

fn sync(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    let dashboard = {
        let mut app = state.borrow_mut();
        if logic::catch_up(&mut app.items, logic::today()) {
            if let Err(error) = logic::save_subscriptions(&app.subs_path, &app.items) {
                ui.set_status(format!("Не удалось сохранить даты: {error}").into());
            }
        }
        let query = view_of(ui, &app.settings);
        logic::compose(&app.items, &query, logic::today(), &app.settings)
    };
    ui.set_monthly(dashboard.monthly.into());
    ui.set_yearly(dashboard.yearly.into());
    ui.set_due(dashboard.due.into());
    ui.set_active_count(dashboard.active_count.into());
    ui.set_hint(dashboard.hint.into());
    ui.set_empty(dashboard.empty);
    ui.set_no_results(dashboard.no_results);
    ui.set_confirm_id(SharedString::default());
    ui.set_rows(as_model(dashboard.rows.into_iter().map(row_model).collect()));
    ui.set_balances(as_model(dashboard.balances.into_iter().map(balance_model).collect()));
    ui.set_timeline(as_model(dashboard.timeline.into_iter().map(timeline_model).collect()));
    ui.set_categories(as_model(dashboard.categories.into_iter().map(category_model).collect()));
}

fn view_of(ui: &MainWindow, settings: &logic::Settings) -> logic::ViewQuery {
    let names = logic::category_names(settings);
    let index = ui.get_category_index();
    let category = if index <= 0 {
        None
    } else {
        names.get((index as usize).saturating_sub(1)).cloned()
    };
    logic::ViewQuery {
        text: ui.get_query().to_string(),
        filter: match ui.get_filter_index() {
            0 => logic::ListFilter::All,
            2 => logic::ListFilter::Paused,
            _ => logic::ListFilter::Active,
        },
        sort: match ui.get_sort_index() {
            1 => logic::SortMode::Amount,
            2 => logic::SortMode::Name,
            _ => logic::SortMode::Date,
        },
        category,
    }
}

fn revalidate(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    let show = state.borrow().attempted;
    let check = logic::validate(&read_form(ui), show);
    show_errors(ui, &check);
}

fn show_errors(ui: &MainWindow, check: &logic::FormCheck) {
    ui.set_name_error(check.name_error.as_str().into());
    ui.set_amount_error(check.amount_error.as_str().into());
    ui.set_date_error(check.date_error.as_str().into());
    ui.set_note_error(check.note_error.as_str().into());
    ui.set_form_balance_error(check.balance_error.as_str().into());
    ui.set_can_save(check.valid);
}

fn submit(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    state.borrow_mut().attempted = true;
    let draft = read_form(ui);
    let check = logic::validate(&draft, true);
    show_errors(ui, &check);
    if !check.valid {
        return;
    }
    let subscription = {
        let app = state.borrow();
        let existing = app.editing.as_deref().and_then(|id| app.items.iter().find(|item| item.id == id)).cloned();
        let names = logic::category_names(&app.settings);
        match logic::subscription_from_form(draft, existing.as_ref(), &names) {
            Ok(item) => item,
            Err(message) => {
                drop(app);
                ui.set_status(message.into());
                return;
            }
        }
    };
    let saved = {
        let mut app = state.borrow_mut();
        if let Some(id) = app.editing.clone() {
            if let Some(current) = app.items.iter_mut().find(|item| item.id == id) {
                *current = subscription;
            } else {
                app.items.push(subscription);
            }
        } else {
            app.items.push(subscription);
        }
        app.editing = None;
        app.attempted = false;
        logic::save_subscriptions(&app.subs_path, &app.items)
    };
    ui.set_form_open(false);
    apply_save(ui, saved);
    sync(ui, state);
}

fn open_form(ui: &MainWindow, state: &Rc<RefCell<AppState>>, current: Option<logic::Subscription>) {
    {
        let mut app = state.borrow_mut();
        app.attempted = false;
        app.editing = current.as_ref().map(|item| item.id.clone());
    }
    ui.set_settings_open(false);
    close_topup(ui, state);
    let settings = state.borrow().settings.clone();
    fill_form(ui, current.as_ref(), &settings);
    ui.set_form_open(true);
    revalidate(ui, state);
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(40), move || {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_focus_name();
        }
    });
}

fn close_form(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    let mut app = state.borrow_mut();
    app.editing = None;
    app.attempted = false;
    drop(app);
    ui.set_form_open(false);
}

fn toggle_pause(ui: &MainWindow, state: &Rc<RefCell<AppState>>, id: String) {
    let saved = {
        let mut app = state.borrow_mut();
        if let Some(item) = app.items.iter_mut().find(|item| item.id == id) {
            item.active = !item.active;
        }
        logic::save_subscriptions(&app.subs_path, &app.items)
    };
    apply_save(ui, saved);
    sync(ui, state);
}

fn remove_item(ui: &MainWindow, state: &Rc<RefCell<AppState>>, id: String) {
    let saved = {
        let mut app = state.borrow_mut();
        app.items.retain(|item| item.id != id);
        if app.editing.as_deref() == Some(id.as_str()) {
            app.editing = None;
            ui.set_form_open(false);
        }
        logic::save_subscriptions(&app.subs_path, &app.items)
    };
    apply_save(ui, saved);
    sync(ui, state);
}

fn open_settings(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    let settings = state.borrow().settings.clone();
    ui.set_form_open(false);
    ui.set_settings_usd(logic::amount_field(settings.rub_per_usd).into());
    ui.set_settings_eur(logic::amount_field(settings.rub_per_eur).into());
    ui.set_settings_base_index(logic::index_of(&logic::CURRENCIES, &settings.base, 0));
    ui.set_import_path(logic::export_json_path().display().to_string().into());
    ui.set_settings_error(SharedString::default());
    ui.set_settings_open(true);
}

fn apply_settings(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    let usd = match logic::parse_rate(&ui.get_settings_usd()) {
        Ok(value) => value,
        Err(error) => {
            ui.set_settings_error(error.into());
            return;
        }
    };
    let eur = match logic::parse_rate(&ui.get_settings_eur()) {
        Ok(value) => value,
        Err(error) => {
            ui.set_settings_error(error.into());
            return;
        }
    };
    let base = logic::CURRENCIES.get(usize::try_from(ui.get_settings_base_index()).unwrap_or(0)).copied().unwrap_or("RUB");
    let saved = {
        let mut app = state.borrow_mut();
        app.settings.dark = ui.global::<Theme>().get_dark();
        app.settings.base = base.to_string();
        app.settings.rub_per_usd = usd;
        app.settings.rub_per_eur = eur;
        logic::save_settings(&app.settings_path, &app.settings)
    };
    match saved {
        Ok(()) => {
            ui.set_settings_error(SharedString::default());
            ui.set_settings_open(false);
            ui.set_status(SharedString::default());
            sync(ui, state);
        }
        Err(error) => ui.set_settings_error(format!("Не сохранилось: {error}").into()),
    }
}

fn save_theme(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    let saved = {
        let mut app = state.borrow_mut();
        app.settings.dark = ui.global::<Theme>().get_dark();
        logic::save_settings(&app.settings_path, &app.settings)
    };
    if let Err(error) = saved {
        ui.set_status(format!("Не удалось сохранить тему: {error}").into());
    }
}

fn export_copy(ui: &MainWindow, state: &Rc<RefCell<AppState>>, json: bool) {
    let items = state.borrow().items.clone();
    let path = if json { logic::export_json_path() } else { logic::export_csv_path() };
    let result = if json { logic::export_json(&path, &items) } else { logic::export_csv(&path, &items) };
    match result {
        Ok(()) => ui.set_status(format!("Файл: {}", path.display()).into()),
        Err(error) => ui.set_status(format!("Экспорт не удался: {error}").into()),
    }
}

fn import_file(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    let path = std::path::PathBuf::from(ui.get_import_path().to_string().trim());
    if path.as_os_str().is_empty() {
        ui.set_settings_error("Укажите путь к JSON или CSV".into());
        return;
    }
    let incoming = match logic::import_subscriptions(&path) {
        Ok(items) => items,
        Err(error) => {
            ui.set_settings_error(error.into());
            return;
        }
    };
    let (count, saved) = {
        let mut app = state.borrow_mut();
        let count = logic::merge_imported(&mut app.items, incoming);
        logic::catch_up(&mut app.items, logic::today());
        let mut added = false;
        let names = app.items.iter().map(|item| item.category.clone()).collect::<Vec<_>>();
        for name in names {
            if logic::remember_category(&mut app.settings, &name) {
                added = true;
            }
        }
        if added {
            let _ = logic::save_settings(&app.settings_path, &app.settings);
        }
        (count, logic::save_subscriptions(&app.subs_path, &app.items))
    };
    match saved {
        Ok(()) => {
            ui.set_settings_error(SharedString::default());
            ui.set_status(format!("Импортировано: {count}").into());
            refresh_choices(ui, state);
            sync(ui, state);
        }
        Err(error) => ui.set_settings_error(format!("Импорт прочитан, но не сохранён: {error}").into()),
    }
}

fn read_form(ui: &MainWindow) -> logic::FormDraft {
    logic::FormDraft {
        name: ui.get_form_name().to_string(),
        amount: ui.get_form_amount().to_string(),
        currency_index: ui.get_form_currency_index(),
        period_index: ui.get_form_period_index(),
        category_index: ui.get_form_category_index(),
        date: ui.get_form_date().to_string(),
        note: ui.get_form_note().to_string(),
        active: ui.get_form_active(),
        source_index: ui.get_form_source_index(),
        balance: ui.get_form_balance().to_string(),
    }
}

fn add_category(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    let name = ui.get_category_draft().to_string();
    let saved = {
        let mut app = state.borrow_mut();
        match logic::add_custom_category(&mut app.settings, &name) {
            Ok(index) => {
                let result = logic::save_settings(&app.settings_path, &app.settings);
                (index, result)
            }
            Err(error) => {
                ui.set_category_error(error.into());
                return;
            }
        }
    };
    match saved.1 {
        Ok(()) => {
            ui.set_category_error(SharedString::default());
            ui.set_category_draft(SharedString::default());
            ui.set_form_category_index(saved.0 as i32);
            refresh_choices(ui, state);
        }
        Err(error) => ui.set_category_error(format!("Не сохранилось: {error}").into()),
    }
}

fn refresh_choices(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    let settings = state.borrow().settings.clone();
    ui.set_category_choices(category_choices(&settings));
}

fn fill_form(ui: &MainWindow, current: Option<&logic::Subscription>, settings: &logic::Settings) {
    match current {
        None => {
            ui.set_form_title("Новая подписка".into());
            ui.set_form_name(SharedString::default());
            ui.set_form_amount(SharedString::default());
            ui.set_form_note(SharedString::default());
            ui.set_form_active(true);
            ui.set_form_currency_index(0);
            ui.set_form_period_index(1);
            ui.set_form_source_index(0);
            ui.set_form_balance(SharedString::default());
            ui.set_form_category_index(logic::default_category_index(settings));
            ui.set_category_draft(SharedString::default());
            ui.set_category_error(SharedString::default());
            ui.set_form_date(logic::format_input_date(logic::today()).into());
        }
        Some(subscription) => {
            ui.set_form_title("Изменить подписку".into());
            ui.set_form_name(subscription.name.as_str().into());
            ui.set_form_amount(logic::amount_field(subscription.amount).into());
            ui.set_form_note(subscription.note.as_str().into());
            ui.set_form_active(subscription.active);
            ui.set_form_currency_index(logic::index_of(&logic::CURRENCIES, &subscription.currency, 0));
            ui.set_form_period_index(logic::index_of(&logic::PERIODS, &subscription.period, 1));
            ui.set_form_source_index(if subscription.source == "balance" { 1 } else { 0 });
            ui.set_form_balance(if subscription.source == "balance" || subscription.balance > 0.0 {
                logic::amount_field(subscription.balance).into()
            } else {
                SharedString::default()
            });
            let names = logic::category_names(settings);
            let fallback = logic::default_category_index(settings);
            let index = names
                .iter()
                .position(|name| name == &subscription.category)
                .map(|index| index as i32)
                .unwrap_or(fallback);
            ui.set_form_category_index(index);
            ui.set_category_draft(SharedString::default());
            ui.set_category_error(SharedString::default());
            let date = logic::parse_date(&subscription.next_charge)
                .map(logic::format_input_date)
                .unwrap_or_default();
            ui.set_form_date(date.into());
        }
    }
}

fn open_topup(ui: &MainWindow, state: &Rc<RefCell<AppState>>, id: String) {
    let item = state.borrow().items.iter().find(|item| item.id == id && item.source == "balance").cloned();
    let Some(item) = item else { return };
    state.borrow_mut().topping = Some(item.id);
    ui.set_form_open(false);
    ui.set_settings_open(false);
    ui.set_topup_title(format!("Пополнить {}", item.name).into());
    ui.set_topup_current(format!("Сейчас {}", logic::format_money(item.balance, &item.currency)).into());
    ui.set_topup_amount(SharedString::default());
    ui.set_topup_next(SharedString::default());
    ui.set_topup_error(SharedString::default());
    ui.set_topup_can_save(false);
    ui.set_topup_open(true);
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(40), move || {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_focus_topup();
        }
    });
}

fn close_topup(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    state.borrow_mut().topping = None;
    ui.set_topup_open(false);
}

fn refresh_topup(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    let current = {
        let app = state.borrow();
        app.topping.as_ref().and_then(|id| app.items.iter().find(|item| item.id == *id)).cloned()
    };
    let Some(item) = current else { return };
    let raw = ui.get_topup_amount().to_string();
    if raw.trim().is_empty() {
        ui.set_topup_error(SharedString::default());
        ui.set_topup_next(SharedString::default());
        ui.set_topup_can_save(false);
        return;
    }
    match logic::apply_topup(item.balance, &raw) {
        Ok(next) => {
            ui.set_topup_error(SharedString::default());
            ui.set_topup_next(format!("Станет {}", logic::format_money(next, &item.currency)).into());
            ui.set_topup_can_save(true);
        }
        Err(error) => {
            ui.set_topup_error(error.into());
            ui.set_topup_next(SharedString::default());
            ui.set_topup_can_save(false);
        }
    }
}

fn submit_topup(ui: &MainWindow, state: &Rc<RefCell<AppState>>) {
    refresh_topup(ui, state);
    if !ui.get_topup_can_save() {
        return;
    }
    let raw = ui.get_topup_amount().to_string();
    let saved = {
        let mut app = state.borrow_mut();
        let Some(id) = app.topping.clone() else { return };
        let Some(item) = app.items.iter_mut().find(|item| item.id == id) else { return };
        match logic::apply_topup(item.balance, &raw) {
            Ok(next) => {
                item.balance = next;
                logic::save_subscriptions(&app.subs_path, &app.items)
            }
            Err(error) => {
                ui.set_topup_error(error.into());
                return;
            }
        }
    };
    apply_save(ui, saved);
    close_topup(ui, state);
    sync(ui, state);
}

fn apply_save(ui: &MainWindow, result: Result<(), String>) {
    match result {
        Ok(()) => ui.set_status(SharedString::default()),
        Err(error) => ui.set_status(format!("Не удалось сохранить: {error}").into()),
    }
}

fn row_model(row: logic::RowView) -> SubscriptionRow {
    SubscriptionRow {
        id: row.id.into(),
        name: row.name.into(),
        detail: row.detail.into(),
        when: row.when.into(),
        price: row.price.into(),
        letter: row.letter.into(),
        swatch: paint(row.rgb),
        paused: row.paused,
        pause_label: row.pause_label.into(),
        balance_text: row.balance.into(),
        can_topup: row.can_topup,
    }
}

fn balance_model(line: logic::BalanceLine) -> BalanceRow {
    BalanceRow {
        id: line.id.into(),
        name: line.name.into(),
        amount: line.amount.into(),
        until: line.until.into(),
        detail: line.detail.into(),
    }
}

fn timeline_model(item: logic::TimelineItem) -> TimelineRow {
    TimelineRow {
        header: item.header,
        label: item.label.into(),
        price: item.price.into(),
    }
}

fn category_model(item: logic::CategoryShare) -> CategoryRow {
    CategoryRow {
        name: item.name.into(),
        amount: item.amount.into(),
        share: item.share,
        swatch: paint(item.rgb),
    }
}

fn category_choices(settings: &logic::Settings) -> ModelRc<Choice> {
    let choices = logic::category_names(settings)
        .into_iter()
        .map(|name| {
            let swatch = paint(logic::category_rgb(&name, settings));
            Choice { label: name.into(), swatch }
        })
        .collect::<Vec<_>>();
    as_model(choices)
}

fn paint(rgb: (u8, u8, u8)) -> slint::Color {
    slint::Color::from_rgb_u8(rgb.0, rgb.1, rgb.2)
}

fn as_model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    ModelRc::from(Rc::new(VecModel::from(rows)))
}
