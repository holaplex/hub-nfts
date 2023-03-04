use hub_core::{prelude::*, uuid::Uuid};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::{
    db::Connection,
    entities::{
        collection_mints, drops, project_wallets,
        sea_orm_active_enums::{Blockchain, CreationStatus},
    },
    proto::{
        treasury_events::{
            Blockchain as ProtoBlockchainEnum, DropCreated, DropMinted, Event, ProjectWallet,
        },
        TreasuryEventKey,
    },
    Services,
};

/// Res
///
/// # Errors
/// This function fails if ...
pub async fn process(msg: Services, db: Connection) -> Result<()> {
    // match topics
    match msg {
        Services::Treasuries(key, e) => match e.event {
            Some(Event::DropCreated(payload)) => update_drop_status(db, key, payload).await,
            Some(Event::DropMinted(payload)) => {
                update_collection_mint_status(db, key, payload).await
            },
            Some(Event::ProjectWalletCreated(payload)) => {
                process_project_wallet_created_event(db, payload).await
            },
            None | Some(_) => Ok(()),
        },
    }
}

/// Res
///
/// # Errors
/// This function fails if .
pub async fn process_project_wallet_created_event(
    db: Connection,
    payload: ProjectWallet,
) -> Result<()> {
    let project_id = Uuid::from_str(&payload.project_id)?;

    let blockchain = ProtoBlockchainEnum::from_i32(payload.blockchain)
        .context("failed to get blockchain enum variant")?;

    let active_model = project_wallets::ActiveModel {
        project_id: Set(project_id),
        wallet_address: Set(payload.wallet_address),
        blockchain: Set(blockchain.try_into()?),
        ..Default::default()
    };

    active_model
        .insert(db.get())
        .await
        .context("failed to insert project wallet")?;

    Ok(())
}

/// Res
///
/// # Errors
/// This function fails if .
pub async fn update_drop_status(
    db: Connection,
    key: TreasuryEventKey,
    payload: DropCreated,
) -> Result<()> {
    let drop_id = Uuid::from_str(&key.id)?;

    let drop = drops::Entity::find_by_id(drop_id)
        .one(db.get())
        .await
        .context("failed to load drop from db")?
        .context("drop not found in db")?;

    let mut drops_active_model: drops::ActiveModel = drop.into();

    drops_active_model.creation_status = Set(payload.status.into());
    drops_active_model.update(db.get()).await?;

    debug!("status updated for drop {:?}", drop_id);

    Ok(())
}

/// Res
///
/// # Errors
/// This function fails if .
pub async fn update_collection_mint_status(
    db: Connection,
    key: TreasuryEventKey,
    payload: DropMinted,
) -> Result<()> {
    let collection_mint_id = Uuid::from_str(&key.id)?;

    let collection_mint = collection_mints::Entity::find_by_id(collection_mint_id)
        .one(db.get())
        .await
        .context("failed to load collection mint from db")?
        .context("collection mint not found in db")?;

    let mut collection_mint_active_model: collection_mints::ActiveModel = collection_mint.into();

    collection_mint_active_model.creation_status = Set(payload.status.into());
    collection_mint_active_model.update(db.get()).await?;

    debug!(
        "status updated for collection mint {:?}",
        collection_mint_id
    );

    Ok(())
}

impl TryFrom<ProtoBlockchainEnum> for Blockchain {
    type Error = Error;

    fn try_from(v: ProtoBlockchainEnum) -> Result<Self> {
        match v {
            ProtoBlockchainEnum::Unspecified => Err(anyhow!("Invalid enum variant")),
            ProtoBlockchainEnum::Solana => Ok(Self::Solana),
            ProtoBlockchainEnum::Polygon => Ok(Self::Polygon),
            ProtoBlockchainEnum::Ethereum => Ok(Self::Ethereum),
        }
    }
}

impl From<i32> for CreationStatus {
    fn from(i: i32) -> Self {
        match i {
            10 => Self::Created,
            _ => Self::Pending,
        }
    }
}
