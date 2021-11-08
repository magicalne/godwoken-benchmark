use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use ckb_fixed_hash::H256;
use ckb_jsonrpc_types::JsonBytes;
use ckb_types::prelude::{Builder, Entity};
use futures::channel::oneshot;
use reqwest::Url;
use tokio::sync::mpsc::{self, Sender};

use crate::{
    api::{godwoken_rpc::GodwokenRpcClient, types::L2TransactionStatus},
    generated::packed::{L2Transaction, RawL2Transaction, SUDTArgs, SUDTTransfer},
    prelude::Pack as GwPack,
    tx::msg::TxStatus,
    types::ScriptsDeploymentResult,
    utils,
};

use super::msg::{TransferInfo, TransferMsg};

pub struct TransferActor {
    url: Url,
    receiver: mpsc::Receiver<TransferMsg>,
    scripts_deployment: ScriptsDeploymentResult,
    rollup_type_hash: H256,
    timeout: u64,
}

impl TransferActor {
    pub fn new(
        timeout: u64,
        url: Url,
        rollup_type_hash: H256,
        scripts_deployment: ScriptsDeploymentResult,
        receiver: mpsc::Receiver<TransferMsg>,
    ) -> Self {
        Self {
            timeout,
            url,
            rollup_type_hash,
            scripts_deployment,
            receiver,
        }
    }

    fn handle_msg(&self, msg: TransferMsg) {
        match msg {
            TransferMsg::Submit {
                tx_info,
                gw_status,
                commit_status,
            } => self.handle_submit_msg(tx_info, gw_status, commit_status),
            TransferMsg::Execute(tx_info, sender) => self.handle_execute_msg(tx_info, sender),
        }
    }

    fn handle_submit_msg(
        &self,
        tx_info: TransferInfo,
        gw_status: oneshot::Sender<TxStatus>,
        commit_status: oneshot::Sender<TxStatus>,
    ) {
        let rollup_type_hash = self.rollup_type_hash.clone();
        let scripts_deployment = self.scripts_deployment.clone();
        let url = self.url.clone();
        let timeout = self.timeout;
        tokio::spawn(async move {
            let mut rpc_client = GodwokenRpcClient::new(url);
            let bytes = match build_transfer_req(
                tx_info,
                &mut rpc_client,
                &rollup_type_hash,
                &scripts_deployment,
            )
            .await
            .map(|req| JsonBytes::from_bytes(req.as_bytes()))
            {
                Ok(req) => req,
                Err(err) => {
                    log::trace!("build request error: {:?}", err);
                    let _ = gw_status.send(TxStatus::Failure);
                    return;
                }
            };

            match rpc_client.submit_l2transaction(bytes).await {
                Ok(tx) => {
                    log::debug!("submit tx: {}", hex::encode(&tx));
                    match wait_receipt(&tx, &mut rpc_client, timeout).await {
                        Ok(_) => {
                            let _ = gw_status.send(TxStatus::PendingCommit);
                            match wait_committed(&tx, &mut rpc_client, timeout).await {
                                Ok(_) => {
                                    let _ = commit_status.send(TxStatus::Committed(Some(tx)));
                                }
                                Err(_) => {
                                    let _ = commit_status.send(TxStatus::Timeout(tx));
                                }
                            };
                        }
                        Err(_) => {
                            let _ = gw_status.send(TxStatus::Timeout(tx));
                        }
                    };
                }
                Err(err) => {
                    log::trace!("submit l2 tx with error: {:?}", err);
                    let _ = gw_status.send(TxStatus::Failure);
                }
            }
        });
    }

    fn handle_execute_msg(&self, tx_info: TransferInfo, sender: oneshot::Sender<TxStatus>) {
        let rollup_type_hash = self.rollup_type_hash.clone();
        let scripts_deployment = self.scripts_deployment.clone();
        let url = self.url.clone();
        tokio::spawn(async move {
            let mut rpc_client = GodwokenRpcClient::new(url);
            let bytes = match build_transfer_req(
                tx_info,
                &mut rpc_client,
                &rollup_type_hash,
                &scripts_deployment,
            )
            .await
            .map(|req| JsonBytes::from_bytes(req.as_bytes()))
            {
                Ok(req) => req,
                Err(_) => {
                    let _ = sender.send(TxStatus::Failure);
                    return;
                }
            };
            let msg = match rpc_client.execute_l2transaction(bytes).await {
                Ok(_) => TxStatus::Committed(None),
                Err(_) => TxStatus::Failure,
            };
            let _ = sender.send(msg);
        });
    }
}

async fn transfer_handler(mut actor: TransferActor) {
    log::info!("transfer handler is running now");
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_msg(msg);
    }
}

