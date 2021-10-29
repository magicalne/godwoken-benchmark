use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::BufRead;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::{self, Duration, Instant};
use std::{fs::File, io::BufReader, path::Path};

use anyhow::Result;
use bytes::Bytes;
use ckb_fixed_hash::H256;
use ckb_jsonrpc_types::JsonBytes;
use ckb_types::prelude::{Builder, Entity};
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::fs::read_dir;
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::api::godwoken_rpc::GodwokenRpcClient;
use crate::generated::packed::{L2Transaction, RawL2Transaction, SUDTArgs, SUDTTransfer};
use crate::prelude::Pack as GwPack;
use crate::types::ScriptsDeploymentResult;
use crate::utils::{self, read_privkey};

const ROLLUP_TYPE_HASH: &str = "af937c85c4c794f165b0cc168c6524b7f70dec05ff478063c99dcc9ce7903fd7";

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct Account {
    mainnet: String,
    testnet: String,
    lock_arg: String,
    privkey: String,
}

impl Account {
    pub async fn get_balance(
        &self,
        rpc_client: &mut GodwokenRpcClient,
        scripts_deployment: &ScriptsDeploymentResult,
    ) -> Result<u128> {
        let privkey = read_privkey(self.privkey.clone())?;
        let rollup_type_hash = H256::from_str(ROLLUP_TYPE_HASH)?;
        let addr =
            utils::privkey_to_short_address(&privkey, &rollup_type_hash, scripts_deployment)?;
        let addr = JsonBytes::from_bytes(addr);
        rpc_client.get_balance(addr, 1).await
    }

    pub async fn transfer_sudt(
        &self,
        rpc_client: &mut GodwokenRpcClient,
        scripts_deployment: &ScriptsDeploymentResult,
        amount: u128,
        fee: u128,
        to_account: &Account,
        sudt_id: u32,
    ) -> Result<H256> {
        log::info!("from: {:?} to {:?}", &self.testnet, &to_account.testnet);
        let privkey = read_privkey(self.privkey.clone())?;

        let rollup_type_hash = H256::from_str(ROLLUP_TYPE_HASH)?;

        let to_privkey = read_privkey(to_account.privkey.clone())?;
        let to_address =
            utils::privkey_to_short_address(&to_privkey, &rollup_type_hash, scripts_deployment)?;

        let to_id = utils::short_address_to_account_id(rpc_client, &to_address).await?;
        // get from_id
        let from_address =
            utils::privkey_to_short_address(&privkey, &rollup_type_hash, scripts_deployment)?;

        let from_id = utils::short_address_to_account_id(rpc_client, &from_address).await?;
        log::info!("from {:?} to {:?} transfer {:?}", from_id, to_id, amount);
        let from_id = from_id.unwrap();

        let nonce = rpc_client.get_nonce(from_id).await?;

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
            &rollup_type_hash,
            &sender_script_hash,
            &receiver_script_hash,
        );
        let signature = utils::eth_sign(&message, privkey)?;

        let l2_transaction = L2Transaction::new_builder()
            .raw(raw_l2transaction)
            .signature(signature.pack())
            .build();

        let bytes = JsonBytes::from_bytes(l2_transaction.as_bytes());
        let tx_hash = rpc_client.submit_l2transaction(bytes).await?;

        Ok(tx_hash)
    }
}

pub struct Godwoken {
    accounts: Vec<Account>,
    rpc_client: GodwokenRpcClient,
    scripts_deployment: ScriptsDeploymentResult,
}

impl Godwoken {
    pub async fn new(
        path: impl AsRef<Path>,
        url: &str,
        scripts_deployment_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let mut dir = read_dir(path).await?;
        let mut accounts = Vec::new();
        while let Some(f) = dir.next_entry().await? {
            if f.path().extension() == Some(OsStr::new("json")) {
                let file = File::open(f.path())?;
                let reader = BufReader::new(file);

                // Read the JSON contents of the file as an instance of `User`.
                let account = serde_json::from_reader(reader)?;
                accounts.push(account);
            }
        }

        let url = reqwest::Url::parse(url)?;
        let rpc_client = GodwokenRpcClient::new(url);

        let scripts_deployment = utils::read_scripts_deployment(scripts_deployment_path)?;

        log::info!("Total accounts: {:?}", accounts.len());

        Ok(Godwoken {
            accounts,
            rpc_client,
            scripts_deployment,
        })
    }

    pub fn get_url(&self) -> reqwest::Url {
        self.rpc_client.get_url()
    }

    pub async fn check_balance(&mut self, min_amount: u128) -> Result<()> {
        let mut cnt = 0;
        for account in &self.accounts {
            let balance = account
                .get_balance(&mut self.rpc_client, &self.scripts_deployment)
                .await?;
            if balance < min_amount {
                cnt += 1;
                log::warn!(
                    "Acount: {:?} doesn't have enough ckb! cnt: {:?}",
                    balance,
                    cnt
                );
            }
        }
        Ok(())
    }

