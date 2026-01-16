//! Name service
//!
//! Handles name resolution and address lookup by proxying to Ord backend.

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{AppError, Result};
use crate::infra::{DynBlockchainClient, DynPostgresClient};

// ============================================================================
// Service response types
// ============================================================================

/// Name resolution result
#[derive(Debug, Clone)]
pub struct NameInfo {
    pub name: String,
    pub address: String,
    pub id: String,
    pub inscription_id: String,
    pub inscription_number: u64,
    pub confirmations: u64,
    pub metadata: HashMap<String, String>,
}

/// Name entry for address lookup
#[derive(Debug, Clone)]
pub struct NameEntry {
    pub name: String,
    pub id: String,
    pub is_primary: bool,
    pub confirmations: u64,
}

/// Address names result with pagination
#[derive(Debug, Clone)]
pub struct AddressNamesResult {
    pub address: String,
    pub names: Vec<NameEntry>,
    pub page: u32,
    pub page_size: u32,
    pub total: u32,
}

// ============================================================================
// Name service
// ============================================================================

/// Name service
pub struct NameService {
    blockchain: DynBlockchainClient,
    postgres: DynPostgresClient,
}

impl NameService {
    pub fn new(blockchain: DynBlockchainClient, postgres: DynPostgresClient) -> Self {
        Self {
            blockchain,
            postgres,
        }
    }

    /// Forward resolution: name -> address
    ///
    /// Calls Ord backend and enriches with metadata from database.
    pub async fn get_name(&self, name: &str) -> Result<NameInfo> {
        let ord_data = self
            .blockchain
            .ord_bns_rune(name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Name '{}' not found", name)))?;

        // Fetch metadata from database
        let mut metadata = HashMap::new();
        if let Ok(Some(db_metadata)) = self.postgres.get_name_metadata(name).await {
            if let Some(desc) = db_metadata.description {
                metadata.insert("description".to_string(), desc);
            }
            if let Some(url) = db_metadata.url {
                metadata.insert("url".to_string(), url);
            }
            if let Some(twitter) = db_metadata.twitter {
                metadata.insert("twitter".to_string(), twitter);
            }
            if let Some(email) = db_metadata.email {
                metadata.insert("email".to_string(), email);
            }
        }

        Ok(NameInfo {
            name: name.to_string(),
            address: ord_data.address,
            id: ord_data.rune_id,
            inscription_id: ord_data.inscription_id,
            inscription_number: ord_data.inscription_number,
            confirmations: ord_data.confirmations,
            metadata,
        })
    }

    /// Reverse resolution: address -> names
    ///
    /// Calls Ord backend and returns all names owned by the address with pagination.
    pub async fn get_address_names(
        &self,
        address: &str,
        page: u32,
        page_size: u32,
    ) -> Result<AddressNamesResult> {
        let ord_data = self.blockchain.ord_bns_address(address).await?;

        let total = ord_data.runes.len() as u32;
        let page = page.max(1);
        let page_size = page_size.clamp(1, 100);

        // Get user's primary name from database
        let primary_name = self
            .postgres
            .get_user(address)
            .await
            .ok()
            .flatten()
            .and_then(|u| u.primary_name);

        // Calculate pagination bounds
        let start = ((page - 1) * page_size) as usize;
        let end = (start + page_size as usize).min(ord_data.runes.len());

        // Map to NameEntry with is_primary flag
        let names: Vec<NameEntry> = if start < ord_data.runes.len() {
            ord_data.runes[start..end]
                .iter()
                .map(|rune| NameEntry {
                    name: rune.rune_name.clone(),
                    id: rune.rune_id.clone(),
                    is_primary: primary_name.as_ref().is_some_and(|p| p == &rune.rune_name),
                    confirmations: rune.confirmations,
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok(AddressNamesResult {
            address: address.to_string(),
            names,
            page,
            page_size,
            total,
        })
    }
}

pub type DynNameService = Arc<NameService>;
