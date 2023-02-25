use hub_core::{prelude::*, uuid::Uuid};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::{
    db::Connection,
    entities::{collection_mints, drops, sea_orm_active_enums::CreationStatus},
    proto::{treasury_events, TreasuryEventKey},
    Services,
};

/// Res
///
/// # Errors
/// This function fails if ...
pub async fn process(msg: Services, db: Connection) -> Result<()> {
    // match topics
    match msg {
        Services::Treasuries(k, e) => match e.event {
            Some(treasury_events::Event::MasterEdition(status)) => {
                update_drop_status(k, status, db).await
            },
            Some(treasury_events::Event::MintEdition(status)) => {
                update_collection_mint_status(k, status, db).await
            },
            None => Ok(()),
        },
    }
}

/// Res
///
/// # Errors
/// This function fails if .
pub async fn update_drop_status(k: TreasuryEventKey, status: i32, db: Connection) -> Result<()> {
    let drop_id = Uuid::from_str(&k.id)?;

    let drop = drops::Entity::find_by_id(drop_id)
        .one(db.get())
        .await
        .context("failed to load drop from db")?
        .context("drop not found in db")?;

    let mut drops_active_model: drops::ActiveModel = drop.into();

    drops_active_model.creation_status = Set(status.into());
    drops_active_model.update(db.get()).await?;

    debug!("status updated for drop {:?}", drop_id);

    Ok(())
}

/// Res
///
/// # Errors
/// This function fails if .
pub async fn update_collection_mint_status(
    k: TreasuryEventKey,
    status: i32,
    db: Connection,
) -> Result<()> {
    let collection_mint_id = Uuid::from_str(&k.id)?;

    let collection_mint = collection_mints::Entity::find_by_id(collection_mint_id)
        .one(db.get())
        .await
        .context("failed to load collection mint from db")?
        .context("collection mint not found in db")?;

    let mut collection_mint_active_model: collection_mints::ActiveModel = collection_mint.into();

    collection_mint_active_model.creation_status = Set(status.into());
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
