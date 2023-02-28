use hub_core::{prelude::*, uuid::Uuid};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::{
    db::Connection,
    entities::{collection_mints, drops, sea_orm_active_enums::CreationStatus},
    proto::{
        treasury_events::{self, DropCreated, DropMinted},
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
            Some(treasury_events::Event::DropCreated(payload)) => {
                update_drop_status(db, key, payload).await
            },
            Some(treasury_events::Event::DropMinted(payload)) => {
                update_collection_mint_status(db, key, payload).await
            },
            None | Some(_) => Ok(()),
        },
    }
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

impl From<i32> for CreationStatus {
    fn from(i: i32) -> Self {
        match i {
            10 => Self::Created,
            _ => Self::Pending,
        }
    }
}
