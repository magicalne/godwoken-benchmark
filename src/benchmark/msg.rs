use super::plan::Privkey;

pub struct BatchReqMsg {
    pub pks: Vec<Privkey>,
    pub method: ReqMethod,
    pub amount: u128,
    pub fee: u128,
    pub sudt_id: u32,
}

pub enum ReqMethod {
    Submit,
    Execute,
}

pub struct BatchResMsg {
    pub pk_idx_vec: Vec<usize>,
    pub stats: Stats,
}

pub struct Stats {
    pub timeout: usize,
    pub failure: usize,
    pub committed: usize,
}
