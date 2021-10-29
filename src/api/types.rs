use ckb_fixed_hash::H256;
use ckb_jsonrpc_types::{JsonBytes, Uint32};
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
