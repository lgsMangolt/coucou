// WhatsApp in the island (experimental), the Windows and Linux port of the Mac's WhatsAppController.swift.
//
// Talks to the local gateway the user runs (GOWA, which Coucou can install and start itself, or
// OpenWA), receives new messages on a webhook bound to 127.0.0.1, falls back to polling every 20 s
// when no webhook comes, and keeps the conversations in memory only: nothing of a message is ever
// written to disk or to a log, and everything is forgotten when Coucou quits or the integration is
// turned off. Runs only while the integration is on and its pill is active: otherwise there is no
// timer, no listener and no request at all.
//
// The rules (inbox, filters, signatures, limits) are in model.rs and gateway.rs, the HTTP calls in
// provider.rs, the listener in webhook.rs, GOWA's install and process in install.rs and process.rs.
// The island draws what `IslandView` says (src/views/whatsapp.ts), Settings what `SettingsView`
// says (src/settings/whatsapp.ts); both only ever see status codes, which they translate.

pub mod archive;
pub mod gateway;
pub mod install;
pub mod ledger;
pub mod model;
pub mod orders;
pub mod prefs;
pub mod process;
pub mod provider;
pub mod suggest;
pub mod webhook;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewWindow};

use gateway::{Kind, Link, WebhookEvent};
use install::Step;
use model::{Conversation, Filters, Inbox, Message, Outcome, RateLimit, Refusal};
use prefs::Prefs;
use provider::{Provider, WaError};
use webhook::Listener;

/// This module is compiled twice (lib.rs): as `whatsapp`, the first account, and, from this same
/// file, as `whatsapp2`, the second one. The copy knows which it is from its own path, and
/// everything that must not be shared (the pill, the keystore entries, the events, the files, the
/// ports, the gateway's folder) is told apart by it. The state is separate because the statics are.
const fn is_second_account() -> bool {
    let path = module_path!().as_bytes();
    let needle = b"whatsapp2";
    let mut i = 0;
    while i + needle.len() <= path.len() {
        let mut j = 0;
        while j < needle.len() && path[i + j] == needle[j] {
            j += 1;
        }
        if j == needle.len() {
            return true;
        }
        i += 1;
    }
    false
}

/// True in the second account's copy.
pub const SECOND: bool = is_second_account();

pub const PILL_ID: &str = if SECOND { "integration_whatsapp2" } else { "integration_whatsapp" };
const WEBHOOK_PATH: &str = "/whatsapp";
/// Credential store entries, Rust-only (secrets.rs INTERNAL_KEYS): the webview never reads them.
pub const URL_KEY: &str = if SECOND { "whatsapp-2-url" } else { "whatsapp-url" };
pub const CREDENTIAL_KEY: &str = if SECOND { "whatsapp-2-api-key" } else { "whatsapp-api-key" };
pub const SECRET_KEY: &str = if SECOND { "whatsapp-2-webhook-secret" } else { "whatsapp-webhook-secret" };

const ISLAND_EVENT: &str = if SECOND { "whatsapp2" } else { "whatsapp" };
const PILL_EVENT: &str = if SECOND { "whatsapp2-pill" } else { "whatsapp-pill" };
const SETTINGS_EVENT: &str = if SECOND { "whatsapp2-settings" } else { "whatsapp-settings" };

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum LinkState {
    Off,
    /// The gateway's address can't be used.
    BadUrl,
    Connecting,
    /// The gateway runs, WhatsApp isn't linked: the QR is shown in Settings.
    NeedsQr,
    Connected,
    Unreachable,
    Unauthorized,
    /// The gateway's own error message.
    Failed { detail: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum WebhookState {
    Off,
    /// Listening on the port, not yet accepted by the gateway.
    Listening,
    /// The gateway accepted the address: messages come on their own.
    Registered,
    /// The gateway refused it (its message): polling instead.
    Refused { detail: String },
    /// The port is taken.
    PortBusy { port: u16 },
}

/// Why the last reply didn't go (shown under the field until the next try).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum SendError {
    TooLong,
    TooFast,
    Unreachable,
    Unauthorized,
    NeedsQr,
    Refused { detail: String },
}

/// What happens to the pill (src/island/whatsapp.ts).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum PillEvent {
    /// A new message reaches you: only the conversation's name, never the text.
    Message { name: String },
    /// Nothing unread is left: the badge goes.
    Read,
    /// Turned off: the pill goes back to idle.
    Reset,
}

/// What the island shows.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IslandView {
    pub on: bool,
    pub link: LinkState,
    /// Newest first, filters applied.
    pub conversations: Vec<Conversation>,
    pub unread: u32,
    pub sending: bool,
    pub send_error: Option<SendError>,
    /// The gateway failed twice in a row: a small red dot on the pill.
    pub down: bool,
    /// Notes on money waiting for the click that saves them to the spreadsheet (ledger.rs).
    pub pending: Vec<ledger::EntryView>,
    /// Print orders read from customers' messages, each waiting for a click (orders.rs).
    pub orders: Vec<orders::OrderView>,
}

/// A conversation the notes can be read from.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatChoice {
    pub id: String,
    pub name: String,
    pub is_group: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoredChat {
    pub id: String,
    pub name: String,
}

/// What Settings shows.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub prefs: Prefs,
    /// The address typed in Settings, "" for the gateway's default.
    pub url: String,
    pub default_url: String,
    pub has_credential: bool,
    pub link: LinkState,
    pub webhook: WebhookState,
    /// A `data:image/png;base64,…` URL while WhatsApp waits for the scan.
    pub qr: Option<String>,
    /// This account's number once linked.
    pub me: Option<String>,
    pub install: Step,
    /// A GOWA binary Coucou can start (the one it installed, or one the user gave).
    pub installed: bool,
    /// Where GOWA keeps its WhatsApp session (and Coucou the binary it installed).
    pub gowa_dir: String,
    pub pill_active: bool,
    pub ignored: Vec<IgnoredChat>,
    /// Where the spreadsheet of money notes is (or will be).
    pub ledger_file: String,
    /// The conversations Coucou has seen, groups first: where the notes can be read from.
    pub chats: Vec<ChatChoice>,
    /// A code for the Google Sheet is stored (it is never handed back).
    pub ledger_sheet_code: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "result", rename_all = "camelCase")]
pub enum TestResult {
    Connected { who: String },
    NeedsQr,
    Connecting,
    BadUrl,
    Unreachable,
    Unauthorized,
    Refused { detail: String },
}

struct Ctl {
    prefs: Prefs,
    url: String,
    has_credential: bool,
    pill_active: bool,
    provider: Option<Arc<Provider>>,
    /// Bumped at each start and stop: an answer from before a restart is dropped.
    generation: u64,
    link: LinkState,
    webhook: WebhookState,
    secret: String,
    qr: Option<String>,
    qr_expiry: Option<Instant>,
    me: Option<String>,
    inbox: Inbox,
    listener: Option<Listener>,
    tick: u64,
    refreshing: bool,
    polling: bool,
    backfilled: bool,
    last_webhook: Option<Instant>,
    last_poll_ms: i64,
    naming: HashSet<String>,
    /// Group chat → how many people it has (None: the gateway did not say). Memory only.
    group_sizes: HashMap<String, Option<u32>>,
    /// Groups whose size was already asked: it is asked once.
    sizing: HashSet<String>,
    /// The conversation open in the card: what arrives there is seen, so it isn't unread.
    open_chat: Option<String>,
    rate: RateLimit,
    /// Failed checks in a row (the gateway down, refusing, unreachable).
    failures: u32,
    down: bool,
    sending: bool,
    send_error: Option<SendError>,
    install: Step,
    /// What you wrote to yourself that may be a note on money, waiting to be read (ledger.rs).
    ledger_queue: Vec<String>,
    /// Read notes waiting for the click, in memory only: nothing is written before it.
    ledger_pending: Vec<(u64, ledger::Entry)>,
    ledger_next: u64,
    ledger_sheet_code: bool,
    /// Chats whose last messages may hold a print order, waiting to be read (orders.rs).
    order_queue: Vec<String>,
    /// Chats being read right now: one reading at a time for each.
    order_reading: HashSet<String>,
    /// The orders on the card, in memory only.
    orders: Vec<(u64, orders::Order)>,
    order_next: u64,
    /// Chat → the file of the order that was discarded or finished: it is not read again.
    order_settled: HashMap<String, String>,
    /// Customers whose "yes" just approved a quote: the pill says so.
    order_news: Vec<String>,
}

static CTL: LazyLock<Mutex<Ctl>> = LazyLock::new(|| {
    Mutex::new(Ctl {
        prefs: Prefs::default(),
        url: String::new(),
        has_credential: false,
        pill_active: false,
        provider: None,
        generation: 0,
        link: LinkState::Off,
        webhook: WebhookState::Off,
        secret: String::new(),
        qr: None,
        qr_expiry: None,
        me: None,
        inbox: Inbox::default(),
        listener: None,
        tick: 0,
        refreshing: false,
        polling: false,
        backfilled: false,
        last_webhook: None,
        last_poll_ms: 0,
        naming: HashSet::new(),
        group_sizes: HashMap::new(),
        sizing: HashSet::new(),
        open_chat: None,
        rate: RateLimit::default(),
        failures: 0,
        down: false,
        sending: false,
        send_error: None,
        install: Step::Idle,
        ledger_queue: Vec::new(),
        ledger_pending: Vec::new(),
        ledger_next: 0,
        ledger_sheet_code: false,
        order_queue: Vec::new(),
        order_reading: HashSet::new(),
        orders: Vec::new(),
        order_next: 0,
        order_settled: HashMap::new(),
        order_news: Vec::new(),
    })
});

