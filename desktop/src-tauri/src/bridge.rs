use dao_shell::{
    capabilities::Interaction,
    config::Config,
    core::{Cancellation, FileObject, Operation},
    session::{Action, FileSession, Reply},
};
use serde_json::{Value, json};
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
    },
    time::{Duration, Instant},
};
use tauri::ipc::Channel;

struct Pending {
    id: String,
    deadline: Instant,
    answer: Sender<bool>,
}
pub struct Bridge {
    session: Mutex<Option<FileSession>>,
    cancellation: Cancellation,
    busy: AtomicBool,
    pending: Mutex<Option<Pending>>,
}

impl Bridge {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
            cancellation: Cancellation::default(),
            busy: AtomicBool::new(false),
            pending: Mutex::new(None),
        }
    }
    fn ensure_session(&self, session: &mut Option<FileSession>) -> Result<(), String> {
        if session.is_none() {
            let path = std::env::var_os("DAO_SHELL_DESKTOP_CONFIG")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(Config::path);
            let config = Config::load(&path).map_err(|e| format!("{e:#}"))?;
            *session = Some(
                FileSession::new(&config, self.cancellation.clone())
                    .map_err(|e| format!("{e:#}"))?,
            );
        }
        Ok(())
    }
    pub fn info(&self) -> Result<Value, String> {
        let mut session = self
            .session
            .try_lock()
            .map_err(|_| "正在处理请求".to_string())?;
        self.ensure_session(&mut session)?;
        serde_json::to_value(session.as_ref().unwrap().info()).map_err(|e| e.to_string())
    }
    pub fn reserve(&self) -> Result<(), String> {
        let _pending = self
            .pending
            .lock()
            .map_err(|_| "确认状态不可用".to_string())?;
        self.busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "上一条请求仍在处理，请先取消或等待完成".to_string())?;
        self.cancellation.reset();
        Ok(())
    }
    pub fn execute(&self, action: Action, channel: Channel<Value>) -> Result<Reply, String> {
        struct Release<'a>(&'a Bridge);
        impl Drop for Release<'_> {
            fn drop(&mut self) {
                if let Ok(mut pending) = self.0.pending.lock() {
                    *pending = None;
                }
                self.0.busy.store(false, Ordering::SeqCst);
            }
        }
        let _release = Release(self);
        let mut session = self
            .session
            .lock()
            .map_err(|_| "会话不可用，请重启".to_string())?;
        self.ensure_session(&mut session)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let mut ui = DesktopInteraction {
            bridge: self,
            channel,
        };
        Ok(runtime.block_on(session.as_mut().unwrap().execute(action, &mut ui)))
    }
    pub fn cancel(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            self.cancellation.cancel();
            if let Some(pending) = pending.take() {
                let _ = pending.answer.send(false);
            }
        }
    }
    pub fn confirm(&self, id: &str, approved: bool) -> Result<(), String> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "确认状态不可用".to_string())?;
        let item = pending.as_ref().ok_or("此确认已失效")?;
        if item.id != id || item.deadline <= Instant::now() || self.cancellation.is_cancelled() {
            return Err("此确认已失效，请重新请求打开".into());
        }
        pending
            .take()
            .unwrap()
            .answer
            .send(approved)
            .map_err(|_| "此请求已结束".into())
    }
}

struct DesktopInteraction<'a> {
    bridge: &'a Bridge,
    channel: Channel<Value>,
}
impl DesktopInteraction<'_> {
    fn send(&self, value: Value) {
        if self.channel.send(value).is_err() {
            self.bridge.cancel();
        }
    }
}
impl Interaction for DesktopInteraction<'_> {
    fn confirm_move(&mut self, _: &Operation) -> anyhow::Result<bool> {
        Ok(false)
    }
    fn confirm_open(&mut self, object: &FileObject) -> anyhow::Result<bool> {
        let id = uuid::Uuid::new_v4().to_string();
        let deadline = Instant::now() + Duration::from_secs(90);
        let (answer, receiver) = mpsc::channel();
        *self
            .bridge
            .pending
            .lock()
            .map_err(|_| anyhow::anyhow!("确认状态不可用"))? = Some(Pending {
            id: id.clone(),
            deadline,
            answer,
        });
        self.send(json!({"kind":"confirm_open", "request_id":id, "object":object, "expires_in_seconds":90}));
        let approved = loop {
            if self.bridge.cancellation.is_cancelled() || Instant::now() >= deadline {
                break false;
            }
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(answer) => break answer,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => break false,
            }
        };
        *self
            .bridge
            .pending
            .lock()
            .map_err(|_| anyhow::anyhow!("确认状态不可用"))? = None;
        self.send(json!({"kind":"confirmation_closed", "request_id":id}));
        Ok(approved)
    }
    fn progress(&mut self, text: &str) {
        self.send(json!({"kind":"progress", "text":text}));
    }
    fn result(&mut self, capability: &str, value: &Value) {
        self.send(json!({"kind":"result", "capability":capability, "value":value}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmation_is_one_time_bound_and_cancel_cannot_be_lost_before_worker_start() {
        let bridge = Bridge::new();
        bridge.reserve().unwrap();
        assert!(bridge.reserve().is_err());
        bridge.cancel();
        assert!(bridge.cancellation.is_cancelled());
        bridge.busy.store(false, Ordering::SeqCst);
        bridge.reserve().unwrap();
        assert!(!bridge.cancellation.is_cancelled());
        let (sender, receiver) = mpsc::channel();
        *bridge.pending.lock().unwrap() = Some(Pending {
            id: "current".into(),
            deadline: Instant::now() + Duration::from_secs(30),
            answer: sender,
        });
        assert!(bridge.confirm("previous", true).is_err());
        bridge.confirm("current", false).unwrap();
        assert!(!receiver.recv().unwrap());
        assert!(bridge.confirm("current", true).is_err());
        let (sender, receiver) = mpsc::channel();
        *bridge.pending.lock().unwrap() = Some(Pending {
            id: "expired".into(),
            deadline: Instant::now() - Duration::from_secs(1),
            answer: sender,
        });
        assert!(bridge.confirm("expired", true).is_err());
        assert!(receiver.try_recv().is_err());
        let (sender, receiver) = mpsc::channel();
        *bridge.pending.lock().unwrap() = Some(Pending {
            id: "cancelled".into(),
            deadline: Instant::now() + Duration::from_secs(30),
            answer: sender,
        });
        bridge.cancel();
        assert!(!receiver.recv().unwrap());
        assert!(bridge.confirm("cancelled", true).is_err());
    }
}
