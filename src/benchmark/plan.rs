use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use ckb_fixed_hash::H256;

use anyhow::Result;
use ckb_jsonrpc_types::JsonBytes;
use reqwest::Url;
use tokio::{sync::mpsc, time};

use crate::{
    api::godwoken_rpc::GodwokenRpcClient, benchmark::msg::Stats, types::ScriptsDeploymentResult,
    utils,
};

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
    stats: Stats,
    ts: Instant,
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
                    pk_map.insert(account_id, (pk, Some(())));
                }
            }
        }
        let pks: Vec<(H256, Option<()>)> = pk_map.into_values().collect();
        log::info!("Valid accounts: {}", pks.len());
        let stats = Stats {
            timeout: 0,
            failure: 0,
            committed: 0,
        };
        Self {
            ts: Instant::now(),
            interval,
            pks,
            req_batch_cnt,
            batch_handler,
            batch_res_receiver,
            stats,
        }
    }

    pub async fn run(&mut self) {
        log::info!("Plan running...");
        let req_freq = Duration::from_millis(self.interval);
        //print stats every 10s
        let mut timer = Instant::now();

        loop {
            if let Some(pks) = self.next_batch() {
                log::debug!("run next batch: {} requests", pks.len());
                let batch_handler = self.batch_handler.clone();
                tokio::spawn(async move {
                    batch_handler
                        .send_batch(pks, ReqMethod::Submit, 100, 1, 1)
                        .await;
                });
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
                let Stats {
                    failure,
                    timeout,
                    committed,
                } = msg.stats;
                self.stats.failure += failure;
                self.stats.timeout += timeout;
                self.stats.committed += committed;
            }
            time::interval(req_freq).tick().await;
            if timer.elapsed().as_secs() >= 10 {
                self.stats();
                timer = Instant::now();
            }
        }
    }

    fn next_batch(&mut self) -> Option<Vec<Privkey>> {
        let mut cnt = 0;
        let mut pks = Vec::new();
        for (idx, (pk, avail)) in self.pks.iter_mut().enumerate() {
            if avail.is_some() {
                *avail = None;
                pks.push(Privkey {
                    pk: pk.clone(),
                    idx,
                });
                cnt += 1;
                if self.req_batch_cnt == cnt {
                    break;
                }
            }
        }
        if pks.is_empty() {
            None
        } else {
            Some(pks)
        }
    }

    fn stats(&self) {
        let Stats {
            failure,
            timeout,
            committed,
        } = self.stats;
        let elapsed = self.ts.elapsed().as_secs();
        let tps = committed as f32 / (elapsed as f32);
        log::info!(
            "Stats -- for last {}s failure: {}, timeout: {}, committed: {}, tps: {}",
            elapsed,
            failure,
            timeout,
            committed,
            tps.round()
        );
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
