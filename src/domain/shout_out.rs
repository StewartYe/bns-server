use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShoutOutRequest {
    pub psbt_hex: String,
    pub ad_words: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ShoutOut {
    pub tx_id: String,
    pub listing_name: String,
    pub user_address: String,
    pub ad_words: String,
    pub status: String,
    pub price: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ShoutOutStatus {
    Pending,
    Confirmed,
}

impl Display for ShoutOutStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShoutOutStatus::Pending => write!(f, "pending"),
            ShoutOutStatus::Confirmed => write!(f, "confirmed"),
        }
    }
}

#[test]
pub fn test_shout_out() {
    let t = ShoutOut {
        tx_id: "2222222".to_string(),
        listing_name: "".to_string(),
        user_address: "tbcq......".to_string(),
        ad_words: "THE FUTURE IS NOW, BUY THE DIP".to_string(),
        status: "confirmed".to_string(),
        price: 111111111,
        created_at: Default::default(),
        updated_at: Default::default(),
    };
    println!("{}", serde_json::to_string_pretty(&t).unwrap());
}
