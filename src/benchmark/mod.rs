pub mod batch;
pub mod msg;
pub mod plan;

use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    str::FromStr,
};

use anyhow::Result;
use ckb_fixed_hash::H256;
use tokio::{fs::read_dir, sync::mpsc};

use crate::utils::{read_privkey, read_scripts_deployment};

use self::plan::GodwokenConfig;

pub async fn run(
    interval: u64,
    req_batch_cnt: usize,
    accounts_path: impl AsRef<Path>,
    url: &str,
    scripts_deployment_path: impl AsRef<Path>,
    rollup_type_hash: String,
) -> Result<()> {
    let mut dir = read_dir(accounts_path).await?;
    let mut pks = Vec::new();
    let url = reqwest::Url::parse(url)?;
    let scripts_deployment = read_scripts_deployment(scripts_deployment_path)?;
    let rollup_type_hash = H256::from_str(&rollup_type_hash)?;
    while let Some(f) = dir.next_entry().await? {
        let file = File::open(f.path())?;
        let reader = BufReader::new(file);
        if let Some(Ok(pk)) = reader.lines().next() {
            if let Ok(pk) = read_privkey(pk) {
                pks.push(pk);
            }
        }
    }

    let transfer_handler = crate::tx::transfer::TransferHandler::new(
        url.clone(),
        rollup_type_hash.clone(),
        scripts_deployment.clone(),
    );

    let (batch_res_sender, batch_res_receiver) = mpsc::channel(20);
    let batch_handler = batch::BatchHandler::new(transfer_handler, batch_res_sender);
    let gw_config = GodwokenConfig {
        scripts_deployment,
        url,
        rollup_type_hash,
    };
    let mut plan = plan::Plan::new(
        interval,
        pks,
        gw_config,
        req_batch_cnt,
        batch_handler,
        batch_res_receiver,
    )
    .await;
    plan.run().await;
    Ok(())
}