#[derive(Clone)]
pub struct TransferHandler {
    sender: Sender<TransferMsg>,
}

impl TransferHandler {
    pub fn new(
        timeout: u64,
        url: Url,
        rollup_type_hash: H256,
        scripts_deployment: ScriptsDeploymentResult,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(200);
        let actor =
            TransferActor::new(timeout, url, rollup_type_hash, scripts_deployment, receiver);

        tokio::spawn(transfer_handler(actor));

        Self { sender }
    }

    // The return value is a two-stage-tuple of status.
    // First status is receipt status.
    // Second is Layer1 commit status.
    pub async fn submit(&self, tx_info: TransferInfo) -> Result<(TxStatus, Option<TxStatus>)> {
        let (gw_status, gw_recv) = oneshot::channel();
        let (commit_status, commit_recv) = oneshot::channel();
        let msg = TransferMsg::Submit {
            tx_info,
            gw_status,
            commit_status,
        };
        let _ = self.sender.send(msg).await;
        let gw_status = gw_recv.await?;
        let commit_status = match &gw_status {
            TxStatus::Failure => None,
            _ => Some(commit_recv.await?),
        };
        Ok((gw_status, commit_status))
    }

    pub async fn execute(&self, tx_info: TransferInfo) -> Result<(TxStatus, Option<TxStatus>)> {
        let (send, recv) = oneshot::channel();
        let msg = TransferMsg::Execute(tx_info, send);
        let _ = self.sender.send(msg).await;
        let tx_status = recv.await?;
        Ok((tx_status, None))
    }
}

async fn build_transfer_req(
    tx_info: TransferInfo,
    rpc_client: &mut GodwokenRpcClient,
    rollup_type_hash: &H256,
    scripts_deployment: &ScriptsDeploymentResult,
) -> Result<L2Transaction> {
    let TransferInfo {
        pk_from,
        pk_to,
        amount,
        fee,
        sudt_id,
        ..
    } = tx_info;
    let to_address = utils::privkey_to_short_address(&pk_to, rollup_type_hash, scripts_deployment)?;

    let _to_id = utils::short_address_to_account_id(rpc_client, &to_address).await?;
    // get from_id
    let from_address =
        utils::privkey_to_short_address(&pk_from, rollup_type_hash, scripts_deployment)?;

    let from_id = utils::short_address_to_account_id(rpc_client, &from_address).await?;
    let from_id = from_id.unwrap();

    let nonce = rpc_client.get_nonce(from_id).await?;
    log::debug!("build tx req for account: {} nonce: {}", &from_id, &nonce);

    let sudt_transfer = SUDTTransfer::new_builder()
        .to(GwPack::pack(&to_address))
        .amount(GwPack::pack(&amount))
        .fee(GwPack::pack(&fee))
        .build();

    let sudt_args = SUDTArgs::new_builder().set(sudt_transfer).build();

    let raw_l2transaction = RawL2Transaction::new_builder()
        .from_id(GwPack::pack(&from_id))
        .to_id(GwPack::pack(&sudt_id))
        .nonce(GwPack::pack(&nonce))
        .args(GwPack::pack(&sudt_args.as_bytes()))
        .build();

    let sender_script_hash = rpc_client.get_script_hash(from_id).await?;
    let receiver_script_hash = rpc_client.get_script_hash(sudt_id).await?;

    let message = utils::generate_transaction_message_to_sign(
        &raw_l2transaction,
        rollup_type_hash,
        &sender_script_hash,
        &receiver_script_hash,
    );
    let signature = utils::eth_sign(&message, pk_from)?;

    Ok(L2Transaction::new_builder()
        .raw(raw_l2transaction)
        .signature(signature.pack())
        .build())
}

async fn wait_receipt(tx: &H256, rpc_client: &mut GodwokenRpcClient, timeout: u64) -> Result<()> {
    let ts = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        if let Ok(res) = rpc_client.get_transaction_receipt(tx).await {
            match res {
                Some(_) => {
                    log::debug!("committed tx: {}", hex::encode(tx));
                    return Ok(());
                }
                None => {
                    if ts.elapsed().as_secs() > timeout {
                        return Err(anyhow!("Wait receipt timeout"));
                    }
                }
            }
        }
    }
}

async fn wait_committed(tx: &H256, rpc_client: &mut GodwokenRpcClient, timeout: u64) -> Result<()> {
    let ts = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        if let Ok(Some(tx_status)) = rpc_client.get_transaction(tx).await {
            if tx_status.status == L2TransactionStatus::Committed {
                return Ok(());
            }
        }
        if ts.elapsed().as_secs() > timeout {
            return Err(anyhow!("Wait committed timeout"));
        }
    }
}
