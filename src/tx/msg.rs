use std::fmt;

use ckb_fixed_hash::H256;
use futures::channel::oneshot;

pub enum TransferMsg {
    Submit(TransferInfo, oneshot::Sender<TxStatus>),
    Execute(TransferInfo, oneshot::Sender<TxStatus>),
}
pub struct TransferInfo {
    pub pk_from: H256,
    pub pk_to: H256,
    pub amount: u128,
    pub fee: u128,
    pub sudt_id: u32,
}

impl fmt::Debug for TransferInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "from idx: {}, to:{}, amount: {}, fee: {}, sudt_id: {}",
            hex::encode(&self.pk_from),
            hex::encode(&self.pk_to),
            self.amount,
            self.fee,
            self.sudt_id,
        )
    }
}

pub enum TxStatus {
    Failure,
    Committed(Option<H256>),
    Timeout(H256),
}