static APP: OnceLock<AppHandle> = OnceLock::new();

fn ctl() -> MutexGuard<'static, Ctl> {
    CTL.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Ctl {
    fn kind(&self) -> Kind {
        Kind::from_name(&self.prefs.gateway)
    }

    fn should_run(&self) -> bool {
        self.prefs.enabled && self.pill_active
    }

    fn chat_choices(&self) -> Vec<ChatChoice> {
        let mut list: Vec<ChatChoice> = self
            .inbox
            .conversations
            .values()
            .map(|c| ChatChoice { id: c.id.clone(), name: c.name.clone(), is_group: c.is_group })
            .collect();
        let chosen = model::normalized(&self.prefs.ledger_chat);
        if !self.prefs.ledger_chat.is_empty() && !list.iter().any(|c| c.id == chosen) {
            list.push(ChatChoice { id: chosen.clone(), name: model::display_number(&chosen), is_group: model::is_group(&chosen) });
        }
        list.sort_by(|a, b| b.is_group.cmp(&a.is_group).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        list
    }

    fn filters(&self) -> Filters {
        let mut ignored: std::collections::BTreeSet<String> = self.prefs.ignored.iter().cloned().collect();
        // Groups bigger than the limit are ignored like the chats the user ignored by hand.
        let limit = self.prefs.ignore_groups_over;
        if limit > 0 {
            ignored.extend(self.group_sizes.iter().filter(|(_, size)| size.is_some_and(|n| n > limit)).map(|(chat, _)| chat.clone()));
        }
        Filters { mute_groups: self.prefs.mute_groups, ignored }
    }

    fn effective_url(&self) -> String {
        if self.url.is_empty() { self.kind().default_url().to_string() } else { self.url.clone() }
    }

    fn webhook_alive(&self) -> bool {
        self.last_webhook.is_some_and(|t| t.elapsed() < Duration::from_secs(300))
    }

    fn island_view(&self) -> IslandView {
        let filters = self.filters();
        IslandView {
            on: self.provider.is_some(),
            link: self.link.clone(),
            conversations: self.inbox.visible(&filters).into_iter().cloned().collect(),
            unread: self.inbox.visible_unread(&filters),
            sending: self.sending,
            send_error: self.send_error.clone(),
            down: self.down,
            pending: self.ledger_pending.iter().map(|(id, entry)| entry.view(*id)).collect(),
            orders: self.orders.iter().map(|(id, order)| order.view(*id)).collect(),
        }
    }

    fn settings_view(&self) -> SettingsView {
        let path = self.prefs.gowa_path.trim();
        SettingsView {
            prefs: self.prefs.clone(),
            url: self.url.clone(),
            default_url: self.kind().default_url().to_string(),
            has_credential: self.has_credential,
            link: self.link.clone(),
            webhook: self.webhook.clone(),
            qr: self.qr.clone(),
            me: self.me.as_deref().map(model::display_number),
            install: self.install.clone(),
            installed: !path.is_empty() && process::is_runnable(std::path::Path::new(path)),
            gowa_dir: process::working_dir().to_string_lossy().into_owned(),
            pill_active: self.pill_active,
            ignored: self
                .prefs
                .ignored
                .iter()
                .map(|id| IgnoredChat {
                    id: id.clone(),
                    name: self.inbox.conversations.get(id).map(|c| c.name.clone()).unwrap_or_else(|| model::display_number(id)),
                })
                .collect(),
            ledger_file: ledger::file(&self.prefs.ledger_dir).to_string_lossy().into_owned(),
            chats: self.chat_choices(),
            ledger_sheet_code: self.ledger_sheet_code,
        }
    }
}

// ── Telling the pages ─────────────────────────────────────────────────────────

/// Both pages redraw from what changed. Never called with the lock held.
fn changed() {
    let Some(app) = APP.get() else { return };
    let (island, settings) = {
        let c = ctl();
        (c.island_view(), c.settings_view())
    };
    let _ = app.emit_to(crate::island::WINDOW_LABEL, ISLAND_EVENT, island);
    let _ = app.emit_to(crate::HUB_LABEL, SETTINGS_EVENT, settings);
}

fn pill(event: PillEvent) {
    if let Some(app) = APP.get() {
        let _ = app.emit_to(crate::island::WINDOW_LABEL, PILL_EVENT, event);
    }
}

fn spawn<F: std::future::Future<Output = ()> + Send + 'static>(f: F) {
    tauri::async_runtime::spawn(f);
}

/// GOWA started or stopped to match Settings, off the async threads (stopping it waits a second).
fn sync_gowa(c: &Ctl) {
    let wanted = c.prefs.auto_start && c.kind() == Kind::Gowa;
    let (binary, url) = (c.prefs.gowa_path.clone(), c.effective_url());
    let credential = if wanted { crate::secrets::get_internal(CREDENTIAL_KEY).unwrap_or_default() } else { String::new() };
    tauri::async_runtime::spawn_blocking(move || {
        process::sync(wanted, &binary, &url, &credential);
    });
}

/// The webhook's signing secret, made once (random, 64 hex characters), kept in the credential store.
fn webhook_secret() -> String {
    if let Some(s) = crate::secrets::get_internal(SECRET_KEY).filter(|s| s.len() >= 16) {
        return s;
    }
    use ring::rand::{SecureRandom, SystemRandom};
    let mut bytes = [0u8; 32];
    if SystemRandom::new().fill(&mut bytes).is_err() {
        return String::new(); // no secret, no webhook: polling only
    }
    let s = gateway::hex(&bytes);
    let _ = crate::secrets::set_internal(SECRET_KEY, &s);
    s
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

/// Called once at launch.
pub fn start(app: &AppHandle, active_integrations: &[String]) {
    let _ = APP.set(app.clone());
    {
        let mut c = ctl();
        c.prefs = prefs::load();
        c.url = crate::secrets::get_internal(URL_KEY).unwrap_or_default();
        c.has_credential = crate::secrets::get_internal(CREDENTIAL_KEY).is_some();
        c.ledger_sheet_code = crate::secrets::get_internal(ledger::SHEET_KEY).is_some();
        c.pill_active = active_integrations.iter().any(|x| x == PILL_ID);
    }
    if !SECOND {
        orders::ensure_file();
    }
    update();
}

/// Settings were saved: the pill turned on or off from anywhere is followed.
pub fn sync(active_integrations: &[String]) {
    let active = active_integrations.iter().any(|x| x == PILL_ID);
    let moved = {
        let mut c = ctl();
        let moved = c.pill_active != active;
        c.pill_active = active;
        moved
    };
    if moved {
        update();
    }
}

/// Coucou quits: GOWA goes with it, and so does everything in memory.
pub fn shutdown() {
    {
        let mut c = ctl();
        c.generation += 1;
        if let Some(l) = c.listener.take() {
            l.stop();
        }
        c.provider = None;
        c.inbox.clear();
    }
    process::stop_now();
}

fn update() {
    let (run, running) = {
        let c = ctl();
        (c.should_run(), c.provider.is_some())
    };
    if run && !running {
        begin();
    } else if !run && running {
        stop(true);
        changed();
    } else {
        changed();
    }
}

/// Settings changed (on/off, gateway, URL, credentials, port): start over.
fn settings_changed() {
    let forget = !ctl().should_run();
    stop(forget);
    update();
}

fn begin() {
    let secret = webhook_secret();
    let (gen, port, kind) = {
        let mut c = ctl();
        let kind = c.kind();
        let Some(base) = provider::parse_base(&c.effective_url()) else {
            c.link = LinkState::BadUrl;
            drop(c);
            changed();
            return;
        };
        let credential = crate::secrets::get_internal(CREDENTIAL_KEY);
        let Ok(provider) = Provider::new(kind, base, credential) else {
            c.link = LinkState::Unreachable;
            drop(c);
            changed();
            return;
        };
        c.provider = Some(Arc::new(provider));
        c.generation += 1;
        c.link = LinkState::Connecting;
        c.secret = secret.clone();
        c.tick = 0;
        // GOWA started by Coucou when asked to, before the first check.
        sync_gowa(&c);
        (c.generation, c.prefs.port, kind)
    };
    start_listener(gen, port, kind, secret);
    spawn_ticker(gen);
    changed();
    refresh();
}

/// Stops talking to the gateway. `forget`: the conversations go too.
fn stop(forget: bool) {
    let mut c = ctl();
    c.generation += 1;
    c.refreshing = false;
    c.polling = false;
    if let Some(l) = c.listener.take() {
        l.stop();
    }
    c.provider = None;
    c.link = LinkState::Off;
    c.webhook = WebhookState::Off;
    c.qr = None;
    c.qr_expiry = None;
    c.me = None;
    c.backfilled = false;
    c.last_webhook = None;
    c.naming.clear();
    c.failures = 0;
    c.down = false;
    c.sending = false;
    c.send_error = None;
    // Turned off: GOWA goes too. Otherwise begin() keeps it, or restarts it if its settings changed.
    if !c.should_run() {
        tauri::async_runtime::spawn_blocking(process::stop_now);
    }
    if forget {
        c.open_chat = None;
        c.inbox.clear();
        drop(c);
        pill(PillEvent::Reset);
    }
}

fn start_listener(gen: u64, port: u16, kind: Kind, secret: String) {
    let handler: webhook::Handler = Arc::new(move |request| {
        if request.method != "POST" || !request.path.starts_with(WEBHOOK_PATH) {
            return 404;
        }
        // Signed by the gateway with Coucou's secret, over the exact bytes received.
        if !gateway::verify(kind, &request.headers, &request.body, &secret) {
            return 401;
        }
        let body = request.body;
        spawn(async move { received(gen, kind, &body) });
        200
    });
    let listener = Listener::start(port, handler);
    let mut c = ctl();
    if c.generation != gen {
        if let Ok(l) = listener {
            l.stop();
        }
        return;
    }
    match listener {
        Ok(l) => {
            c.listener = Some(l);
            c.webhook = WebhookState::Listening;
        }
        Err(()) => c.webhook = WebhookState::PortBusy { port },
    }
}

fn spawn_ticker(gen: u64) {
    spawn(async move {
        let mut last = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if ctl().generation != gen {
                break;
            }
            // Back from sleep (the 5 s took far longer): nothing is trusted, the gateway may have
            // restarted without Coucou's webhook.
            if last.elapsed() > Duration::from_secs(30) {
                let mut c = ctl();
                if c.webhook == WebhookState::Registered {
                    c.webhook = WebhookState::Listening;
                }
                c.last_webhook = None;
            }
            last = Instant::now();
            on_tick();
        }
    });
}

