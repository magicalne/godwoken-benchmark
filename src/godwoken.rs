use std::ffi::OsStr;
use std::str::FromStr;
use std::{fs::File, io::BufReader, path::Path};

use anyhow::Result;
use bytes::Bytes;
use ckb_fixed_hash::H256;
use ckb_jsonrpc_types::JsonBytes;
use ckb_types::prelude::{Builder, Entity};
use serde::{Deserialize, Serialize};
use tokio::fs::read_dir;

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

pub async fn get_valid_accounts(
    accounts: Vec<Account>,
    rpc_client: &mut GodwokenRpcClient,
    scripts_deployment: &ScriptsDeploymentResult,
) -> Result<Vec<(u128, Account)>> {
    let mut vec = Vec::new();
    for account in &accounts {
        let balance = account.get_balance(rpc_client, scripts_deployment).await?;
        if balance > 0 {
            vec.push((balance, account.clone()))
        }
    }

    Ok(vec)
}
