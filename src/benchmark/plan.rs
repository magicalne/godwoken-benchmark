use std::{cmp, collections::HashMap, time::Duration};

use ckb_fixed_hash::H256;

use anyhow::Result;
use ckb_jsonrpc_types::JsonBytes;
use rand::prelude::*;
use reqwest::Url;
use tokio::{sync::mpsc, time};

use crate::{api::godwoken_rpc::GodwokenRpcClient, types::ScriptsDeploymentResult, utils};

use super::{
    batch::BatchHandler,
    msg::{BatchResMsg, ReqMethod},
};

#[derive(Clone)]
pub struct Privkey {
    pub pk: H256,
    pub idx: usize,
}

pub struct GodwokenConfig {
    pub scripts_deployment: ScriptsDeploymentResult,
    pub url: Url,
    pub rollup_type_hash: H256,
}

impl GodwokenConfig {
    pub async fn new(
        scripts_deployment: ScriptsDeploymentResult,
        url: Url,
        rollup_type_hash: H256,
    ) -> Result<Self> {
        Ok(Self {
            scripts_deployment,
            url,
            rollup_type_hash,
        })
    }
}

pub struct Plan {
    // Send batch requests every {interval}ms
    interval: u64,
    // How many requsts in a batch of sending.
    req_batch_cnt: usize,
    batch_handler: BatchHandler,
    batch_res_receiver: mpsc::Receiver<BatchResMsg>,
    pks: Vec<(H256, Option<()>)>,
    rng: ThreadRng,
}

impl Plan {
    pub async fn new(
        interval: u64,
        pks_: Vec<H256>,
        gw_config: GodwokenConfig,
        req_batch_cnt: usize,
        batch_handler: BatchHandler,
        batch_res_receiver: mpsc::Receiver<BatchResMsg>,
    ) -> Self {
        let mut pk_map = HashMap::new();
        for pk in pks_.into_iter() {
            if let Ok((Some(account_id), balance)) = check_gw_balance(
                &pk,
                gw_config.url.clone(),
                &gw_config.scripts_deployment,
                &gw_config.rollup_type_hash,
            )
            .await
            {
                if balance > 100 {
                    log::trace!("account: {}, balance: {}", &account_id, &balance);
                    pk_map.insert(account_id, (pk, Some(())));
                }
            }
        }
        let pks: Vec<(H256, Option<()>)> = pk_map.into_values().collect();
        log::info!("Valid accounts: {}", pks.len());
        Self {
            interval,
            pks,
            req_batch_cnt,
            batch_handler,
            batch_res_receiver,
            rng: rand::thread_rng(),
        }
    }

    pub async fn run(&mut self) {
        log::info!("Plan running...");
        let req_freq = Duration::from_millis(self.interval);
        let mut interval = time::interval(req_freq);

        loop {
            if let Some(pks) = self.next_batch() {
                log::debug!("run next batch: {} requests", pks.len());
                let batch_handler = self.batch_handler.clone();
                if let Err(pks) = batch_handler.try_send_batch(pks, ReqMethod::Submit, 100, 1, 1) {
                    for pk in pks {
                        if let Some((_, avail)) = self.pks.get_mut(pk.idx) {
                            *avail = Some(())
                        }
                    }
                }
            } else {
                log::warn!("All privkeys are used in txs!");
            }

            if let Ok(msg) = self.batch_res_receiver.try_recv() {
                log::debug!("receive batch responses: {}", &msg.pk_idx_vec.len());
                for pk_idx in msg.pk_idx_vec {
                    if let Some((_, avali)) = self.pks.get_mut(pk_idx) {
                        *avali = Some(())
                    }
                }
            }
            interval.tick().await;
        }
    }

    fn next_batch(&mut self) -> Option<Vec<Privkey>> {
        let mut cnt = 0;
        let mut pks = Vec::new();
        let available_idx_vec: Vec<usize> = self
            .pks
            .iter()
            .enumerate()
            .filter(|(_, (_, a))| a.is_some())
            .map(|(idx, _)| idx)
            .collect();
        if available_idx_vec.is_empty() {
            return None;
        }
        let batch_cnt = cmp::min(available_idx_vec.len(), self.req_batch_cnt);
        loop {
            let nxt = self.rng.gen_range(0..available_idx_vec.len());
            let idx = available_idx_vec.get(nxt).unwrap();
            if let Some((pk, avail)) = self.pks.get_mut(*idx) {
                if avail.is_some() {
                    *avail = None;
                    pks.push(Privkey {
                        pk: pk.clone(),
                        idx: *idx,
                    });
                    cnt += 1;
                    if batch_cnt == cnt {
                        break;
                    }
                }
            }
        }
        Some(pks)
    }
}

pub async fn check_gw_balance(
    pk: &H256,
    url: reqwest::Url,
    scripts_deployment: &ScriptsDeploymentResult,
    rollup_type_hash: &H256,
) -> Result<(Option<u32>, u128)> {
    let mut rpc_client = GodwokenRpcClient::new(url);
    let short_address = utils::privkey_to_short_address(pk, rollup_type_hash, scripts_deployment)?;
    let account_id = utils::short_address_to_account_id(&mut rpc_client, &short_address).await?;
    let short_address = JsonBytes::from_bytes(short_address);
    let balance = rpc_client.get_balance(short_address, 1).await?;
    Ok((account_id, balance))
}