fn on_tick() {
    // Paused from the tray: nothing reaches the network.
    if crate::integrations::PAUSED.load(Ordering::Relaxed) {
        return;
    }
    let (refresh_now, poll_now) = {
        let mut c = ctl();
        c.tick += 1;
        if c.link != LinkState::Connected {
            // Every 5 s while linking (QR, gateway coming up); every 30 s once the gateway has
            // been down for a minute.
            (!(c.failures > 12 && c.tick % 6 != 0), false)
        } else {
            // Every minute: still linked? Every 20 s: new messages, unless the webhook delivers.
            (c.tick % 12 == 0, c.tick % 4 == 0)
        }
    };
    if refresh_now {
        refresh();
    }
    if poll_now {
        poll();
    }
}

// ── Status and QR ─────────────────────────────────────────────────────────────

fn refresh() {
    let (provider, gen) = {
        let mut c = ctl();
        let Some(p) = c.provider.clone() else { return };
        if c.refreshing {
            return;
        }
        c.refreshing = true;
        (p, c.generation)
    };
    spawn(async move {
        let result = provider.status().await;
        let (mut register, mut backfill_now, mut ask_qr) = (false, false, false);
        {
            let mut c = ctl();
            if c.generation != gen {
                return;
            }
            match result {
                Ok(status) => {
                    if c.failures > 0 {
                        c.failures = 0;
                        c.down = false;
                    }
                    c.me = status.me;
                    match status.link {
                        Link::Connected => {
                            if c.link != LinkState::Connected {
                                c.qr = None;
                            }
                            c.link = LinkState::Connected;
                            register = c.webhook == WebhookState::Listening;
                            if !c.backfilled {
                                c.backfilled = true;
                                backfill_now = true;
                            }
                        }
                        Link::NeedsQr => {
                            c.link = LinkState::NeedsQr;
                            if c.qr.is_none() || c.qr_expiry.is_none_or(|t| Instant::now() > t) {
                                // Not asked again for 15 s even when nothing came: each GOWA login
                                // call restarts the pairing.
                                c.qr_expiry = Some(Instant::now() + Duration::from_secs(15));
                                ask_qr = true;
                            }
                        }
                        Link::Connecting => c.link = LinkState::Connecting,
                    }
                }
                Err(err) => apply(&mut c, err),
            }
        }
        changed();
        if register {
            register_webhook(&provider, gen).await;
        }
        if backfill_now {
            backfill(&provider, gen).await;
        }
        if ask_qr {
            let qr = provider.qr().await;
            let mut c = ctl();
            if c.generation == gen {
                match qr {
                    Ok(Some(qr)) => {
                        c.qr = Some(qr.data_url);
                        c.qr_expiry = Some(Instant::now() + Duration::from_secs(qr.seconds));
                    }
                    Ok(None) => {}
                    Err(err) => apply(&mut c, err),
                }
            }
            drop(c);
            changed();
        }
        let mut c = ctl();
        if c.generation == gen {
            c.refreshing = false;
        }
    });
}

fn apply(c: &mut Ctl, error: WaError) {
    c.link = match &error {
        WaError::Unreachable => LinkState::Unreachable,
        WaError::Unauthorized => LinkState::Unauthorized,
        WaError::NotLoggedIn => LinkState::NeedsQr,
        WaError::Refused(why) => LinkState::Failed { detail: why.clone() },
    };
    if error == WaError::NotLoggedIn {
        return;
    }
    c.failures += 1;
    // Twice in a row (not a gateway still starting): a small red dot on the pill, nothing more.
    if c.failures == 2 {
        c.down = true;
    }
    // GOWA started by Coucou and gone (crash, killed): started again, once a minute at most.
    if error == WaError::Unreachable {
        sync_gowa(c);
    }
}

