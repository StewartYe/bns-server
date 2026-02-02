use candid::Deserialize;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fmt::Display;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Star {
    pub id: i32,
    pub user_address: String,
    pub target: String,
    pub target_type: StarTargetType,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarResponse {
    pub user_address: String,
    pub target: String,
    pub target_type: String,
}

impl From<Star> for StarResponse {
    fn from(star: Star) -> Self {
        Self {
            user_address: star.user_address,
            target: star.target,
            target_type: star.target_type.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
pub enum StarTargetType {
    Name,
    Collector,
}

impl Display for StarTargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StarTargetType::Name => write!(f, "name"),
            StarTargetType::Collector => write!(f, "collector"),
        }
    }
}
