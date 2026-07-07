use std::time::Duration;

use tokio::sync::mpsc;

use crate::{
    disk_usage::DiskUsageSnapshot,
    git_status::GitStatus,
    input::InputEvent,
    nextest::{DiscoveryEvent, RunEvent},
    xtask::XtaskEvent,
};

#[derive(Debug, Clone)]
pub enum QueueEvent {
    Input(InputEvent),
    Discovery(DiscoveryEvent),
    CargoClean(Result<(), String>),
    DiskUsage(Result<DiskUsageSnapshot, String>),
    GitStatus(GitStatus),
    Run(RunEvent),
    Xtask(XtaskEvent),
    Tick,
}

pub type QueueSender = mpsc::UnboundedSender<QueueEvent>;
pub type QueueReceiver = mpsc::UnboundedReceiver<QueueEvent>;

pub fn channel() -> (QueueSender, QueueReceiver) {
    mpsc::unbounded_channel()
}

pub fn start_ticker(tx: QueueSender, interval: Duration) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            if tx.send(QueueEvent::Tick).is_err() {
                break;
            }
        }
    })
}
