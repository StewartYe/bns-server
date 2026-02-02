use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShoutOutRequest {
    pub psbt_hex: String,
    pub ad_words: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShoutOut {
    pub id: String,
    pub listing_name: String,
    pub user: String,
    pub ad_words: String,
    pub status: String,
    pub price: u64,
    pub created_at: String,
    pub updated_at: String,
}