/// One check from Settings' "Test connection": what the gateway says.
async fn test_connection() -> TestResult {
    // The running connection when there is one (a second one could race it for OpenWA's session).
    let provider = {
        let c = ctl();
        match c.provider.clone() {
            Some(p) => p,
            None => {
                let Some(base) = provider::parse_base(&c.effective_url()) else { return TestResult::BadUrl };
                match Provider::new(c.kind(), base, crate::secrets::get_internal(CREDENTIAL_KEY)) {
                    Ok(p) => Arc::new(p),
                    Err(_) => return TestResult::Unreachable,
                }
            }
        }
    };
    let mut attempt = 0;
    let status = loop {
        match provider.status().await {
            // GOWA just started by Coucou: a few seconds to come up.
            Err(WaError::Unreachable) if process::is_running() && attempt < 8 => {
                attempt += 1;
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            other => break other,
        }
    };
    match status {
        Ok(s) => match s.link {
            Link::Connected => TestResult::Connected { who: s.me.as_deref().map(model::display_number).unwrap_or_default() },
            Link::NeedsQr => TestResult::NeedsQr,
            Link::Connecting => TestResult::Connecting,
        },
        Err(WaError::Unreachable) => TestResult::Unreachable,
        Err(WaError::Unauthorized) => TestResult::Unauthorized,
        Err(WaError::NotLoggedIn) => TestResult::NeedsQr,
        Err(WaError::Refused(detail)) => TestResult::Refused { detail },
    }
}

// ── Webhook ───────────────────────────────────────────────────────────────────

async fn register_webhook(provider: &Provider, gen: u64) {
    let (url, secret) = {
        let c = ctl();
        (format!("http://127.0.0.1:{}{WEBHOOK_PATH}", c.prefs.port), c.secret.clone())
    };
    if secret.is_empty() {
        return;
    }
    let result = provider.register_webhook(&url, &secret).await;
    {
        let mut c = ctl();
        if c.generation != gen {
            return;
        }
        match result {
            Ok(()) => c.webhook = WebhookState::Registered,
            Err(WaError::Refused(why)) => {
                crate::log::line(format!("whatsapp: the gateway refused the webhook: {why}"));
                c.webhook = WebhookState::Refused { detail: why };
            }
            // Tried again on the next minute's check.
            Err(_) => {}
        }
    }
    changed();
}

fn received(gen: u64, kind: Kind, body: &[u8]) {
    let (fresh, groups) = {
        let mut c = ctl();
        if c.generation != gen || c.provider.is_none() {
            return;
        }
        c.last_webhook = Some(Instant::now());
        if c.webhook != WebhookState::Registered {
            c.webhook = WebhookState::Registered;
        }
        let me = c.me.clone();
        let Some(WebhookEvent::Message(message)) = gateway::parse_webhook(kind, body, me.as_deref()) else { return };
        ingest(&mut c, vec![message])
    };
    after_ingest(gen, fresh, groups);
}

// ── Messages ──────────────────────────────────────────────────────────────────

/// The last hours of history, once linked: kept, but nothing counts as new.
async fn backfill(provider: &Arc<Provider>, gen: u64) {
    let me = ctl().me.clone();
    let since = gateway::now_ms() - 12 * 3600 * 1000;
    let Ok(messages) = provider.recent_messages(since, me.as_deref()).await else { return };
    let groups = {
        let mut c = ctl();
        if c.generation != gen {
            return;
        }
        let filters = c.filters();
        // History: kept, never counted as unread, never announced.
        let chats: Vec<String> = messages.iter().map(|m| model::normalized(&m.chat)).collect();
        for message in messages {
            c.inbox.add(message, &filters, None, true);
        }
        let groups = chats.iter().filter_map(|chat| needs_name(&mut c, chat)).collect::<Vec<_>>();
        c.last_poll_ms = gateway::now_ms();
        groups
    };
    name_groups(provider.clone(), gen, groups, true);
    changed();
}

fn poll() {
    let (provider, gen, since, me) = {
        let mut c = ctl();
        let Some(p) = c.provider.clone() else { return };
        if c.polling || c.webhook_alive() || !c.backfilled {
            return;
        }
        c.polling = true;
        // A little overlap; duplicates are dropped.
        (p, c.generation, c.last_poll_ms - 30_000, c.me.clone())
    };
    spawn(async move {
        let started = gateway::now_ms();
        let result = provider.recent_messages(since, me.as_deref()).await;
        let (fresh, groups) = {
            let mut c = ctl();
            if c.generation != gen {
                return;
            }
            c.polling = false;
            let Ok(messages) = result else { return };
            c.last_poll_ms = started;
            ingest(&mut c, messages)
        };
        after_ingest(gen, fresh, groups);
    });
}

/// Adds what arrived. Returns the conversation to announce (its name), and the groups to name.
fn ingest(c: &mut Ctl, messages: Vec<Message>) -> (Option<String>, Vec<String>) {
    let filters = c.filters();
    let mut fresh: Option<String> = None;
    let mut groups = Vec::new();
    for message in messages {
        let key = model::normalized(&message.chat);
        let from_me = message.from_me;
        // Your own message, in the chat chosen for the notes (your own chat, or a group you made for
        // them), that may note money (ledger.rs). Only what you wrote: never anybody else's words.
        let in_notes_chat = if c.prefs.ledger_chat.is_empty() {
            !model::is_group(&key) && c.me.as_deref().is_some_and(|me| model::same_user(me, &key))
        } else {
            key == model::normalized(&c.prefs.ledger_chat)
        };
        let note = (c.prefs.ledger_enabled
            && from_me
            && in_notes_chat
            && ledger::looks_like_a_note(&message.text))
        .then(|| message.text.clone());
        // Big groups are ignored; one whose size is not known yet waits for the answer before it is announced.
        let waiting_for_size = c.prefs.ignore_groups_over > 0 && model::is_group(&key) && !c.group_sizes.contains_key(&key);
        let said = if from_me || model::is_group(&key) { None } else { Some(message.text.clone()) };
        let outcome = c.inbox.add(message, &filters, None, false);
        if let (Some(text), true) = (note, outcome != Outcome::Duplicate) {
            c.ledger_queue.push(text);
        }
        // A customer's new message: the answer to a quote, or maybe a print order (orders.rs).
        if let (Some(text), true) = (said, outcome != Outcome::Duplicate) {
            order_step(c, &key, &text);
        }
        if let Some(g) = needs_name(c, &key) {
            groups.push(g);
        }
        if c.open_chat.as_deref() == Some(key.as_str()) {
            // Read on screen: no unread, no Mochi. The receipt waits for a reply (or the next
            // opening), as Settings says.
            c.inbox.mark_read(&key);
            continue;
        }
        if outcome == Outcome::Notify && !from_me && !waiting_for_size {
            fresh = Some(key);
        }
    }
    let name = fresh.map(|key| c.inbox.conversations.get(&key).map(|conv| conv.name.clone()).unwrap_or_else(|| model::display_number(&key)));
    (name, groups)
}

/// A customer wrote. With a quote waiting, a "yes" approves it; else, a file and words about
/// printing in the last messages queue the conversation to be read as an order.
fn order_step(c: &mut Ctl, key: &str, text: &str) {
    if let Some((_, order)) = c.orders.iter_mut().find(|(_, o)| o.chat == key && o.state == orders::State::Quoted) {
        if orders::approves(text) {
            order.state = orders::State::Approved;
            c.order_news.push(order.name.clone());
        }
        return;
    }
    if c.orders.iter().any(|(_, o)| o.chat == key && o.state == orders::State::Approved) {
        return;
    }
    let Some(conversation) = c.inbox.conversations.get(key) else { return };
    if !orders::worth_reading(conversation) {
        return;
    }
    let Some((file, _)) = orders::file_of(conversation) else { return };
    if c.order_settled.get(key) == Some(&file) || c.order_queue.iter().any(|k| k == key) {
        return;
    }
    c.order_queue.push(key.to_string());
}

/// The last messages of one conversation, read by the model: a print order becomes a card that
/// waits for your click, anything else is forgotten (orders.rs).
async fn read_order(gen: u64, chat: String) {
    let config = tauri::async_runtime::spawn_blocking(orders::load).await.unwrap_or_default();
    if !config.ativo {
        return;
    }
    let (prompt, file, name) = {
        let mut c = ctl();
        if c.generation != gen || c.order_reading.contains(&chat) {
            return;
        }
        let found = c.inbox.conversations.get(&chat).and_then(|conv| Some((orders::prompt(conv)?, orders::file_of(conv)?, conv.name.clone())));
        let Some(found) = found else { return };
        c.order_reading.insert(chat.clone());
        found
    };
    let answer = orders::read_order(&prompt, &config).await;
    let (names, busy) = crate::printers::queue();
    let fresh = {
        let mut c = ctl();
        c.order_reading.remove(&chat);
        if c.generation != gen {
            return;
        }
        match answer {
            Ok(Some(asked)) => {
                let kind = config.item(&asked.item).map(|i| i.impressora.clone()).unwrap_or_default();
                let order = orders::Order {
                    chat: chat.clone(),
                    name,
                    quantity: asked.quantity,
                    lines: orders::price(&asked, &config),
                    note: asked.note.clone(),
                    file,
                    printer: orders::printer(&kind, &config.impressoras, &names, &busy),
                    state: orders::State::Draft,
                    saved: None,
                    payment: config.pagamento.clone(),
                };
                place_order(&mut c, order)
            }
            Ok(None) => false,
            Err(reason) => {
                crate::log::line(format!("whatsapp: could not read a print order: {reason}"));
                false
            }
        }
    };
    if fresh {
        pill(PillEvent::Message { name: crate::i18n::t("New print order") });
    }
    changed();
}

/// Puts a freshly read order on the card, in place of the draft the same customer had. False when
/// nothing changed: the same draft again, or a quote already on its way.
fn place_order(c: &mut Ctl, order: orders::Order) -> bool {
    if c.orders.iter().any(|(_, o)| o.chat == order.chat && o.state != orders::State::Draft) {
        return false;
    }
    if c.orders.iter().any(|(_, o)| *o == order) {
        return false;
    }
    c.orders.retain(|(_, o)| o.chat != order.chat);
    c.order_next += 1;
    let id = c.order_next;
    c.orders.push((id, order));
    if c.orders.len() > orders::MAX_ORDERS {
        c.orders.remove(0);
    }
    true
}

fn after_ingest(gen: u64, fresh: Option<String>, groups: Vec<String>) {
    if let Some(name) = fresh {
        pill(PillEvent::Message { name });
    }
    let (notes, queued, approved) = {
        let mut c = ctl();
        (std::mem::take(&mut c.ledger_queue), std::mem::take(&mut c.order_queue), std::mem::take(&mut c.order_news))
    };
    for text in notes {
        spawn(read_note(gen, text));
    }
    for chat in queued {
        spawn(read_order(gen, chat));
    }
    for name in approved {
        pill(PillEvent::Message { name: crate::i18n::tf("Order approved: {name}", &[("name", name.as_str())]) });
    }
    if let Some(provider) = ctl().provider.clone() {
        name_groups(provider, gen, groups, true);
    }
    changed();
}

/// One message you wrote to yourself, read by the model: a note on money waits for your click,
/// anything else is forgotten (ledger.rs).
async fn read_note(gen: u64, text: String) {
    let today = crate::platform::local_time();
    match ledger::read(&text, (today.year, today.month, today.day)).await {
        Ok(Some(entry)) => {
            {
                let mut c = ctl();
                if c.generation != gen || !c.prefs.ledger_enabled {
                    return;
                }
                c.ledger_next += 1;
                let id = c.ledger_next;
                c.ledger_pending.push((id, entry));
                if c.ledger_pending.len() > 20 {
                    c.ledger_pending.remove(0);
                }
            }
            pill(PillEvent::Message { name: crate::i18n::t("New entry to confirm") });
            changed();
        }
        Ok(None) => {}
        Err(reason) => crate::log::line(format!("whatsapp: could not read a note on money: {reason}")),
    }
}

/// Groups arrive under their id (GOWA's webhook carries no group name): asked once.
fn needs_name(c: &mut Ctl, chat: &str) -> Option<String> {
    if !model::is_group(chat) {
        return None;
    }
    let conv = c.inbox.conversations.get(chat)?;
    let wants_name = conv.name == model::display_number(chat) && !c.naming.contains(chat);
    // How many people it has, only while big groups are ignored.
    let wants_size = c.prefs.ignore_groups_over > 0 && !c.sizing.contains(chat);
    if !wants_name && !wants_size {
        return None;
    }
    if wants_name {
        c.naming.insert(chat.to_string());
    }
    if wants_size {
        c.sizing.insert(chat.to_string());
    }
    Some(chat.to_string())
}

/// Asks the gateway about each group (its name and its size, one call). `announce`: a message held back
/// while the size was unknown is announced now, if the group is allowed.
fn name_groups(provider: Arc<Provider>, gen: u64, groups: Vec<String>, announce: bool) {
    if groups.is_empty() {
        return;
    }
    spawn(async move {
        for chat in groups {
            let info = provider.group_info(&chat).await;
            let late = {
                let mut c = ctl();
                if c.generation != gen {
                    return;
                }
                if let Some(name) = info.as_ref().and_then(|i| i.name.as_deref()) {
                    c.inbox.rename(&chat, name);
                }
                // None when the gateway did not say: a group of unknown size is allowed.
                c.group_sizes.insert(chat.clone(), info.as_ref().and_then(|i| i.size));
                let filters = c.filters();
                let held = announce
                    && c.prefs.ignore_groups_over > 0
                    && c.open_chat.as_deref() != Some(chat.as_str())
                    && c.inbox.visible(&filters).iter().any(|conv| conv.id == chat && conv.unread > 0);
                held.then(|| c.inbox.conversations.get(&chat).map(|conv| conv.name.clone()).unwrap_or_else(|| model::display_number(&chat)))
            };
            changed();
            if let Some(name) = late {
                pill(PillEvent::Message { name });
            }
        }
    });
}

// ── The conversation ──────────────────────────────────────────────────────────

/// Marks the chat read here and, when it had something unread, on WhatsApp too (up to its newest
/// received message). Returns the receipt to send.
fn mark_read(c: &mut Ctl, chat: &str) -> Option<(String, Option<String>)> {
    let key = model::normalized(chat);
    let conv = c.inbox.conversations.get(&key)?;
    let had_unread = conv.unread > 0;
    let last = conv.messages.iter().rev().find(|m| !m.from_me).map(|m| m.id.clone());
    c.inbox.mark_read(&key);
    if c.inbox.visible_unread(&c.filters()) == 0 {
        pill(PillEvent::Read);
    }
    // Nothing new: no receipt to send.
    had_unread.then_some((key, last))
}

/// A conversation opened in the card: its latest messages from the gateway, and the read receipt
/// when Settings says so.
fn open(chat: &str) {
    let (provider, gen, receipt, me) = {
        let mut c = ctl();
        c.send_error = None;
        c.open_chat = Some(model::normalized(chat));
        let Some(p) = c.provider.clone() else { return };
        let receipt = if c.prefs.mark_read_on_open { mark_read(&mut c, chat) } else { None };
        (p, c.generation, receipt, c.me.clone())
    };
    changed();
    let chat = chat.to_string();
    spawn(async move {
        if let Some((key, last)) = receipt {
            let _ = provider.mark_read(&key, last.as_deref()).await;
        }
        let Ok(history) = provider.messages(&chat, 10, me.as_deref()).await else { return };
        {
            let mut c = ctl();
            if c.generation != gen {
                return;
            }
            let filters = c.filters();
            for message in history {
                c.inbox.add(message, &filters, None, true);
            }
        }
        changed();
    });
}

fn close(chat: &str) {
    let mut c = ctl();
    if c.open_chat.as_deref() == Some(model::normalized(chat).as_str()) {
        c.open_chat = None;
    }
}

/// Sends a reply: only to a conversation already in the inbox, one at a time, a few a minute, and
/// only from the user's own Enter or click in the card. True once the gateway took it.
async fn send(chat: &str, text: &str) -> bool {
    let key = model::normalized(chat);
    let (provider, gen, body, me) = {
        let mut c = ctl();
        let Some(p) = c.provider.clone() else { return false };
        if c.sending {
            return false;
        }
        match model::check_reply(text, &key, &c.inbox) {
            Some(Refusal::TooLong) => {
                c.send_error = Some(SendError::TooLong);
                drop(c);
                changed();
                return false;
            }
            Some(_) => return false,
            None => {}
        }
        if !c.rate.allow(Instant::now()) {
            c.send_error = Some(SendError::TooFast);
            drop(c);
            changed();
            return false;
        }
        c.sending = true;
        c.send_error = None;
        (p, c.generation, text.trim().to_string(), c.me.clone())
    };
    changed();
    let result = provider.send(&body, &key).await;
    let receipt = {
        let mut c = ctl();
        // Turned off or restarted meanwhile: the inbox it belonged to is gone.
        if c.generation != gen {
            return result.is_ok();
        }
        c.sending = false;
        match result {
            Ok(id) => {
                let id = id.unwrap_or_else(|| format!("coucou-{}-{}", gateway::now_ms(), c.inbox.conversations.len()));
                let mut mine = Message::new(&id, &key, me.as_deref().unwrap_or(""), &body, gateway::now_ms());
                mine.from_me = true;
                mine.chat_name = c.inbox.conversations.get(&key).map(|conv| conv.name.clone());
                let filters = c.filters();
                c.inbox.add(mine, &filters, None, false);
                // Replying reads the conversation, whatever the setting.
                let last = c.inbox.conversations.get(&key).and_then(|conv| conv.messages.iter().rev().find(|m| !m.from_me)).map(|m| m.id.clone());
                if c.inbox.visible_unread(&filters) == 0 {
                    pill(PillEvent::Read);
                }
                Some(last)
            }
            Err(err) => {
                c.send_error = Some(match err {
                    WaError::Unreachable => SendError::Unreachable,
                    WaError::Unauthorized => SendError::Unauthorized,
                    WaError::NotLoggedIn => SendError::NeedsQr,
                    WaError::Refused(detail) => SendError::Refused { detail },
                });
                None
            }
        }
    };
    changed();
    match receipt {
        Some(last) => {
            let _ = provider.mark_read(&key, last.as_deref()).await;
            true
        }
        None => false,
    }
}

fn ignore(chat: &str) {
    let key = model::normalized(chat);
    {
        let mut c = ctl();
        c.prefs.ignored.push(key.clone());
        c.prefs = c.prefs.clone().cleaned();
        prefs::save(&c.prefs);
        c.inbox.remove(&key);
        if c.open_chat.as_deref() == Some(key.as_str()) {
            c.open_chat = None;
        }
    }
    changed();
}

/// What Settings' "Add" turns into a chat id: a phone number ("+33 6 12…") or a full id ("…@g.us").
pub fn chat_from_entry(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.contains('@') {
        return (raw.len() > 3 && !raw.chars().any(char::is_whitespace)).then(|| raw.to_string());
    }
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    (!digits.is_empty()).then(|| format!("{digits}@s.whatsapp.net"))
}

// ── WhatsApp itself ───────────────────────────────────────────────────────────

/// The card's ↗ when no app on this computer opens `whatsapp:` links.
const WEB_URL: &str = "https://web.whatsapp.com";

/// A person's number, for WhatsApp's `send?phone=` links. None for a group, a chat whose number
/// WhatsApp keeps hidden (`@lid`) or no chat at all: those open WhatsApp as it is.
fn phone_of(chat: Option<&str>) -> Option<String> {
    chat.map(model::normalized)
        .and_then(|jid| jid.strip_suffix("@s.whatsapp.net").map(str::to_string))
        .filter(|n| (6..=15).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit()))
}

