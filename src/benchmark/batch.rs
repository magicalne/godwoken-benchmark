use futures::future::join_all;
use tokio::sync::mpsc;

use crate::tx::{msg::TransferInfo, transfer::TransferHandler};

use super::{
    msg::{BatchReqMsg, BatchResMsg, ReqMethod},
    plan::Privkey,
};

pub struct BatchActor {
    receiver: mpsc::Receiver<BatchReqMsg>,
    tx_handler: TransferHandler,
    batch_res_sender: mpsc::Sender<BatchResMsg>,
}

impl BatchActor {
    fn new(
        receiver: mpsc::Receiver<BatchReqMsg>,
        tx_handler: TransferHandler,
        batch_res_sender: mpsc::Sender<BatchResMsg>,
    ) -> Self {
        Self {
            receiver,
            tx_handler,
            batch_res_sender,
        }
    }

    fn handle(&self, msg: BatchReqMsg) {
        let BatchReqMsg {
            method,
            pks,
            amount,
            fee,
            sudt_id,
        } = msg;

        let mut to = pks.clone();
        to.rotate_right(1);
        let tx_handler = self.tx_handler.clone();
        let batch_res_sender = self.batch_res_sender.clone();
        tokio::spawn(async move {
            let futures = pks
                .clone()
                .into_iter()
                .zip(to.into_iter())
                .into_iter()
                .map(|(pk_from, pk_to)| TransferInfo {
                    pk_from: pk_from.pk,
                    pk_to: pk_to.pk,
                    amount,
                    fee,
                    sudt_id,
                })
                .map(|t| async {
                    match method {
                        ReqMethod::Submit => {
                            let _ = tx_handler.submit(t).await;
                        }
                        ReqMethod::Execute => {
                            let _ = tx_handler.execute(t).await;
                        }
                    };
                })
                .collect::<Vec<_>>();
            let _ = join_all(futures).await;
            let pk_idx_vec = pks.into_iter().map(|pk| pk.idx).collect();
            let msg = BatchResMsg { pk_idx_vec };
            let _ = batch_res_sender.send(msg).await;
        });
    }
}

async fn batch_handler(mut actor: BatchActor) {
    log::info!("batch handler is running now");
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle(msg);
    }
}

#[derive(Clone)]
pub struct BatchHandler {
    sender: mpsc::Sender<BatchReqMsg>,
}

impl BatchHandler {
    pub fn new(tx_handler: TransferHandler, batch_res_sender: mpsc::Sender<BatchResMsg>) -> Self {
        let (sender, receiver) = mpsc::channel(100);
        let actor = BatchActor::new(receiver, tx_handler, batch_res_sender);
        tokio::spawn(batch_handler(actor));
        Self { sender }
    }

    pub fn try_send_batch(
        &self,
        pks: Vec<Privkey>,
        method: ReqMethod,
        amount: u128,
        fee: u128,
        sudt_id: u32,
    ) -> Result<(), Vec<Privkey>> {
        let msg = BatchReqMsg {
            pks,
            method,
            amount,
            fee,
            sudt_id,
        };
        self.sender.try_send(msg).map_err(|err| match err {
            mpsc::error::TrySendError::Full(msg) => msg.pks,
            mpsc::error::TrySendError::Closed(msg) => msg.pks,
        })
    }
}
