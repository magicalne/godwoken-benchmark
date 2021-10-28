use crate::generated::packed;
use crate::prelude::*;
use ckb_fixed_hash::H256;
use ckb_jsonrpc_types::{CellDep, JsonBytes, Script, Uint32, Uint64};
use serde::{Deserialize, Serialize};
use sparse_merkle_tree::H256 as SMT_H256;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug, Default)]
pub struct SetupConfig {
    pub l1_sudt_script_type_hash: H256,
    pub l1_sudt_cell_dep: CellDep,
    pub node_initial_ckb: u64,
    pub cells_lock: Script,
    pub burn_lock: Script,
    pub reward_lock: Script,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct RollupConfig {
    pub l1_sudt_script_type_hash: H256,
    pub custodian_script_type_hash: H256,
    pub deposit_script_type_hash: H256,
    pub withdrawal_script_type_hash: H256,
    pub challenge_script_type_hash: H256,
    pub stake_script_type_hash: H256,
    pub l2_sudt_validator_script_type_hash: H256,
    pub burn_lock_hash: H256,
    pub required_staking_capacity: Uint64,
    pub challenge_maturity_blocks: Uint64,
    pub finality_blocks: Uint64,
    pub reward_burn_rate: Uint32,           // * reward_burn_rate / 100
    pub allowed_eoa_type_hashes: Vec<H256>, // list of script code_hash allowed an EOA(external owned account) to use
    pub allowed_contract_type_hashes: Vec<H256>, // list of script code_hash allowed a contract account to use
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenesisConfig {
    pub timestamp: u64,
    pub rollup_type_hash: H256,
    pub meta_contract_validator_type_hash: H256,
    pub rollup_config: RollupConfig,
    // For load secp data and use in challenge transaction
    pub secp_data_dep: CellDep,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug, Default)]
pub struct RollupDeploymentResult {
    pub tx_hash: H256,
    pub timestamp: u64,
    pub rollup_type_hash: H256,
    pub rollup_type_script: ckb_jsonrpc_types::Script,
    pub rollup_config: RollupConfig,
    pub rollup_config_cell_dep: ckb_jsonrpc_types::CellDep,
    pub layer2_genesis_hash: H256,
    pub genesis_config: GenesisConfig,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Default)]
pub struct PoAConfig {
    pub poa_setup: PoASetup,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Default)]
pub struct PoASetup {
    pub identity_size: u8,
    pub round_interval_uses_seconds: bool,
    pub identities: Vec<JsonBytes>,
    pub aggregator_change_threshold: u8,
    pub round_intervals: u32,
    pub subblocks_per_round: u32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Default)]
pub struct UserRollupConfig {
    pub l1_sudt_script_type_hash: H256,
    pub l1_sudt_cell_dep: CellDep,
    pub cells_lock: Script,
    pub burn_lock: Script,
    pub reward_lock: Script,
    pub required_staking_capacity: u64,
    pub challenge_maturity_blocks: u64,
    pub finality_blocks: u64,
    pub reward_burn_rate: u8, // * reward_burn_rate / 100
    pub allowed_eoa_type_hashes: Vec<H256>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Default)]
pub struct DeployItem {
    pub script_type_hash: H256,
    pub cell_dep: CellDep,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Default)]
pub struct ScriptsDeploymentResult {
    pub custodian_lock: DeployItem,
    pub deposit_lock: DeployItem,
    pub withdrawal_lock: DeployItem,
    pub challenge_lock: DeployItem,
    pub stake_lock: DeployItem,
    pub state_validator: DeployItem,
    pub meta_contract_validator: DeployItem,
    pub l2_sudt_validator: DeployItem,
    pub eth_account_lock: DeployItem,
    pub tron_account_lock: DeployItem,
    pub polyjuice_validator: DeployItem,
    pub state_validator_lock: DeployItem,
    pub poa_state: DeployItem,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct BuildScriptsResult {
    pub programs: Programs,
    pub lock: Script,
    pub built_scripts: HashMap<String, PathBuf>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Default)]
pub struct Programs {
    // path: godwoken-scripts/build/release/custodian-lock
    pub custodian_lock: PathBuf,
    // path: godwoken-scripts/build/release/deposit-lock
    pub deposit_lock: PathBuf,
    // path: godwoken-scripts/build/release/withdrawal-lock
    pub withdrawal_lock: PathBuf,
    // path: godwoken-scripts/build/release/challenge-lock
    pub challenge_lock: PathBuf,
    // path: godwoken-scripts/build/release/stake-lock
    pub stake_lock: PathBuf,
    // path: godwoken-scripts/build/release/state-validator
    pub state_validator: PathBuf,
    // path: godwoken-scripts/c/build/sudt-validator
    pub l2_sudt_validator: PathBuf,

    // path: godwoken-scripts/c/build/account_locks/eth-account-lock
    pub eth_account_lock: PathBuf,
    // path: godwoken-scripts/c/build/account_locks/tron-account-lock
    pub tron_account_lock: PathBuf,

    // path: godwoken-scripts/c/build/meta-contract-validator
    pub meta_contract_validator: PathBuf,
    // path: godwoken-polyjuice/build/validator
    pub polyjuice_validator: PathBuf,

    // path: clerkb/build/debug/poa.strip
    pub state_validator_lock: PathBuf,
    // path: clerkb/build/debug/state.strip
    pub poa_state: PathBuf,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RecoverAccount {
    pub message: H256,
    pub signature: Vec<u8>,
    pub lock_script: Script,
}

#[derive(Debug, Clone, Default)]
pub struct RunResult {
    pub read_values: HashMap<SMT_H256, SMT_H256>,
    pub write_values: HashMap<SMT_H256, SMT_H256>,
    pub return_data: Vec<u8>,
    pub account_count: Option<u32>,
    pub recover_accounts: HashSet<RecoverAccount>,
    pub new_scripts: HashMap<SMT_H256, Vec<u8>>,
    pub get_scripts: HashSet<Vec<u8>>,
    pub write_data: HashMap<SMT_H256, Vec<u8>>,
    // data hash -> data full size
    pub read_data: HashMap<SMT_H256, Vec<u8>>,
    // log data
    pub logs: Vec<packed::LogItem>,
    // used cycles
    pub used_cycles: u64,
    pub exit_code: i8,
}

impl packed::CellOutput {
    pub fn occupied_capacity(&self, data_capacity: usize) -> ckb_types::core::CapacityResult<u64> {
        let output = ckb_types::packed::CellOutput::new_unchecked(self.as_bytes());
        output
            .occupied_capacity(ckb_types::core::Capacity::bytes(data_capacity)?)
            .map(|c| c.as_u64())
    }
}

impl std::hash::Hash for packed::Script {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_reader().as_slice().hash(state)
    }
}