/// WhatsApp Desktop's own link: at a person's chat, or the app as it is.
pub fn app_link(chat: Option<&str>) -> String {
    match phone_of(chat) {
        Some(n) => format!("whatsapp://send?phone={n}"),
        None => "whatsapp://".into(),
    }
}

/// WhatsApp Web's, in the browser: at a person's chat, or its home page.
pub fn web_link(chat: Option<&str>) -> String {
    match phone_of(chat) {
        Some(n) => format!("{WEB_URL}/send?phone={n}"),
        None => WEB_URL.into(),
    }
}

/// WhatsApp Desktop when it is installed (the Microsoft Store app registers `whatsapp:`), at the
/// chat; WhatsApp Web in the browser otherwise. True when it went to the app.
fn open_in_whatsapp(chat: Option<&str>) -> bool {
    let app = crate::platform::handles_scheme("whatsapp");
    crate::platform::open_url(&if app { app_link(chat) } else { web_link(chat) });
    app
}

// ── GOWA install ──────────────────────────────────────────────────────────────

fn set_install(step: Step) {
    ctl().install = step;
    changed();
}

fn install() {
    {
        let mut c = ctl();
        if c.install.busy() {
            return;
        }
        c.install = Step::Checking;
    }
    changed();
    spawn(async move {
        match install::run(set_install).await {
            Ok(version) => {
                {
                    let mut c = ctl();
                    c.prefs.gowa_version = Some(version.clone());
                    c.prefs.gateway = "gowa".into();
                    c.prefs.gowa_path = process::installed_binary().to_string_lossy().into_owned();
                    c.prefs.auto_start = true;
                    prefs::save(&c.prefs);
                    // Back to http://127.0.0.1:3000.
                    let _ = crate::secrets::set_internal(URL_KEY, "");
                    c.url.clear();
                    c.install = Step::Done { version: version.clone() };
                }
                crate::log::line(format!("whatsapp: GOWA {version} installed ({} {})", gateway::release::this_os(), gateway::release::this_arch()));
                // Starts it; the QR follows.
                settings_changed();
            }
            Err(reason) => {
                crate::log::line(format!("whatsapp: GOWA install failed: {reason}"));
                set_install(Step::Failed { reason });
            }
        }
    });
}

// ── Commands ──────────────────────────────────────────────────────────────────

fn only(window: &WebviewWindow, label: &str) -> Result<(), String> {
    if window.label() == label { Ok(()) } else { Err("not from this window".into()) }
}

pub fn whatsapp_island(window: WebviewWindow) -> Result<IslandView, String> {
    only(&window, crate::island::WINDOW_LABEL)?;
    Ok(ctl().island_view())
}

pub fn whatsapp_open(window: WebviewWindow, chat: String) -> Result<(), String> {
    only(&window, crate::island::WINDOW_LABEL)?;
    open(&chat);
    Ok(())
}

pub fn whatsapp_close(window: WebviewWindow, chat: String) -> Result<(), String> {
    only(&window, crate::island::WINDOW_LABEL)?;
    close(&chat);
    Ok(())
}

