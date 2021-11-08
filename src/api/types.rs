use crate::generated::packed;
use crate::prelude::*;
use crate::utils::new_blake2b;
use bytes::Bytes;
use ckb_fixed_hash::H256;
use ckb_jsonrpc_types::{JsonBytes, Uint32};
use ckb_types::prelude::{Builder, Entity};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct TxReceipt {
    pub tx_witness_hash: H256,
    pub post_state: AccountMerkleState,
    pub read_data_hashes: Vec<H256>,
    pub logs: Vec<LogItem>,
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AccountMerkleState {
    pub merkle_root: H256,
    pub count: Uint32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub struct LogItem {
    pub account_id: Uint32,
    // The actual type is `u8`
    pub service_flag: Uint32,
    pub data: JsonBytes,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct RunResult {
    // return data
    pub return_data: JsonBytes,
    // log data
    pub logs: Vec<LogItem>,
}

// impl From<offchain::RunResult> for RunResult {
//     fn from(data: offchain::RunResult) -> RunResult {
//         let offchain::RunResult {
//             return_data, logs, ..
//         } = data;
//         RunResult {
//             return_data: JsonBytes::from_vec(return_data),
//             logs: logs.into_iter().map(Into::into).collect(),
//         }
//     }
// }

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct RawL2Transaction {
    pub from_id: Uint32,
    pub to_id: Uint32,
    pub nonce: Uint32,
    pub args: JsonBytes,
}

impl From<RawL2Transaction> for packed::RawL2Transaction {
    fn from(tx: RawL2Transaction) -> Self {
        let RawL2Transaction {
            from_id,
            to_id,
            nonce,
            args,
        } = tx;
        let args: Bytes = args.into_bytes();
        packed::RawL2Transaction::new_builder()
            .from_id(u32::from(from_id).pack())
            .to_id(u32::from(to_id).pack())
            .nonce(u32::from(nonce).pack())
            .args(args.pack())
            .build()
    }
}

impl From<packed::RawL2Transaction> for RawL2Transaction {
    fn from(raw_l2_transaction: packed::RawL2Transaction) -> RawL2Transaction {
        let from_id: u32 = raw_l2_transaction.from_id().unpack();
        let to_id: u32 = raw_l2_transaction.to_id().unpack();
        let nonce: u32 = raw_l2_transaction.nonce().unpack();
        Self {
            from_id: from_id.into(),
            to_id: to_id.into(),
            nonce: nonce.into(),
            args: JsonBytes::from_bytes(raw_l2_transaction.args().unpack()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "snake_case")]
pub enum L2TransactionStatus {
    Pending,
    Committed,
}

impl Default for L2TransactionStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct L2Transaction {
    pub raw: RawL2Transaction,
    pub signature: JsonBytes,
}

impl From<L2Transaction> for packed::L2Transaction {
    fn from(tx: L2Transaction) -> Self {
        let L2Transaction { raw, signature } = tx;

        packed::L2Transaction::new_builder()
            .raw(raw.into())
            .signature(signature.into_bytes().pack())
            .build()
    }
}

impl From<packed::L2Transaction> for L2Transaction {
    fn from(l2_transaction: packed::L2Transaction) -> L2Transaction {
        Self {
            raw: l2_transaction.raw().into(),
            signature: JsonBytes::from_bytes(l2_transaction.signature().unpack()),
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "snake_case")]
pub struct L2TransactionView {
    #[serde(flatten)]
    pub inner: L2Transaction,
    pub hash: H256,
}

impl From<packed::L2Transaction> for L2TransactionView {
    fn from(l2_tx: packed::L2Transaction) -> L2TransactionView {
        let hash = H256::from(l2_tx.raw().hash());
        let inner = L2Transaction::from(l2_tx);
        L2TransactionView { inner, hash }
    }
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "snake_case")]
pub struct L2TransactionWithStatus {
    pub transaction: Option<L2TransactionView>,
    pub status: L2TransactionStatus,
}
macro_rules! impl_hash {
    ($struct:ident) => {
        impl<'a> packed::$struct<'a> {
            pub fn hash(&self) -> [u8; 32] {
                let mut hasher = new_blake2b();
                hasher.update(self.as_slice());
                let mut hash = [0u8; 32];
                hasher.finalize(&mut hash);
                hash
            }
        }
    };
}
macro_rules! impl_witness_hash {
    ($struct:ident) => {
        impl<'a> packed::$struct<'a> {
            pub fn hash(&self) -> [u8; 32] {
                self.raw().hash()
            }

            pub fn witness_hash(&self) -> [u8; 32] {
                let mut hasher = new_blake2b();
                hasher.update(self.as_slice());
                let mut hash = [0u8; 32];
                hasher.finalize(&mut hash);
                hash
            }
        }
    };
}
impl_hash!(RawL2TransactionReader);
impl_witness_hash!(L2TransactionReader);
impl packed::RawL2Transaction {
    pub fn hash(&self) -> [u8; 32] {
        self.as_reader().hash()
    }
}

impl packed::L2Transaction {
    pub fn hash(&self) -> [u8; 32] {
        self.raw().hash()
    }

    pub fn witness_hash(&self) -> [u8; 32] {
        self.as_reader().witness_hash()
    }
}
