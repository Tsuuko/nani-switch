use std::{
    collections::HashMap,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage, WM_QUIT,
};

use crate::{
    db, dialog, logger,
    model::{Accounts, CurrentSnapshot, Usage},
    nani, paths, startup, store, usage,
};

const USAGE_REFRESH: Duration = Duration::from_secs(5 * 60);
const CURRENT_REFRESH: Duration = Duration::from_secs(15);

#[derive(Clone)]
enum Action {
    Switch(String),
    Delete(String),
    RefreshUsage,
    SaveCurrent,
    OpenAccounts,
    OpenNani,
    TogglePeriodicUsageRefresh,
    ToggleStartup,
    Exit,
}

#[derive(Clone, Copy, PartialEq)]
enum UsageStatus {
    Loading,
    Refreshing,
    Ready,
    Error,
    Stale,
}

#[derive(Clone)]
struct UsageState {
    status: UsageStatus,
    usage: Option<Usage>,
    error: Option<String>,
}

enum WorkerResult {
    Usage(Vec<(String, Result<Usage, String>)>),
    Switch {
        name: String,
        result: Result<(), String>,
    },
    Save(Result<(String, bool), String>),
}

struct State {
    accounts: Accounts,
    current_snapshot: Option<CurrentSnapshot>,
    current_name: Option<String>,
    usage: HashMap<String, UsageState>,
    usage_refreshing: bool,
    periodic_usage_refresh: bool,
    busy: bool,
    last_usage_finished: Option<Instant>,
    last_usage_started: Instant,
    last_current_refresh: Instant,
}

impl State {
    fn load() -> Self {
        let accounts = store::read_accounts();
        let current_snapshot = db::read_current_snapshot().ok().flatten();
        let current_name = store::find_current_account_name(&accounts, current_snapshot.as_ref());
        Self {
            accounts,
            current_snapshot,
            current_name,
            usage: HashMap::new(),
            usage_refreshing: false,
            periodic_usage_refresh: store::periodic_usage_refresh_enabled(),
            busy: false,
            last_usage_finished: None,
            last_usage_started: Instant::now() - USAGE_REFRESH,
            last_current_refresh: Instant::now(),
        }
    }

    fn reload_accounts_and_current(&mut self) {
        self.accounts = store::read_accounts();
        match db::read_current_snapshot() {
            Ok(snapshot) => self.current_snapshot = snapshot,
            Err(error) => {
                logger::error(format!("Failed to read current account: {error:#}"));
                self.current_snapshot = None;
            }
        }
        self.current_name =
            store::find_current_account_name(&self.accounts, self.current_snapshot.as_ref());
        self.last_current_refresh = Instant::now();
    }
}

fn load_icon() -> Result<Icon> {
    let image = image::load_from_memory(include_bytes!("../assets/tray.png"))?.into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).context("tray icon is invalid")
}

