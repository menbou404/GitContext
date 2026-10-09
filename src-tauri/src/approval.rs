use gitcontext_core::approval::{self, ApprovalRequest};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
    Mutex,
};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

struct Pending {
    request: ApprovalRequest,
    displayed: Option<Instant>,
    respond: SyncSender<bool>,
}

pub struct ApprovalState {
    enabled: AtomicBool,
    started: AtomicBool,
    error: Mutex<Option<String>>,
    pending: Mutex<Option<Pending>>,
}

impl ApprovalState {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            started: AtomicBool::new(false),
            error: Mutex::new(None),
            pending: Mutex::new(None),
        }
    }

    pub fn status(&self) -> Option<String> {
        self.error.lock().ok()?.clone()
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
        if !enabled {
            self.decline();
        }
    }

    pub fn decline(&self) {
        if let Some(pending) = self.pending.lock().ok().and_then(|mut p| p.take()) {
            let _ = pending.respond.send(false);
        }
    }

    pub fn answer(&self, id: &str, approved: bool) -> Result<(), String> {
        let mut guard = self.pending.lock().map_err(|e| e.to_string())?;
        let pending = guard.as_ref().ok_or("No approval is pending")?;
        if pending.request.id != id {
            return Err("Approval ID does not match".into());
        }
        if pending
            .displayed
            .is_none_or(|time| time.elapsed() < Duration::from_secs(1))
            || pending.request.remaining().is_zero()
        {
            return Err("Approval is too early or has expired".into());
        }
        if let Some(pending) = guard.take() {
            let _ = pending.respond.send(approved);
        }
        Ok(())
    }

    pub fn current(&self) -> Option<ApprovalRequest> {
        let mut pending = self.pending.lock().ok()?;
        let pending = pending.as_mut()?;
        pending.displayed.get_or_insert_with(Instant::now);
        Some(pending.request.clone())
    }
}

pub fn start(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<ApprovalState>();
    if state.started.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let name = match approval::pipe_name() {
        Ok(name) => name,
        Err(error) => {
            state.started.store(false, Ordering::SeqCst);
            state.enabled.store(false, Ordering::SeqCst);
            *state.error.lock().map_err(|e| e.to_string())? = Some(error.clone());
            return Err(error);
        }
    };
    let app = app.clone();
    let result = approval::start_server(name, move |request| {
        let state = app.state::<ApprovalState>();
        if !state.enabled.load(Ordering::SeqCst) {
            return false;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        if let Ok(mut pending) = state.pending.lock() {
            *pending = Some(Pending {
                request: request.clone(),
                displayed: None,
                respond: sender,
            });
        } else {
            return false;
        }
        let app_for_window = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(window) = app_for_window.get_webview_window("approval") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.request_user_attention(Some(tauri::UserAttentionType::Critical));
                let _ = window.emit("approval-request", ());
            } else if let Ok(window) = WebviewWindowBuilder::new(
                &app_for_window,
                "approval",
                WebviewUrl::App("index.html?approval".into()),
            )
            .title("GitContext")
            .inner_size(540.0, 460.0)
            .min_inner_size(440.0, 380.0)
            .always_on_top(true)
            .build()
            {
                let _ = window.request_user_attention(Some(tauri::UserAttentionType::Critical));
            }
        });
        let remaining = request.remaining().min(Duration::from_secs(120));
        let approved = receiver.recv_timeout(remaining).unwrap_or(false);
        state.decline();
        let app_for_hide = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(window) = app_for_hide.get_webview_window("approval") {
                let _ = window.hide();
            }
        });
        approved
    });
    if let Err(error) = result {
        state.started.store(false, Ordering::SeqCst);
        state.enabled.store(false, Ordering::SeqCst);
        *state.error.lock().map_err(|e| e.to_string())? = Some(error.clone());
        return Err(error);
    }
    *state.error.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}
