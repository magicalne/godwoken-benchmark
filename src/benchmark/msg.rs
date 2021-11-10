use std::time::Duration;

use tokio::sync::oneshot;

use crate::tx::msg::TxStatus;

use super::plan::Privkey;

pub struct BatchReqMsg {
    pub pks: Vec<Privkey>,
    pub method: ReqMethod,
    pub amount: u128,
    pub fee: u128,
    pub sudt_id: u32,
}

pub enum ReqMethod {
    Submit,
    Execute,
}

pub struct BatchResMsg {
    pub pk_idx_vec: Vec<usize>,
}

#[derive(Debug)]
pub struct Stats {
    pub timeout: usize,
    pub failure: usize,
    pub pending_commit: usize,
    pub committed: usize,
}

pub enum ApiStatus {
    Success,
    Failure,
}

pub enum StatsReqMsg {
    SendApiStatus {
        api: String,
        duration: Duration,
        status: ApiStatus,
    },
    SendTxStatus(TxStatus),
    Get(oneshot::Sender<crate::benchmark::stats::Stats>),
}
