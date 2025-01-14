use crate::context::AppContext;
use crate::database::Database;
use crate::model::qualified_identity::QualifiedIdentity;
use crate::model::wallet::{Wallet, WalletSeedHash};
use dash_sdk::dpp::identity::accessors::IdentityGettersV0;
use dash_sdk::platform::Identifier;
use rusqlite::params;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard};

impl Database {
    /// Updates the alias of a specified identity.
    pub fn set_alias(
        &self,
        identifier: &Identifier,
        new_alias: Option<&str>,
    ) -> rusqlite::Result<()> {
        let id = identifier.to_vec();
        let conn = self.conn.lock().unwrap();

        let rows_updated = conn.execute(
            "UPDATE identity SET alias = ? WHERE id = ?",
            params![new_alias, id],
        )?;

        if rows_updated == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }

        Ok(())
    }
    pub fn insert_local_qualified_identity(
        &self,
        qualified_identity: &QualifiedIdentity,
        wallet_and_identity_id_info: Option<(&[u8], u32)>,
        app_context: &AppContext,
    ) -> rusqlite::Result<()> {
        let id = qualified_identity.identity.id().to_vec();
        let data = qualified_identity.to_bytes();
        let alias = qualified_identity.alias.clone();
        let identity_type = format!("{:?}", qualified_identity.identity_type);

        let network = app_context.network_string();

        if let Some((wallet, wallet_index)) = wallet_and_identity_id_info {
            // If wallet information is provided, insert with wallet and wallet_index
            self.execute(
                "INSERT OR REPLACE INTO identity
             (id, data, is_local, alias, identity_type, network, wallet, wallet_index)
             VALUES (?, ?, 1, ?, ?, ?, ?, ?)",
                params![
                    id,
                    data,
                    alias,
                    identity_type,
                    network,
                    wallet,
                    wallet_index
                ],
            )?;
        } else {
            // If wallet information is not provided, insert without wallet and wallet_index
            self.execute(
                "INSERT OR REPLACE INTO identity
             (id, data, is_local, alias, identity_type, network)
             VALUES (?, ?, 1, ?, ?, ?)",
                params![id, data, alias, identity_type, network],
            )?;
        }

        Ok(())
    }

    pub fn update_local_qualified_identity(
        &self,
        qualified_identity: &QualifiedIdentity,
        app_context: &AppContext,
    ) -> rusqlite::Result<()> {
        // Extract the fields from `qualified_identity` to use in the SQL update
        let id = qualified_identity.identity.id().to_vec();
        let data = qualified_identity.to_bytes();
        let alias = qualified_identity.alias.clone();
        let identity_type = format!("{:?}", qualified_identity.identity_type);

        // Get the network string from the app context
        let network = app_context.network_string();

        // Execute the update statement
        self.execute(
            "UPDATE identity
         SET data = ?, alias = ?, identity_type = ?, network = ?, is_local = 1
         WHERE id = ?",
            params![data, alias, identity_type, network, id],
        )?;

        Ok(())
    }

    pub fn insert_local_qualified_identity_in_creation(
        &self,
        qualified_identity: &QualifiedIdentity,
        wallet_id: &[u8],
        identity_index: u32,
        app_context: &AppContext,
    ) -> rusqlite::Result<()> {
        let id = qualified_identity.identity.id().to_vec();
        let data = qualified_identity.to_bytes();
        let alias = qualified_identity.alias.clone();
        let identity_type = format!("{:?}", qualified_identity.identity_type);

        let network = app_context.network_string();

        self.execute(
            "INSERT OR REPLACE INTO identity
         (id, data, is_local, alias, identity_type, network, is_in_creation, wallet, wallet_index)
         VALUES (?, ?, 1, ?, ?, ?, 1, ?, ?)",
            params![
                id,
                data,
                alias,
                identity_type,
                network,
                wallet_id,
                identity_index
            ],
        )?;

        Ok(())
    }

    pub fn insert_remote_identity_if_not_exists(
        &self,
        identifier: &Identifier,
        qualified_identity: Option<&QualifiedIdentity>,
        app_context: &AppContext,
    ) -> rusqlite::Result<()> {
        let id = identifier.to_vec();
        let alias = qualified_identity.and_then(|qi| qi.alias.clone());
        let identity_type =
            qualified_identity.map_or("".to_string(), |qi| format!("{:?}", qi.identity_type));
        let data = qualified_identity.map(|qi| qi.to_bytes());

        let network = app_context.network_string();

        // Check if the identity already exists
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT COUNT(*) FROM identity WHERE id = ? AND network = ?")?;
        let count: i64 = stmt.query_row(params![id, network], |row| row.get(0))?;

        // If the identity doesn't exist, insert it
        if count == 0 {
            self.execute(
                "INSERT INTO identity (id, data, is_local, alias, identity_type, network)
             VALUES (?, ?, 0, ?, ?, ?)",
                params![id, data, alias, identity_type, network],
            )?;
        }

        Ok(())
    }

    pub fn get_local_qualified_identities(
        &self,
        app_context: &AppContext,
        wallets: &BTreeMap<WalletSeedHash, Arc<RwLock<Wallet>>>,
    ) -> rusqlite::Result<Vec<QualifiedIdentity>> {
        let network = app_context.network_string();

        let conn = self.conn.lock().unwrap();

        // Prepare the main statement to select identities, including wallet_index
        let mut stmt = conn.prepare(
            "SELECT data, alias, wallet_index FROM identity WHERE is_local = 1 AND network = ? AND data IS NOT NULL",
        )?;

        // Prepare the statement to select top-ups (will be used multiple times)
        let mut top_up_stmt =
            conn.prepare("SELECT top_up_index, amount FROM top_up WHERE identity_id = ?")?;

        // Iterate over each identity
        let identity_iter = stmt.query_map(params![network], |row| {
            let data: Vec<u8> = row.get(0)?;
            let alias: Option<String> = row.get(1)?;
            let wallet_index: Option<u32> = row.get(2)?;

            let mut identity: QualifiedIdentity = QualifiedIdentity::from_bytes(&data);
            identity.alias = alias;
            identity.wallet_index = wallet_index;

            // Associate wallets
            identity.associated_wallets = wallets.clone(); //todo: use less wallets

            // Retrieve the identity_id as bytes
            let identity_id = identity.identity.id().to_buffer();

            // Query the top_up table for this identity_id
            let mut top_ups = BTreeMap::new();
            let mut rows = top_up_stmt.query(params![identity_id])?;

            while let Some(top_up_row) = rows.next()? {
                let top_up_index: u32 = top_up_row.get(0)?;
                let amount: u32 = top_up_row.get(1)?;
                top_ups.insert(top_up_index, amount);
            }

            // Assign the top_ups to the identity
            identity.top_ups = top_ups;

            Ok(identity)
        })?;

        let identities: rusqlite::Result<Vec<QualifiedIdentity>> = identity_iter.collect();
        identities
    }

    pub fn get_local_voting_identities(
        &self,
        app_context: &AppContext,
    ) -> rusqlite::Result<Vec<QualifiedIdentity>> {
        let network = app_context.network_string();

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT data FROM identity WHERE is_local = 1 AND network = ? AND identity_type != 'User' AND data IS NOT NULL",
        )?;
        let identity_iter = stmt.query_map(params![network], |row| {
            let data: Vec<u8> = row.get(0)?;
            let identity: QualifiedIdentity = QualifiedIdentity::from_bytes(&data);

            Ok(identity)
        })?;

        let identities: rusqlite::Result<Vec<QualifiedIdentity>> = identity_iter.collect();
        identities
    }

    pub fn get_local_user_identities(
        &self,
        app_context: &AppContext,
    ) -> rusqlite::Result<Vec<QualifiedIdentity>> {
        let network = app_context.network_string();

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT data FROM identity WHERE is_local = 1 AND network = ? AND identity_type = 'User' AND data IS NOT NULL",
        )?;
        let identity_iter = stmt.query_map(params![network], |row| {
            let data: Vec<u8> = row.get(0)?;
            let identity: QualifiedIdentity = QualifiedIdentity::from_bytes(&data);

            Ok(identity)
        })?;

        let identities: rusqlite::Result<Vec<QualifiedIdentity>> = identity_iter.collect();
        identities
    }

    /// Deletes a local qualified identity with the given identifier from the database.
    pub fn delete_local_qualified_identity(
        &self,
        identifier: &Identifier,
        app_context: &AppContext,
    ) -> rusqlite::Result<()> {
        let id = identifier.to_vec();
        let network = app_context.network_string();

        let conn = self.conn.lock().unwrap();

        // Perform the deletion only if the identity is marked as local
        conn.execute(
            "DELETE FROM identity WHERE id = ? AND network = ? AND is_local = 1",
            params![id, network],
        )?;

        Ok(())
    }
}