    pub async fn transfer_to_first_empty_account(&mut self) -> Result<()> {
        let (from_account, to_account) = {
            let mut from_account = None;
            let mut to_account = None;
            for (idx, account) in self.accounts.iter().enumerate() {
                let balance = account
                    .get_balance(&mut self.rpc_client, &self.scripts_deployment)
                    .await?;
                if balance > 100 && from_account.is_none() {
                    from_account = Some(account);
                }

                if balance == 0 && to_account.is_none() {
                    to_account = Some(account)
                }

                if from_account.is_some() && to_account.is_some() {
                    break;
                }
            }
            (from_account, to_account)
        };
        if from_account.is_some() && to_account.is_some() {
            let tx = from_account
                .unwrap()
                .transfer_sudt(
                    &mut self.rpc_client,
                    &self.scripts_deployment,
                    100,
                    0,
                    to_account.unwrap(),
                    1,
                )
                .await?;
            log::debug!("TX: {:?}", hex::encode(&tx));
            let receipt = self.rpc_client.get_transaction_receipt(&tx).await?;
            log::debug!("TX: {:?}", receipt);
        } else {
            log::error!("Cannot continue!")
        }

        Ok(())
    }

    pub async fn get_account_id_by_short_address(
        &mut self,
        short_address: &'static str,
    ) -> Result<Option<u32>> {
        let short_address = Bytes::from(short_address);
        utils::short_address_to_account_id(&mut self.rpc_client, &short_address).await
    }
}

struct CallbackMsg {
    tx: Option<H256>,
    ts: time::Instant,
}

pub struct Plan {
    // Send batch requests every {interval}ms
    interval: u128,
    // How many requsts in a batch of sending.
    req_batch_cnt: usize,
    current_idx: usize,
    scripts_deployment: ScriptsDeploymentResult,
    // Godwoken url
    url: reqwest::Url,
    sender: Sender<CallbackMsg>,
    pks: Vec<H256>,
}

impl Plan {
    pub async fn new(
        interval: u128,
        req_batch_cnt: usize,
        path: impl AsRef<Path>,
        url: &str,
        scripts_deployment_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let mut dir = read_dir(path).await?;
        let mut pks = Vec::new();
        let url_ = reqwest::Url::parse(url)?;
        let scripts_deployment = utils::read_scripts_deployment(scripts_deployment_path)?;
        while let Some(f) = dir.next_entry().await? {
            let file = File::open(f.path())?;
            let reader = BufReader::new(file);
            if let Some(Ok(pk)) = reader.lines().next() {
                if let Ok(pk) = read_privkey(pk) {
                    if let Ok(balance) =
                        check_gw_balance(&pk, url_.clone(), &scripts_deployment).await
                    {
                        log::info!("pk: {:?} balance: {:?}", hex::encode(&pk), balance);
                        if balance > 100 {
                            pks.push(pk);
                        }
                    }
                }
            }
        }
        let (callback_sender, callback_receiver) = mpsc::channel(200);
        let (tx_status_sender, tx_status_receiver) = mpsc::channel(200);
        tokio::spawn(async move {
            let mut stats = Stats::new();
            stats.run(tx_status_receiver).await;
        });

        let url = url_.clone();
        tokio::spawn(async move {
            TxStatsCollector::new(callback_receiver, tx_status_sender, url)
                .collect()
                .await;
        });

        Ok(Self {
            interval,
            req_batch_cnt,
            current_idx: 0,
            scripts_deployment,
            url: url_,
            pks,
            sender: callback_sender,
        })
    }

    pub async fn run(&mut self) {
        let mut now = Instant::now();
        loop {
            if now.elapsed().as_millis() < self.interval {
                continue;
            }
            now = Instant::now();
            let tx_info = self.next_batch();
            tokio::spawn(send_batch(
                tx_info,
                self.scripts_deployment.clone(),
                self.url.clone(),
                self.sender.clone(),
            ));
        }
    }

    fn next_batch(&mut self) -> Vec<TransferInfo> {
        let right_bound = self.current_idx + self.req_batch_cnt;
        let from = if right_bound < self.pks.len() {
            let batch_vec = Vec::from(&self.pks[self.current_idx..right_bound - 1]);
            self.current_idx = right_bound;
            batch_vec
        } else {
            let batch_vec = Vec::from(&self.pks[self.current_idx..]);
            self.current_idx = 0;
            batch_vec
        };
        let mut to = from.clone();
        to.rotate_right(1);
        from.into_iter()
            .zip(to.into_iter())
            .into_iter()
            .map(|(pk_from, pk_to)| TransferInfo {
                pk_from,
                pk_to,
                amount: 100,
                fee: 1,
                sudt_id: 1,
            })
            .collect()
    }
}

struct TransferInfo {
    pk_from: H256,
    pk_to: H256,
    amount: u128,
    fee: u128,
    sudt_id: u32,
}