/// The card's ↗: WhatsApp at the conversation open in the card (`open_in_whatsapp`). True when it
/// went to WhatsApp Desktop rather than WhatsApp Web.
pub fn whatsapp_open_app(window: WebviewWindow) -> Result<bool, String> {
    only(&window, crate::island::WINDOW_LABEL)?;
    let chat = ctl().open_chat.clone();
    Ok(open_in_whatsapp(chat.as_deref()))
}

/// Enter or the send button in the island's card: the only way a message leaves Coucou.
pub async fn whatsapp_send(window: WebviewWindow, chat: String, text: String) -> Result<bool, String> {
    only(&window, crate::island::WINDOW_LABEL)?;
    Ok(send(&chat, &text).await)
}

/// Save on a card of the island: the entry goes to the Google Sheet (when one is set up) and to the
/// CSV on this computer, and only then.
pub async fn whatsapp_ledger_save(window: WebviewWindow, id: u64) -> Result<String, String> {
    only(&window, crate::island::WINDOW_LABEL)?;
    // Taken out first: a double click cannot write it twice.
    let (entry, path, sheet) = {
        let mut c = ctl();
        let index = c
            .ledger_pending
            .iter()
            .position(|(n, _)| *n == id)
            .ok_or_else(|| crate::i18n::t("That entry is no longer waiting."))?;
        let (_, entry) = c.ledger_pending.remove(index);
        let url = c.prefs.ledger_sheet_url.clone();
        let code = crate::secrets::get_internal(ledger::SHEET_KEY);
        (entry, ledger::file(&c.prefs.ledger_dir), if url.is_empty() { None } else { code.map(|code| (url, code)) })
    };
    // The sheet first: if it refuses, nothing is half done and the entry waits for another click.
    if let Some((url, code)) = &sheet {
        if let Err(reason) = ledger::post_to_sheet(url, &ledger::sheet_payload(&entry, code)).await {
            ctl().ledger_pending.insert(0, (id, entry));
            changed();
            return Err(crate::i18n::tf("Could not send the entry to the sheet: {reason}", &[("reason", ledger::sheet_reason(&reason).as_str())]));
        }
    }
    let (target, saved) = (path.clone(), entry.clone());
    let written = tauri::async_runtime::spawn_blocking(move || ledger::append(&target, &saved))
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);
    if let Err(reason) = written {
        if sheet.is_none() {
            ctl().ledger_pending.insert(0, (id, entry));
            changed();
            return Err(crate::i18n::tf("Could not save the entry: {reason}", &[("reason", reason.as_str())]));
        }
        // It is in the sheet; only the local copy failed.
        crate::log::line(format!("whatsapp: the local copy of a note could not be written: {reason}"));
    }
    changed();
    Ok(path.to_string_lossy().into_owned())
}

/// "Send quote" on an order's card: the quote goes to the customer, and only then.
pub async fn whatsapp_order_quote(window: WebviewWindow, id: u64) -> Result<(), String> {
    only(&window, crate::island::WINDOW_LABEL)?;
    let payment = tauri::async_runtime::spawn_blocking(orders::load).await.map(|c| c.pagamento).unwrap_or_default();
    let (chat, text) = {
        let c = ctl();
        let (_, order) = c
            .orders
            .iter()
            .find(|(n, o)| *n == id && o.state == orders::State::Draft)
            .ok_or_else(|| crate::i18n::t("That order is no longer waiting."))?;
        let text = order.quote(&payment).ok_or_else(|| crate::i18n::t("The table has no price for this order."))?;
        (order.chat.clone(), text)
    };
    if !send(&chat, &text).await {
        return Err(crate::i18n::t("Could not send the quote."));
    }
    if let Some((_, order)) = ctl().orders.iter_mut().find(|(n, _)| *n == id) {
        order.state = orders::State::Quoted;
    }
    changed();
    Ok(())
}

/// "Copy" on an order's card: the user sends the reply themself, so from now on the customer's
/// "yes" approves it, as when Coucou sends it.
pub fn whatsapp_order_copied(window: WebviewWindow, id: u64) -> Result<(), String> {
    only(&window, crate::island::WINDOW_LABEL)?;
    if let Some((_, order)) = ctl().orders.iter_mut().find(|(n, o)| *n == id && o.state == orders::State::Draft) {
        order.state = orders::State::Quoted;
    }
    changed();
    Ok(())
}

/// "Discard" (or "Done") on an order's card: it goes, and its file is not read as an order again.
pub fn whatsapp_order_discard(window: WebviewWindow, id: u64) -> Result<(), String> {
    only(&window, crate::island::WINDOW_LABEL)?;
    {
        let mut c = ctl();
        if let Some(index) = c.orders.iter().position(|(n, _)| *n == id) {
            let (_, order) = c.orders.remove(index);
            if c.order_settled.len() > 500 {
                c.order_settled.clear();
            }
            c.order_settled.insert(order.chat, order.file.0);
        }
    }
    changed();
    Ok(())
}

/// "Get file" on an order's card: the customer's file is fetched into Documents\Coucou\Pedidos
/// and its folder opens. Resolves with the file's path.
pub async fn whatsapp_order_file(window: WebviewWindow, id: u64) -> Result<String, String> {
    only(&window, crate::island::WINDOW_LABEL)?;
    let (provider, chat, message, name, saved) = {
        let c = ctl();
        let (_, order) = c.orders.iter().find(|(n, _)| *n == id).ok_or_else(|| crate::i18n::t("That order is no longer waiting."))?;
        let provider = c.provider.clone().ok_or_else(|| crate::i18n::t("Could not fetch the file."))?;
        (provider, order.chat.clone(), order.file.0.clone(), order.name.clone(), order.saved.clone())
    };
    let dir = orders::folder(&name);
    if let Some(path) = saved.filter(|path| std::path::Path::new(path).exists()) {
        crate::platform::reveal_folder(&dir.to_string_lossy());
        return Ok(path);
    }
    let relative = provider.download(&message, &chat).await.map_err(|_| crate::i18n::t("Could not fetch the file."))?;
    let source = orders::downloaded(&process::working_dir(), &relative).ok_or_else(|| crate::i18n::t("Could not fetch the file."))?;
    let target = dir.clone();
    let kept = tauri::async_runtime::spawn_blocking(move || orders::keep(&source, &target))
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r)
        .map_err(|reason| {
            crate::log::line(format!("whatsapp: could not keep an order's file: {reason}"));
            crate::i18n::t("Could not fetch the file.")
        })?;
    let path = kept.to_string_lossy().into_owned();
    if let Some((_, order)) = ctl().orders.iter_mut().find(|(n, _)| *n == id) {
        order.saved = Some(path.clone());
    }
    changed();
    crate::platform::reveal_folder(&dir.to_string_lossy());
    Ok(path)
}

pub fn whatsapp_ledger_discard(window: WebviewWindow, id: u64) -> Result<(), String> {
    only(&window, crate::island::WINDOW_LABEL)?;
    ctl().ledger_pending.retain(|(n, _)| *n != id);
    changed();
    Ok(())
}

/// Settings → WhatsApp → money notes: on or off, and the folder of the spreadsheet.
pub fn whatsapp_set_ledger(window: WebviewWindow, enabled: bool, dir: String, chat: String) -> Result<(), String> {
    only(&window, crate::HUB_LABEL)?;
    let dir = dir.trim().to_string();
    if !dir.is_empty() && !std::path::Path::new(&dir).is_absolute() {
        return Err(crate::i18n::t("Type the whole folder path, like C:\\Users\\you\\OneDrive\\Coucou."));
    }
    {
        let mut c = ctl();
        c.prefs.ledger_enabled = enabled;
        c.prefs.ledger_dir = dir;
        c.prefs.ledger_chat = chat.trim().to_string();
        c.prefs = std::mem::take(&mut c.prefs).cleaned();
        if !enabled {
            c.ledger_queue.clear();
            c.ledger_pending.clear();
        }
        prefs::save(&c.prefs);
    }
    changed();
    Ok(())
}

/// Settings → WhatsApp → money notes → Google Sheet: the script's address and, when given, its code
/// (None keeps the stored one, "" removes it). The code is read by Rust only.
pub fn whatsapp_set_ledger_sheet(window: WebviewWindow, url: String, code: Option<String>) -> Result<(), String> {
    only(&window, crate::HUB_LABEL)?;
    let url = url.trim().to_string();
    if !url.is_empty() && !ledger::valid_sheet_url(&url) {
        return Err(crate::i18n::t("Type the address Google gave when you deployed the script (it starts with https://script.google.com/macros/s/)."));
    }
    let stored = match code {
        Some(code) => {
            crate::secrets::set_internal(ledger::SHEET_KEY, code.trim())?;
            Some(!code.trim().is_empty())
        }
        None => None,
    };
    {
        let mut c = ctl();
        c.prefs.ledger_sheet_url = url;
        c.prefs = std::mem::take(&mut c.prefs).cleaned();
        if let Some(has) = stored {
            c.ledger_sheet_code = has;
        }
        prefs::save(&c.prefs);
    }
    changed();
    Ok(())
}

/// "Test the sheet": one call to the script that adds nothing; it only has to answer yes.
pub async fn whatsapp_ledger_sheet_test(window: WebviewWindow) -> Result<(), String> {
    only(&window, crate::HUB_LABEL)?;
    let url = ctl().prefs.ledger_sheet_url.clone();
    let code = crate::secrets::get_internal(ledger::SHEET_KEY);
    let (Some(code), false) = (code, url.is_empty()) else {
        return Err(crate::i18n::t("Fill in the address and the code first."));
    };
    ledger::post_to_sheet(&url, &serde_json::json!({ "code": code, "ping": true }))
        .await
        .map_err(|reason| ledger::sheet_reason(&reason))
}

