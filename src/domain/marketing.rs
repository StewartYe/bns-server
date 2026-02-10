use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketingInfo {
    pub total_users: u64,
    pub total_online: u64,
    pub listed_count: u64,
    pub txs_24h: u64,
    pub vol_24h: u64,
    pub listed_value: u64,
}