async fn send_batch(
    trans: Vec<TransferInfo>,
    scripts_deployment: ScriptsDeploymentResult,
    url: reqwest::Url,
    callback: Sender<CallbackMsg>,
) {
    let rpc_client = GodwokenRpcClient::new(url);
    stream::iter(trans)
        .for_each(|t| {
            let mut rpc_client = rpc_client.clone();
            let scripts_deployment = scripts_deployment.clone();
            let callback = callback.clone();
            async move {
                let _ = send_req(t, &mut rpc_client, &scripts_deployment, callback);
            }
        })
        .await;
}
async fn send_req(
    transfer_info: TransferInfo,
    rpc_client: &mut GodwokenRpcClient,
    scripts_deployment: &ScriptsDeploymentResult,
    callback: Sender<CallbackMsg>,
) -> Result<()> {
    let rollup_type_hash = H256::from_str(ROLLUP_TYPE_HASH)?;

    let TransferInfo {
        pk_from,
        pk_to,
        amount,
        fee,
        sudt_id,
    } = transfer_info;

    let to_address =
        utils::privkey_to_short_address(&pk_to, &rollup_type_hash, scripts_deployment)?;

    let to_id = utils::short_address_to_account_id(rpc_client, &to_address).await?;
    // get from_id
    let from_address =
        utils::privkey_to_short_address(&pk_from, &rollup_type_hash, scripts_deployment)?;

    let from_id = utils::short_address_to_account_id(rpc_client, &from_address).await?;
    let from_id = from_id.unwrap();

    let nonce = rpc_client.get_nonce(from_id).await?;

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
        &rollup_type_hash,
        &sender_script_hash,
        &receiver_script_hash,
    );
    let signature = utils::eth_sign(&message, pk_from)?;

    let l2_transaction = L2Transaction::new_builder()
        .raw(raw_l2transaction)
        .signature(signature.pack())
        .build();

    let bytes = JsonBytes::from_bytes(l2_transaction.as_bytes());
    let res = rpc_client.submit_l2transaction(bytes).await;
    let _ = callback
        .send(CallbackMsg {
            tx: res.ok(),
            ts: Instant::now(),
        })
        .await;
    Ok(())
}

struct TxStatsCollector {
    receiver: Receiver<CallbackMsg>,
    ts: time::Instant,
    url: reqwest::Url,
    receipt_callback: Sender<TxStatus>,
}

impl TxStatsCollector {
    fn new(
        receiver: Receiver<CallbackMsg>,
        receipt_callback: Sender<TxStatus>,
        url: reqwest::Url,
    ) -> Self {
        Self {
            receiver,
            ts: time::Instant::now(),
            receipt_callback,
            url,
        }
    }

    async fn collect(&mut self) {
        while let Some(msg) = self.receiver.recv().await {
            match msg.tx {
                Some(tx) => {
                    let url = self.url.clone();
                    let sender = self.receipt_callback.clone();
                    let _ = sender.send(TxStatus::Success).await;
                    let sender = self.receipt_callback.clone();
                    tokio::spawn(refresh_receipt(url, tx, msg.ts, sender));
                }
                None => {
                    let sender = self.receipt_callback.clone();
                    let _ = sender.send(TxStatus::Failure).await;
                }
            };
        }
    }
}

#[derive(Debug)]
struct Stats {
    start_ts: Instant,
    success: usize,
    failure: usize,
    timeout: usize,
    committed: usize,
}

impl Stats {
    fn new() -> Self {
        Self {
            start_ts: Instant::now(),
            success: 0,
            failure: 0,
            timeout: 0,
            committed: 0,
        }
    }

    async fn run(&mut self, mut receiver: Receiver<TxStatus>) {
        let mut timer = Instant::now();
        while let Some(tx_status) = &mut receiver.recv().await {
            match tx_status {
                TxStatus::Success => self.success += 1,
                TxStatus::Failure => self.failure += 1,
                TxStatus::Timeout => self.timeout += 1,
                TxStatus::Committed => self.committed += 1,
            };
            if timer.elapsed().as_secs() > 5 {
                log::info!("stats: {:?}", &self);
                timer = Instant::now();
            }
        }
    }
}

enum TxStatus {
    Success,
    Failure,
    Timeout,
    Committed,
}

pub async fn check_gw_balance(
    pk: &H256,
    url: reqwest::Url,
    scripts_deployment: &ScriptsDeploymentResult,
) -> Result<u128> {
    let rollup_type_hash = H256::from_str(ROLLUP_TYPE_HASH)?;
    let addr = utils::privkey_to_short_address(pk, &rollup_type_hash, scripts_deployment)?;
    let addr = JsonBytes::from_bytes(addr);
    let mut rpc_client = GodwokenRpcClient::new(url);
    rpc_client.get_balance(addr, 1).await
}

async fn refresh_receipt(url: reqwest::Url, tx: H256, ts: Instant, sender: Sender<TxStatus>) {
    log::info!("Starting refresh_receipt");
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    let mut rpc_client = GodwokenRpcClient::new(url);
    loop {
        interval.tick().await;
        if let Ok(res) = rpc_client.get_transaction_receipt(&tx).await {
            match res {
                Some(_) => {
                    let _ = sender.send(TxStatus::Committed).await;
                    return;
                }
                None => {
                    if ts.elapsed().as_secs() > 120 {
                        let _ = sender.send(TxStatus::Timeout).await;
                        return;
                    }
                }
            }
        }
    }
}