/// Settings → ignore groups with more than this many people (you included); 0 turns it off. The groups
/// already seen are asked about at once, without announcing what was waiting.
pub fn whatsapp_set_group_limit(window: WebviewWindow, limit: u32) -> Result<(), String> {
    only(&window, crate::HUB_LABEL)?;
    let (provider, gen, groups) = {
        let mut c = ctl();
        c.prefs.ignore_groups_over = limit.min(1024);
        prefs::save(&c.prefs);
        let chats: Vec<String> = c.inbox.conversations.keys().cloned().collect();
        let groups = chats.iter().filter_map(|chat| needs_name(&mut c, chat)).collect::<Vec<_>>();
        (c.provider.clone(), c.generation, groups)
    };
    if let Some(provider) = provider {
        name_groups(provider, gen, groups, false);
    }
    changed();
    Ok(())
}

/// A reply the user may send, written from the conversation's last messages (suggest.rs). ""
/// when there is nothing to answer. Shown in the card only: sending stays `whatsapp_send`.
pub async fn whatsapp_suggest(window: WebviewWindow, chat: String) -> Result<String, String> {
    only(&window, crate::island::WINDOW_LABEL)?;
    let prompt = ctl().inbox.conversations.get(&chat).and_then(suggest::prompt);
    match prompt {
        Some(prompt) => suggest::suggest(&prompt).await,
        None => Ok(String::new()),
    }
}

pub fn whatsapp_ignore(window: WebviewWindow, chat: String) -> Result<(), String> {
    if window.label() != crate::island::WINDOW_LABEL {
        only(&window, crate::HUB_LABEL)?;
    }
    let chat = chat_from_entry(&chat).ok_or("not a chat")?;
    ignore(&chat);
    Ok(())
}

pub fn whatsapp_unignore(window: WebviewWindow, chat: String) -> Result<(), String> {
    only(&window, crate::HUB_LABEL)?;
    {
        let mut c = ctl();
        c.prefs.ignored.retain(|x| x != &chat);
        prefs::save(&c.prefs);
    }
    changed();
    Ok(())
}

pub fn whatsapp_settings(window: WebviewWindow) -> Result<SettingsView, String> {
    only(&window, crate::HUB_LABEL)?;
    Ok(ctl().settings_view())
}

/// The switches: on/off, mute groups, read receipts on opening.
pub fn whatsapp_set_options(window: WebviewWindow, enabled: bool, mute_groups: bool, mark_read_on_open: bool) -> Result<(), String> {
    only(&window, crate::HUB_LABEL)?;
    let restart = {
        let mut c = ctl();
        let restart = c.prefs.enabled != enabled;
        c.prefs.enabled = enabled;
        c.prefs.mute_groups = mute_groups;
        c.prefs.mark_read_on_open = mark_read_on_open;
        prefs::save(&c.prefs);
        restart
    };
    if restart { settings_changed() } else { changed() }
    Ok(())
}

/// Advanced → Save: the gateway, its address and credential (None keeps the stored one, "" removes
/// it), the webhook's port, and GOWA started by Coucou.
pub fn whatsapp_save_gateway(
    window: WebviewWindow,
    gateway: String,
    url: String,
    credential: Option<String>,
    port: u16,
    auto_start: bool,
    gowa_path: String,
) -> Result<(), String> {
    only(&window, crate::HUB_LABEL)?;
    let url = url.trim().to_string();
    if url.len() > 512 || credential.as_ref().is_some_and(|c| c.len() > 1024) {
        return Err("too long".into());
    }
    crate::secrets::set_internal(URL_KEY, &url)?;
    if let Some(credential) = &credential {
        crate::secrets::set_internal(CREDENTIAL_KEY, credential.trim())?;
    }
    {
        let mut c = ctl();
        c.url = url;
        if let Some(credential) = credential {
            c.has_credential = !credential.trim().is_empty();
        }
        let mut next = c.prefs.clone();
        next.gateway = gateway;
        if (1024..=65535).contains(&port) {
            next.port = port;
        }
        next.auto_start = auto_start;
        next.gowa_path = gowa_path;
        c.prefs = next.cleaned();
        prefs::save(&c.prefs);
    }
    settings_changed();
    Ok(())
}

pub async fn whatsapp_test(window: WebviewWindow) -> Result<TestResult, String> {
    only(&window, crate::HUB_LABEL)?;
    Ok(test_connection().await)
}

