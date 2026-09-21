//! Entry-independent admission, cancellation and expiring one-time confirmations.
use crate::core::Cancellation;
use anyhow::{Result, ensure};
use std::{
    sync::{
        Mutex,
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};

struct Pending {
    id: String,
    deadline: Instant,
    answer: Sender<bool>,
}
#[derive(Default)]
struct State {
    busy: bool,
    pending: Option<Pending>,
}
#[derive(Default)]
pub struct RequestControl {
    state: Mutex<State>,
    cancel: Cancellation,
}
pub struct Confirmation {
    pub id: String,
    deadline: Instant,
    receiver: Receiver<bool>,
}
impl RequestControl {
    pub fn cancellation(&self) -> Cancellation {
        self.cancel.clone()
    }
    pub fn reserve(&self) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("请求状态不可用"))?;
        ensure!(!state.busy, "上一条请求仍在处理，请先取消或等待完成");
        state.busy = true;
        self.cancel.reset();
        Ok(())
    }
    pub fn release(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.pending = None;
            state.busy = false;
        }
    }
    pub fn cancel(&self) {
        if let Ok(mut state) = self.state.lock() {
            self.cancel.cancel();
            if let Some(p) = state.pending.take() {
                let _ = p.answer.send(false);
            }
        }
    }
    pub fn confirmation(&self, timeout: Duration) -> Result<Confirmation> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("确认状态不可用"))?;
        ensure!(
            state.busy && !self.cancel.is_cancelled() && state.pending.is_none(),
            "此请求已失效"
        );
        let id = uuid::Uuid::new_v4().to_string();
        let deadline = Instant::now() + timeout;
        let (answer, receiver) = mpsc::channel();
        state.pending = Some(Pending {
            id: id.clone(),
            deadline,
            answer,
        });
        Ok(Confirmation {
            id,
            deadline,
            receiver,
        })
    }
    pub fn answer(&self, id: &str, approved: bool) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("确认状态不可用"))?;
        let pending = state
            .pending
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("此确认已失效"))?;
        ensure!(
            pending.id == id && pending.deadline > Instant::now() && !self.cancel.is_cancelled(),
            "此确认已失效，请重新请求打开"
        );
        state
            .pending
            .take()
            .unwrap()
            .answer
            .send(approved)
            .map_err(|_| anyhow::anyhow!("此请求已结束"))
    }
    pub fn wait(&self, confirmation: &Confirmation) -> bool {
        let approved = loop {
            if self.cancel.is_cancelled() || Instant::now() >= confirmation.deadline {
                break false;
            }
            match confirmation
                .receiver
                .recv_timeout(Duration::from_millis(100))
            {
                Ok(answer) => break answer,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => break false,
            }
        };
        if let Ok(mut state) = self.state.lock()
            && state
                .pending
                .as_ref()
                .is_some_and(|p| p.id == confirmation.id)
        {
            state.pending = None;
        }
        approved && !self.cancel.is_cancelled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_early_cancel_expiry_and_replay() {
        let control = RequestControl::default();
        control.reserve().unwrap();
        assert!(control.reserve().is_err());
        control.cancel();
        assert!(control.cancellation().is_cancelled());
        assert!(control.confirmation(Duration::from_secs(1)).is_err());
        control.release();
        control.reserve().unwrap();
        let pending = control.confirmation(Duration::from_secs(5)).unwrap();
        assert!(control.answer("wrong", true).is_err());
        control.answer(&pending.id, false).unwrap();
        assert!(!control.wait(&pending));
        assert!(control.answer(&pending.id, true).is_err());
        let expired = control.confirmation(Duration::ZERO).unwrap();
        assert!(control.answer(&expired.id, true).is_err());
        assert!(!control.wait(&expired));
        let cancelled = control.confirmation(Duration::from_secs(5)).unwrap();
        control.cancel();
        assert!(!control.wait(&cancelled));
        assert!(control.answer(&cancelled.id, true).is_err());
    }
}
