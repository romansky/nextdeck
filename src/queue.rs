use std::time::Duration;

use tokio::sync::mpsc;

use crate::{
    disk_usage::DiskUsageSnapshot,
    git_status::GitStatus,
    input::InputEvent,
    nextest::{DiscoveryEvent, RunEvent},
    request::RequestId,
    xtask::XtaskEvent,
};

#[derive(Debug)]
pub enum QueueEvent {
    Input(InputEvent),
    Discovery(RequestId, DiscoveryEvent),
    CargoClean(RequestId, Result<(), String>),
    DiskUsage(RequestId, Result<DiskUsageSnapshot, String>),
    GitStatus(GitStatus),
    Run(RunEvent),
    TestStackSample(Result<String, String>),
    Xtask(XtaskEvent),
    Tick,
}

pub(crate) const APP_EVENT_QUEUE_CAPACITY: usize = 4096;

pub type QueueSender = mpsc::Sender<QueueEvent>;
pub type QueueReceiver = mpsc::Receiver<QueueEvent>;

pub fn channel() -> (QueueSender, QueueReceiver) {
    mpsc::channel(APP_EVENT_QUEUE_CAPACITY)
}

pub fn start_ticker(tx: QueueSender, interval: Duration) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            if tx.send(QueueEvent::Tick).await.is_err() {
                break;
            }
        }
    })
}
