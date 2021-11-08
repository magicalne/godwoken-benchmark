use std::{io::ErrorKind, path::Path, str::FromStr};

use crate::{
    api::godwoken_rpc::GodwokenRpcClient,
    generated::packed::{Byte32, RawL2Transaction, Script},
};
pub use blake2b_ref::{Blake2b, Blake2bBuilder};
use ckb_crypto::secp::Privkey;
use ckb_fixed_hash::H256;
use ckb_jsonrpc_types::JsonBytes;

use crate::prelude::Pack as GwPack;
use crate::types::ScriptsDeploymentResult;
use ckb_types::{
    bytes::Bytes as CKBBytes, core::ScriptHashType, prelude::Builder as CKBBuilder,
    prelude::Entity as CKBEntity,
};
use molecule::bytes::Bytes as GwBytes;
use sha3::{Digest, Keccak256};

use anyhow::Result;

lazy_static::lazy_static! {
    /// The reference to lazily-initialized static secp256k1 engine, used to execute all signature operations
    pub static ref SECP256K1: secp256k1::Secp256k1<secp256k1::All> = secp256k1::Secp256k1::new();
}

pub fn l2_script_hash_to_short_address(script_hash: &H256) -> GwBytes {
    let short_address = &script_hash.as_bytes()[..20];

    GwBytes::from(short_address.to_vec())
}

pub fn privkey_to_short_address(
    privkey: &H256,
    rollup_type_hash: &H256,
    scripts_deployment: &ScriptsDeploymentResult,
) -> Result<GwBytes> {
    let script_hash = privkey_to_l2_script_hash(privkey, rollup_type_hash, scripts_deployment)?;

    let short_address = l2_script_hash_to_short_address(&script_hash);
    Ok(short_address)
}

pub fn privkey_to_eth_address(privkey: &H256) -> Result<CKBBytes> {
    let privkey = secp256k1::SecretKey::from_slice(privkey.as_bytes())?;
    let pubkey = secp256k1::PublicKey::from_secret_key(&SECP256K1, &privkey);
    let pubkey_hash = {
        let mut hasher = Keccak256::new();
        hasher.update(&pubkey.serialize_uncompressed()[1..]);
        let buf = hasher.finalize();
        let mut pubkey_hash = [0u8; 20];
        pubkey_hash.copy_from_slice(&buf[12..]);
        pubkey_hash
    };
    let s = CKBBytes::from(pubkey_hash.to_vec());
    Ok(s)
}

pub fn privkey_to_l2_script_hash(
    privkey: &H256,
    rollup_type_hash: &H256,
    scripts_deployment: &ScriptsDeploymentResult,
) -> Result<H256> {
    let eth_address = privkey_to_eth_address(privkey)?;

    let code_hash = Byte32::from_slice(
        scripts_deployment
            .eth_account_lock
            .script_type_hash
            .as_bytes(),
    )?;

    let mut args_vec = rollup_type_hash.as_bytes().to_vec();
    args_vec.append(&mut eth_address.to_vec());
    let b = &GwBytes::from(args_vec);
    let args = GwPack::pack(b);

    let script = Script::new_builder()
        .code_hash(code_hash)
        .hash_type(ScriptHashType::Type.into())
        .args(args)
        .build();

    let script_hash = CkbHasher::new().update(script.as_slice()).finalize();

    Ok(script_hash)
}

pub fn generate_transaction_message_to_sign(
    raw_l2transaction: &RawL2Transaction,
    rollup_type_hash: &H256,
    sender_script_hash: &H256,
    receiver_script_hash: &H256,
) -> H256 {
    let raw_data = raw_l2transaction.as_slice();
    let rollup_type_hash_data = rollup_type_hash.as_bytes();

    let digest = CkbHasher::new()
        .update(rollup_type_hash_data)
        .update(sender_script_hash.as_bytes())
        .update(receiver_script_hash.as_bytes())
        .update(raw_data)
        .finalize();

    let message = EthHasher::new()
        .update("\x19Ethereum Signed Message:\n32")
        .update(digest.as_bytes())
        .finalize();

    message
}

fn sign_message(msg: &H256, privkey_data: H256) -> anyhow::Result<[u8; 65]> {
    let privkey = Privkey::from(privkey_data);
    let signature = privkey.sign_recoverable(msg)?;
    let mut inner = [0u8; 65];
    inner.copy_from_slice(&signature.serialize());
    Ok(inner)
}

pub fn eth_sign(msg: &H256, privkey: H256) -> anyhow::Result<[u8; 65]> {
    let mut signature = sign_message(msg, privkey)?;
    let v = &mut signature[64];
    if *v >= 27 {
        *v -= 27;
    }
    Ok(signature)
}

pub async fn short_address_to_account_id(
    godwoken_rpc_client: &mut GodwokenRpcClient,
    short_address: &GwBytes,
) -> Result<Option<u32>> {
    let bytes = JsonBytes::from_bytes(short_address.clone());
    let script_hash = match godwoken_rpc_client
        .get_script_hash_by_short_address(bytes)
        .await?
    {
        Some(h) => Ok(h),
        None => Err(std::io::Error::new(
            ErrorKind::Other,
            format!(
                "script hash by short address: 0x{} not found",
                hex::encode(short_address.to_vec()),
            ),
        )),
    };
    let account_id = godwoken_rpc_client
        .get_account_id_by_script_hash(script_hash?)
        .await?;

    Ok(account_id)
}

pub fn read_privkey(privkey: String) -> Result<H256> {
    Ok(H256::from_str(privkey.trim().trim_start_matches("0x"))?)
}

pub fn read_scripts_deployment(path: impl AsRef<Path>) -> anyhow::Result<ScriptsDeploymentResult> {
    let scripts_deployment_content = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&scripts_deployment_content)?)
}

pub struct CkbHasher {
    hasher: Blake2b,
}

impl CkbHasher {
    pub fn new() -> Self {
        Self {
            hasher: new_blake2b(),
        }
    }

    pub fn update(mut self, data: &[u8]) -> Self {
        self.hasher.update(data);
        self
    }

    pub fn finalize(self) -> H256 {
        let mut hash = [0u8; 32];
        self.hasher.finalize(&mut hash);
        hash.into()
    }
}

impl Default for CkbHasher {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EthHasher {
    hasher: Keccak256,
}

impl EthHasher {
    pub fn new() -> Self {
        Self {
            hasher: Keccak256::new(),
        }
    }

    pub fn update(mut self, data: impl AsRef<[u8]>) -> Self {
        self.hasher.update(data);
        self
    }

    pub fn finalize(self) -> H256 {
        let buf = self.hasher.finalize();
        let mut result = [0u8; 32];
        result.copy_from_slice(&buf[..]);
        result.into()
    }
}

impl Default for EthHasher {
    fn default() -> Self {
        Self::new()
    }
}

pub const BLAKE2B_KEY: &[u8] = &[];
pub const BLAKE2B_LEN: usize = 32;
pub const CKB_PERSONALIZATION: &[u8] = b"ckb-default-hash";

pub fn new_blake2b() -> Blake2b {
    Blake2bBuilder::new(32)
        .personal(CKB_PERSONALIZATION)
        .build()
}