fn number(value: f64) -> String {
    let raw = (value.max(0.0).floor() as u64).to_string();
    let mut output = String::with_capacity(raw.len() + raw.len() / 3);
    for (index, ch) in raw.chars().enumerate() {
        if index > 0 && (raw.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(ch);
    }
    output
}

fn usage_summary(state: Option<&UsageState>) -> String {
    let Some(state) = state else {
        return "loading usage...".into();
    };
    if state.status == UsageStatus::Loading {
        return "loading usage...".into();
    }
    if state.status == UsageStatus::Error {
        return format!(
            "usage error ({})",
            state.error.as_deref().unwrap_or("unknown error")
        );
    }
    let Some(usage) = state.usage.as_ref() else {
        return "usage unavailable".into();
    };
    let suffix = match state.status {
        UsageStatus::Stale => " - stale",
        UsageStatus::Refreshing => " - refreshing",
        _ => "",
    };
    format!(
        "{} tokens, {} calls left{suffix}",
        number(usage.tokens_left()),
        number(usage.calls_left())
    )
}

fn append(menu: &Menu, item: &impl tray_icon::menu::IsMenuItem) -> Result<()> {
    menu.append(item).map_err(Into::into)
}

fn append_submenu(submenu: &Submenu, item: &impl tray_icon::menu::IsMenuItem) -> Result<()> {
    submenu.append(item).map_err(Into::into)
}

fn add_action(
    menu: &Menu,
    actions: &mut HashMap<String, Action>,
    id: &str,
    label: &str,
    enabled: bool,
    action: Action,
) -> Result<()> {
    append(menu, &MenuItem::with_id(id, label, enabled, None))?;
    actions.insert(id.into(), action);
    Ok(())
}

fn build_menu(state: &State) -> Result<(Menu, HashMap<String, Action>)> {
    let menu = Menu::new();
    let mut actions = HashMap::new();
    append(&menu, &MenuItem::new("Nani Switch", false, None))?;
    let current = if let Some(name) = &state.current_name {
        format!("Current: {name}")
    } else if state.current_snapshot.is_some() {
        "Current account is not saved".into()
    } else {
        "Nani is not signed in".into()
    };
    append(&menu, &MenuItem::new(current, false, None))?;
    append(&menu, &PredefinedMenuItem::separator())?;

    if state.accounts.is_empty() {
        append(&menu, &MenuItem::new("No saved accounts", false, None))?;
    }
    for (index, name) in state.accounts.keys().enumerate() {
        let is_current = state.current_name.as_deref() == Some(name);
        let submenu = Submenu::with_id(
            format!("account-{index}"),
            format!(
                "{}{} - {}",
                if is_current { "✓ " } else { "    " },
                name,
                usage_summary(state.usage.get(name))
            ),
            true,
        );
        let usage_state = state.usage.get(name);
        if let Some(usage) = usage_state.and_then(|entry| entry.usage.as_ref()) {
            append_submenu(
                &submenu,
                &MenuItem::new(
                    format!(
                        "Tokens: {} left ({} / {})",
                        number(usage.tokens_left()),
                        number(usage.monthly_usage.tokens),
                        number(usage.max_tokens_total())
                    ),
                    false,
                    None,
                ),
            )?;
            append_submenu(
                &submenu,
                &MenuItem::new(
                    format!(
                        "Calls: {} left ({} / {})",
                        number(usage.calls_left()),
                        number(usage.monthly_usage.calls),
                        number(usage.max_calls)
                    ),
                    false,
                    None,
                ),
            )?;
            if let Some(reset) = &usage.time_until_reset {
                append_submenu(
                    &submenu,
                    &MenuItem::new(
                        format!("Reset in: {}d {}h", number(reset.days), number(reset.hours)),
                        false,
                        None,
                    ),
                )?;
            }
            if usage_state.is_some_and(|entry| entry.status == UsageStatus::Stale) {
                append_submenu(
                    &submenu,
                    &MenuItem::new(
                        format!(
                            "Last refresh failed: {}",
                            usage_state
                                .and_then(|entry| entry.error.as_deref())
                                .unwrap_or("unknown error")
                        ),
                        false,
                        None,
                    ),
                )?;
            }
        } else if usage_state.is_some_and(|entry| entry.status == UsageStatus::Error) {
            append_submenu(&submenu, &MenuItem::new("Usage unavailable", false, None))?;
            append_submenu(
                &submenu,
                &MenuItem::new(
                    usage_state
                        .and_then(|entry| entry.error.as_deref())
                        .unwrap_or("unknown error"),
                    false,
                    None,
                ),
            )?;
        } else {
            append_submenu(&submenu, &MenuItem::new("Loading usage...", false, None))?;
        }
        append_submenu(&submenu, &PredefinedMenuItem::separator())?;
        let switch_id = format!("switch-{index}");
        append_submenu(
            &submenu,
            &MenuItem::with_id(
                &switch_id,
                if is_current {
                    "Current account".into()
                } else {
                    format!("Switch to {name}")
                },
                !is_current && !state.busy,
                None,
            ),
        )?;
        actions.insert(switch_id, Action::Switch(name.clone()));
        let delete_id = format!("delete-{index}");
        append_submenu(
            &submenu,
            &MenuItem::with_id(&delete_id, "Remove saved account", !state.busy, None),
        )?;
        actions.insert(delete_id, Action::Delete(name.clone()));
        append(&menu, &submenu)?;
    }

    append(&menu, &PredefinedMenuItem::separator())?;
    let usage_label = if state.usage_refreshing {
        "Refreshing usage...".into()
    } else if let Some(finished) = state.last_usage_finished {
        let seconds = finished.elapsed().as_secs();
        if seconds < 60 {
            "Usage updated: just now".into()
        } else {
            format!("Usage updated: {} min ago", seconds / 60)
        }
    } else {
        "Usage has not been refreshed".into()
    };
    append(&menu, &MenuItem::new(usage_label, false, None))?;
    add_action(
        &menu,
        &mut actions,
        "refresh",
        "Refresh usage",
        !state.accounts.is_empty() && !state.usage_refreshing,
        Action::RefreshUsage,
    )?;
    add_action(
        &menu,
        &mut actions,
        "save",
        "Save / update current login",
        !state.busy,
        Action::SaveCurrent,
    )?;
    add_action(
        &menu,
        &mut actions,
        "folder",
        "Open accounts folder",
        true,
        Action::OpenAccounts,
    )?;
    add_action(
        &menu,
        &mut actions,
        "nani",
        "Open Nani",
        true,
        Action::OpenNani,
    )?;
    append(&menu, &PredefinedMenuItem::separator())?;
    let periodic_refresh = CheckMenuItem::with_id(
        "periodic-refresh",
        "Refresh usage every 5 minutes",
        true,
        state.periodic_usage_refresh,
        None,
    );
    append(&menu, &periodic_refresh)?;
    actions.insert(
        "periodic-refresh".into(),
        Action::TogglePeriodicUsageRefresh,
    );
    let startup = CheckMenuItem::with_id(
        "startup",
        "Start with Windows",
        true,
        startup::is_enabled(),
        None,
    );
    append(&menu, &startup)?;
    actions.insert("startup".into(), Action::ToggleStartup);
    add_action(
        &menu,
        &mut actions,
        "exit",
        "Exit Nani Switch",
        true,
        Action::Exit,
    )?;
    Ok((menu, actions))
}

fn rebuild_menu(tray: &TrayIcon, state: &State) -> Result<HashMap<String, Action>> {
    let (menu, actions) = build_menu(state)?;
    tray.set_menu(Some(Box::new(menu)));
    Ok(actions)
}

fn start_usage_refresh(state: &mut State, sender: &Sender<WorkerResult>) {
    if state.usage_refreshing || state.accounts.is_empty() {
        return;
    }
    let names: std::collections::HashSet<_> = state.accounts.keys().cloned().collect();
    state.usage.retain(|name, _| names.contains(name));
    for name in state.accounts.keys() {
        let previous = state.usage.get(name).cloned();
        state.usage.insert(
            name.clone(),
            UsageState {
                status: if previous
                    .as_ref()
                    .and_then(|entry| entry.usage.as_ref())
                    .is_some()
                {
                    UsageStatus::Refreshing
                } else {
                    UsageStatus::Loading
                },
                usage: previous.as_ref().and_then(|entry| entry.usage.clone()),
                error: None,
            },
        );
    }
    state.usage_refreshing = true;
    state.last_usage_started = Instant::now();
    let accounts = state.accounts.clone();
    let sender = sender.clone();
    thread::spawn(move || {
        let _ = sender.send(WorkerResult::Usage(usage::fetch_all(accounts)));
    });
}

fn apply_worker_result(state: &mut State, result: WorkerResult, sender: &Sender<WorkerResult>) {
    match result {
        WorkerResult::Usage(results) => {
            let mut failed = 0;
            for (name, result) in results {
                match result {
                    Ok(usage) => {
                        state.usage.insert(
                            name,
                            UsageState {
                                status: UsageStatus::Ready,
                                usage: Some(usage),
                                error: None,
                            },
                        );
                    }
                    Err(error) => {
                        failed += 1;
                        let previous = state.usage.get(&name).and_then(|entry| entry.usage.clone());
                        logger::error(format!("Usage refresh failed for {name}: {error}"));
                        state.usage.insert(
                            name,
                            UsageState {
                                status: if previous.is_some() {
                                    UsageStatus::Stale
                                } else {
                                    UsageStatus::Error
                                },
                                usage: previous,
                                error: Some(error),
                            },
                        );
                    }
                }
            }
            state.usage_refreshing = false;
            state.last_usage_finished = Some(Instant::now());
            logger::info(format!("Usage refresh finished with {failed} failure(s)"));
        }
        WorkerResult::Switch { name, result } => {
            state.busy = false;
            match result {
                Ok(()) => logger::info(format!("Switched to account {name}")),
                Err(error) => {
                    logger::error(format!("Failed to switch to {name}: {error}"));
                    dialog::error("Account switch failed", &error);
                }
            }
            thread::sleep(Duration::from_millis(500));
            state.reload_accounts_and_current();
        }
        WorkerResult::Save(result) => {
            state.busy = false;
            match result {
                Ok((name, updated)) => dialog::info(
                    "Nani account saved",
                    &if updated {
                        format!("Updated {name}.")
                    } else {
                        format!("Saved as {name}.")
                    },
                ),
                Err(error) => {
                    logger::error(format!("Could not save account: {error}"));
                    dialog::error("Could not save account", &error);
                }
            }
            state.reload_accounts_and_current();
            start_usage_refresh(state, sender);
        }
    }
}

fn handle_action(action: Action, state: &mut State, sender: &Sender<WorkerResult>) -> bool {
    match action {
        Action::Switch(name) => {
            if !state.busy && state.current_name.as_deref() != Some(&name) {
                state.busy = true;
                let sender = sender.clone();
                thread::spawn(move || {
                    let result =
                        nani::switch_to_account(&name).map_err(|error| format!("{error:#}"));
                    let _ = sender.send(WorkerResult::Switch { name, result });
                });
            }
        }
        Action::Delete(name) => {
            if !state.busy && dialog::confirm_delete(&name) {
                if let Err(error) = store::delete_account(&name) {
                    dialog::error("Could not remove account", &format!("{error:#}"));
                }
                state.reload_accounts_and_current();
                start_usage_refresh(state, sender);
            }
        }
        Action::RefreshUsage => start_usage_refresh(state, sender),
        Action::SaveCurrent => {
            if !state.busy {
                state.busy = true;
                let sender = sender.clone();
                thread::spawn(move || {
                    let result =
                        store::save_current_account().map_err(|error| format!("{error:#}"));
                    let _ = sender.send(WorkerResult::Save(result));
                });
            }
        }
        Action::OpenAccounts => {
            let _ = std::fs::create_dir_all(paths::store_dir());
            let _ = Command::new("explorer.exe")
                .arg(paths::store_dir())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        }
        Action::OpenNani => {
            if let Err(error) = nani::launch() {
                dialog::error("Could not open Nani", &format!("{error:#}"));
            }
        }
        Action::TogglePeriodicUsageRefresh => {
            let enabled = !state.periodic_usage_refresh;
            match store::set_periodic_usage_refresh(enabled) {
                Ok(()) => {
                    state.periodic_usage_refresh = enabled;
                    if enabled && state.last_usage_started.elapsed() >= USAGE_REFRESH {
                        start_usage_refresh(state, sender);
                    }
                }
                Err(error) => {
                    logger::error(format!("Usage refresh setting failed: {error:#}"));
                    dialog::error("Usage refresh setting failed", &format!("{error:#}"));
                }
            }
        }
        Action::ToggleStartup => {
            let enabled = !startup::is_enabled();
            if let Err(error) = startup::set_enabled(enabled) {
                logger::error(format!("Startup setting failed: {error:#}"));
                dialog::error("Startup setting failed", &format!("{error:#}"));
            }
        }
        Action::Exit => return false,
    }
    true
}

pub fn run() -> Result<()> {
    let mut state = State::load();
    logger::info(format!(
        "Loaded {} account(s); current saved account matched: {}",
        state.accounts.len(),
        state.current_name.is_some()
    ));
    let (menu, _) = build_menu(&state)?;
    let tray = TrayIconBuilder::new()
        .with_tooltip("Nani Switch")
        .with_icon(load_icon()?)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(true)
        .build()
        .context("could not create the tray icon")?;
    let (sender, receiver): (Sender<WorkerResult>, Receiver<WorkerResult>) = mpsc::channel();
    start_usage_refresh(&mut state, &sender);
    let mut actions = rebuild_menu(&tray, &state)?;
    logger::info("Nani Switch started");

    let mut running = true;
    while running {
        unsafe {
            let mut message: MSG = std::mem::zeroed();
            while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if message.message == WM_QUIT {
                    running = false;
                    break;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        let mut changed = false;
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if let Some(action) = actions.get(event.id.as_ref()).cloned() {
                running = handle_action(action, &mut state, &sender);
                changed = true;
            }
        }
        while let Ok(result) = receiver.try_recv() {
            apply_worker_result(&mut state, result, &sender);
            changed = true;
        }
        if state.last_current_refresh.elapsed() >= CURRENT_REFRESH && !state.busy {
            state.reload_accounts_and_current();
            changed = true;
        }
        if state.periodic_usage_refresh && state.last_usage_started.elapsed() >= USAGE_REFRESH {
            start_usage_refresh(&mut state, &sender);
            changed = true;
        }
        if changed && running {
            actions = rebuild_menu(&tray, &state)?;
        }
        thread::sleep(Duration::from_millis(40));
    }
    logger::info("Nani Switch exited");
    Ok(())
}