/// "Install WhatsApp" / "Update": the one network call to GitHub, on this click only.
pub fn whatsapp_install(window: WebviewWindow) -> Result<(), String> {
    only(&window, crate::HUB_LABEL)?;
    install();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: &str, chat: &str, text: &str) -> Message {
        Message::new(id, chat, chat, text, gateway::now_ms())
    }

    fn fresh_ctl() -> Ctl {
        Ctl {
            prefs: Prefs::default(),
            url: String::new(),
            has_credential: false,
            pill_active: true,
            provider: None,
            generation: 0,
            link: LinkState::Off,
            webhook: WebhookState::Off,
            secret: String::new(),
            qr: None,
            qr_expiry: None,
            me: None,
            inbox: Inbox::default(),
            listener: None,
            tick: 0,
            refreshing: false,
            polling: false,
            backfilled: false,
            last_webhook: None,
            last_poll_ms: 0,
            naming: HashSet::new(),
            group_sizes: HashMap::new(),
            sizing: HashSet::new(),
            open_chat: None,
            rate: RateLimit::default(),
            failures: 0,
            down: false,
            sending: false,
            send_error: None,
            install: Step::Idle,
            ledger_queue: Vec::new(),
            ledger_pending: Vec::new(),
            ledger_next: 0,
            ledger_sheet_code: false,
            order_queue: Vec::new(),
            order_reading: HashSet::new(),
            orders: Vec::new(),
            order_next: 0,
            order_settled: HashMap::new(),
            order_news: Vec::new(),
        }
    }

    #[test]
    fn what_arrives_is_announced_by_name_only_and_never_in_the_open_chat() {
        let mut c = fresh_ctl();
        let mut m = msg("1", "33612345678@s.whatsapp.net", "secret text");
        m.sender_name = "Marie".into();
        let (fresh, groups) = ingest(&mut c, vec![m]);
        assert_eq!(fresh.as_deref(), Some("Marie"));
        assert!(groups.is_empty());
        // Open on screen: seen, not unread, not announced.
        c.open_chat = Some("33612345678@s.whatsapp.net".into());
        let (fresh, _) = ingest(&mut c, vec![msg("2", "33612345678@c.us", "again")]);
        assert_eq!(fresh, None);
        assert_eq!(c.inbox.conversations["33612345678@s.whatsapp.net"].unread, 0, "read on screen");
        assert_eq!(c.inbox.conversations["33612345678@s.whatsapp.net"].messages.len(), 2);
        c.open_chat = None;
        let (fresh, _) = ingest(&mut c, vec![msg("2b", "33612345678@s.whatsapp.net", "and again")]);
        assert_eq!(fresh.as_deref(), Some("Marie"), "the person still names the chat");
        // A muted group without a mention stays quiet, but is asked its name once.
        let (fresh, groups) = ingest(&mut c, vec![msg("3", "120363@g.us", "hi all")]);
        assert_eq!(fresh, None);
        assert_eq!(groups, ["120363@g.us"]);
        let (_, groups) = ingest(&mut c, vec![msg("4", "120363@g.us", "hi again")]);
        assert!(groups.is_empty(), "asked once");
        // The island only sees the visible conversations.
        let view = c.island_view();
        assert_eq!(view.conversations.len(), 1);
        assert_eq!(view.unread, 1);
        // Ignored: never shown again.
        c.prefs.ignored = vec!["33612345678@s.whatsapp.net".into()];
        let (fresh, _) = ingest(&mut c, vec![msg("5", "33612345678@s.whatsapp.net", "x")]);
        assert_eq!(fresh, None);
        assert!(c.island_view().conversations.is_empty());
    }

    #[test]
    fn errors_light_the_pill_on_the_second_failure_only() {
        let mut c = fresh_ctl();
        apply(&mut c, WaError::NotLoggedIn);
        assert_eq!((c.link.clone(), c.failures, c.down), (LinkState::NeedsQr, 0, false));
        apply(&mut c, WaError::Unauthorized);
        assert_eq!((c.failures, c.down), (1, false));
        apply(&mut c, WaError::Refused("NOPE".into()));
        assert_eq!((c.link.clone(), c.failures, c.down), (LinkState::Failed { detail: "NOPE".into() }, 2, true));
    }

    #[test]
    fn mark_read_sends_a_receipt_only_for_something_unread() {
        let mut c = fresh_ctl();
        ingest(&mut c, vec![msg("a", "336@s.whatsapp.net", "1"), msg("b", "336@s.whatsapp.net", "2")]);
        assert_eq!(mark_read(&mut c, "336@s.whatsapp.net"), Some(("336@s.whatsapp.net".into(), Some("b".into()))));
        assert_eq!(c.inbox.conversations["336@s.whatsapp.net"].unread, 0);
        assert_eq!(mark_read(&mut c, "336@s.whatsapp.net"), None, "nothing new: no receipt");
        assert_eq!(mark_read(&mut c, "999@s.whatsapp.net"), None);
    }

    #[test]
    fn the_gateways_secrets_never_reach_a_webview() {
        // The code of the Google Sheet is one of them: Rust reads it, no page ever does.
        for key in [URL_KEY, CREDENTIAL_KEY, SECRET_KEY, ledger::SHEET_KEY] {
            assert!(crate::secrets::is_internal(key), "{key}");
            assert!(!crate::secrets::KNOWN_KEYS.contains(&key), "{key}");
            assert!(crate::secrets::set(key, "x").is_err(), "{key}: secret_set must refuse it");
            assert!(!crate::secrets::present(key), "{key}");
        }
        // Four for each of the two accounts.
        assert_eq!(crate::secrets::INTERNAL_KEYS.iter().filter(|k| k.starts_with("whatsapp-")).count(), 8);
    }


    #[test]
    fn big_groups_are_ignored_and_a_group_of_unknown_size_waits_for_its_size() {
        let group = "120363000000000001@g.us";
        let other = "120363000000000002@g.us";
        let mut c = fresh_ctl();
        c.prefs.mute_groups = false;
        c.prefs.ignore_groups_over = 4;

        // Size unknown: kept and looked up, not announced yet.
        let (fresh, asked) = ingest(&mut c, vec![msg("g1", group, "oi")]);
        assert_eq!((fresh, asked), (None, vec![group.to_string()]));
        assert!(c.sizing.contains(group), "asked once");

        // The answer: 6 people. Hidden from the list, later messages dropped, nothing announced.
        c.group_sizes.insert(group.into(), Some(6));
        assert!(c.inbox.visible(&c.filters()).is_empty());
        let (fresh, _) = ingest(&mut c, vec![msg("g2", group, "de novo")]);
        assert_eq!(fresh, None);

        // 4 people is allowed ("more than 4" is what is ignored).
        c.group_sizes.insert(group.into(), Some(4));
        let (fresh, _) = ingest(&mut c, vec![msg("g3", group, "oi")]);
        assert!(fresh.is_some(), "a group of four is announced");

        // The gateway did not say: allowed.
        c.group_sizes.insert(other.into(), None);
        let (fresh, _) = ingest(&mut c, vec![msg("o1", other, "oi")]);
        assert!(fresh.is_some());

        // Off: never held back, whatever the size.
        c.prefs.ignore_groups_over = 0;
        c.group_sizes.insert(group.into(), Some(50));
        let (fresh, _) = ingest(&mut c, vec![msg("g4", group, "oi")]);
        assert!(fresh.is_some());
        assert_eq!(c.filters().ignored.len(), 0);
    }

    #[test]
    fn a_private_chat_is_never_held_for_a_size() {
        let mut c = fresh_ctl();
        c.prefs.ignore_groups_over = 4;
        let (fresh, asked) = ingest(&mut c, vec![msg("p1", "336@s.whatsapp.net", "oi")]);
        assert!(fresh.is_some() && asked.is_empty());
    }

    #[test]
    fn ignore_entries() {
        assert_eq!(chat_from_entry("+33 6 12 34 56 78").as_deref(), Some("33612345678@s.whatsapp.net"));
        assert_eq!(chat_from_entry(" 120363@g.us ").as_deref(), Some("120363@g.us"));
        assert_eq!(chat_from_entry("abc"), None);
        assert_eq!(chat_from_entry("a b@g.us"), None);
        assert_eq!(chat_from_entry(""), None);
    }

    #[test]
    fn the_arrow_opens_a_persons_chat_in_whatsapp_and_anything_else_as_it_is() {
        assert_eq!(app_link(Some("447700900123@s.whatsapp.net")), "whatsapp://send?phone=447700900123");
        assert_eq!(app_link(Some("447700900123:3@s.whatsapp.net")), "whatsapp://send?phone=447700900123");
        assert_eq!(app_link(Some("447700900123@c.us")), "whatsapp://send?phone=447700900123");
        assert_eq!(app_link(Some("120363012345678901@g.us")), "whatsapp://", "a group has no number");
        assert_eq!(app_link(Some("123456789012345@lid")), "whatsapp://", "a hidden number stays hidden");
        assert_eq!(app_link(Some("12@s.whatsapp.net")), "whatsapp://");
        assert_eq!(app_link(None), "whatsapp://");
        // Without the app, WhatsApp Web at the same chat.
        assert_eq!(web_link(Some("447700900123@s.whatsapp.net")), "https://web.whatsapp.com/send?phone=447700900123");
        assert_eq!(web_link(Some("120363012345678901@g.us")), "https://web.whatsapp.com");
        assert_eq!(web_link(None), "https://web.whatsapp.com");
    }

    #[test]
    fn views_say_codes_never_sentences() {
        let c = fresh_ctl();
        let json = serde_json::to_value(c.island_view()).unwrap();
        assert_eq!(json["link"]["state"], "off");
        let json = serde_json::to_value(c.settings_view()).unwrap();
        assert_eq!(json["webhook"]["state"], "off");
        assert_eq!(json["install"]["step"], "idle");
        // Each account has its own default address: the second one's gateway sits next to the first one's.
        assert_eq!(json["defaultUrl"], if SECOND { "http://127.0.0.1:3011" } else { "http://127.0.0.1:3000" });
        assert_eq!(serde_json::to_value(SendError::Refused { detail: "x".into() }).unwrap()["code"], "refused");
        assert_eq!(serde_json::to_value(WebhookState::PortBusy { port: 1 }).unwrap()["state"], "portBusy");
        assert_eq!(serde_json::to_value(LinkState::NeedsQr).unwrap()["state"], "needsQr");
        assert_eq!(serde_json::to_value(TestResult::Connected { who: "+1".into() }).unwrap()["result"], "connected");
    }

    fn file_from(id: &str, chat: &str) -> Message {
        let mut m = msg(id, chat, "");
        m.media = model::Media::Document { name: Some("a.pdf".into()) };
        m
    }

    fn draft(chat: &str, file: &str) -> orders::Order {
        orders::Order {
            chat: chat.into(),
            name: "Ana".into(),
            quantity: 6,
            lines: vec![orders::Line { text: "6 × impressão colorida".into(), cents: Some(1200) }],
            note: String::new(),
            file: (file.into(), "a.pdf".into()),
            printer: "WF5390".into(),
            state: orders::State::Draft,
            saved: None,
            payment: String::new(),
        }
    }

    #[test]
    fn a_customers_file_and_words_queue_an_order_and_a_yes_approves_the_quote() {
        let chat = "5511999990000@s.whatsapp.net";
        let mut c = fresh_ctl();
        ingest(&mut c, vec![msg("1", chat, "bom dia")]);
        ingest(&mut c, vec![file_from("2", chat)]);
        assert!(c.order_queue.is_empty(), "a file with no word about printing is not read");
        ingest(&mut c, vec![msg("3", chat, "quero 6 folhas coloridas")]);
        assert_eq!(c.order_queue, [chat]);
        ingest(&mut c, vec![msg("3", chat, "quero 6 folhas coloridas")]);
        assert_eq!(c.order_queue.len(), 1, "a duplicate queues nothing");
        c.order_queue.clear();

        assert!(place_order(&mut c, draft(chat, "2")));
        assert!(!place_order(&mut c, draft(chat, "2")), "the same draft again is not news");
        assert_eq!(c.island_view().orders.len(), 1);
        // A "yes" before any quote was sent approves nothing.
        ingest(&mut c, vec![msg("4", chat, "sim")]);
        assert_eq!(c.orders[0].1.state, orders::State::Draft);
        c.order_queue.clear();
        let id = c.orders[0].0;
        if let Some((_, order)) = c.orders.iter_mut().find(|(n, o)| *n == id && o.state == orders::State::Draft) {
            order.state = orders::State::Quoted;
        }
        assert_eq!(c.orders[0].1.state, orders::State::Quoted, "copying the reply waits for the customer");
        assert!(!place_order(&mut c, draft(chat, "2")), "a quote on its way is not replaced");
        ingest(&mut c, vec![msg("5", chat, "quanto tempo demora?")]);
        assert_eq!(c.orders[0].1.state, orders::State::Quoted);
        ingest(&mut c, vec![msg("6", chat, "Pode imprimir")]);
        assert_eq!(c.orders[0].1.state, orders::State::Approved);
        assert_eq!(c.order_news, ["Ana"]);
        assert!(c.order_queue.is_empty());

        // What I write myself, and what is said in a group, is never an order.
        let mut mine = file_from("7", "5511888880000@s.whatsapp.net");
        mine.from_me = true;
        ingest(&mut c, vec![mine, msg("8", "5511888880000@s.whatsapp.net", "imprime isso")]);
        assert_eq!(c.order_queue.len(), 0);
        ingest(&mut c, vec![file_from("9", "120363000000000000@g.us"), msg("10", "120363000000000000@g.us", "imprime 3 folhas")]);
        assert!(c.order_queue.is_empty());
    }

    #[test]
    fn a_discarded_order_is_not_read_again_until_a_new_file_comes() {
        let chat = "5511999990000@s.whatsapp.net";
        let mut c = fresh_ctl();
        ingest(&mut c, vec![file_from("1", chat), msg("2", chat, "imprime 3 folhas")]);
        c.order_queue.clear();
        c.order_settled.insert(chat.into(), "1".into());
        ingest(&mut c, vec![msg("3", chat, "em preto e branco")]);
        assert!(c.order_queue.is_empty());
        ingest(&mut c, vec![file_from("4", chat)]);
        assert_eq!(c.order_queue, [chat]);
    }
}
