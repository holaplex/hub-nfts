use hub_core::{prelude::*, uuid::Uuid};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, JoinType, QueryFilter, QuerySelect, RelationTrait,
    Set,
};

use crate::{
    db::Connection,
    entities::{
        collection_mints, collections, drops,
        prelude::Purchases,
        project_wallets, purchases,
        sea_orm_active_enums::{Blockchain, CreationStatus},
    },
    proto::{
        treasury_events::{
            Blockchain as ProtoBlockchainEnum, DropCreated, DropMinted, Event, ProjectWallet,
            TransactionStatus,
        },
        TreasuryEventKey,
    },
    Services,
};

/// Process the given message for various services.
///
/// # Errors
/// This functioncan return an error if it fails to process any event
pub async fn process(msg: Services, db: Connection) -> Result<()> {
    // match topics
    match msg {
        Services::Treasuries(key, e) => match e.event {
            Some(Event::DropCreated(payload)) => process_drop_created_event(db, key, payload).await,
            Some(Event::DropMinted(payload)) => process_drop_minted_event(db, key, payload).await,
            Some(Event::ProjectWalletCreated(payload)) => {
                process_project_wallet_created_event(db, payload).await
            },
            None | Some(_) => Ok(()),
        },
    }
}

/// Process a project wallet created event.
///
/// # Errors
/// This function can return an error in the following cases:
/// - Failed to parse UUID from string
/// - Failed to get blockchain enum variant
/// - Failed to insert project wallet into the database
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

/// Process a drop created event.
///
/// # Errors
/// This function can return an error in the following cases:
/// - Failed to parse transaction status from i32
/// - Failed to parse UUID from string
/// - Failed to load drop from the database
/// - Failed to update collection in the database
pub async fn process_drop_created_event(
    db: Connection,
    key: TreasuryEventKey,
    payload: DropCreated,
) -> Result<()> {
    let DropCreated {
        status,
        tx_signature,
        ..
    } = payload;

    let tx_status = TransactionStatus::from_i32(status).context("failed to parse the status")?;
    let drop_id = Uuid::from_str(&key.id)?;

    let (drop, collection_model) = drops::Entity::find_by_id(drop_id)
        .select_also(collections::Entity)
        .join(JoinType::InnerJoin, drops::Relation::Collections.def())
        .one(db.get())
        .await
        .context("failed to load drop from db")?
        .context("drop not found in db")?;

    let collection = collection_model.context("failed to get collection from db")?;
    let mut collection_active_model: collections::ActiveModel = collection.into();
    collection_active_model.signature = Set(Some(tx_signature));
    collection_active_model.update(db.get()).await?;

    let mut drops_active_model: drops::ActiveModel = drop.into();

    drops_active_model.creation_status = Set(tx_status.try_into()?);
    drops_active_model.update(db.get()).await?;

    debug!("status updated for drop {:?}", drop_id);

    Ok(())
}

/// Process a drop minted event.
///
/// # Errors
/// This function can return an error in the following cases:
/// - Failed to parse UUID from string
/// - Failed to parse transaction status from i32
/// - Failed to load or update collection mint or purchase from the database
pub async fn process_drop_minted_event(
    db: Connection,
    key: TreasuryEventKey,
    payload: DropMinted,
) -> Result<()> {
    let DropMinted {
        status,
        tx_signature,
        ..
    } = payload;

    let collection_mint_id = Uuid::from_str(&key.id)?;
    let tx_status = TransactionStatus::from_i32(status).context("failed to parse the status")?;

    let collection_mint = collection_mints::Entity::find_by_id(collection_mint_id)
        .one(db.get())
        .await
        .context("failed to load collection mint from db")?
        .context("collection mint not found in db")?;

    let mut collection_mint_active_model: collection_mints::ActiveModel = collection_mint.into();

    collection_mint_active_model.creation_status = Set(tx_status.try_into()?);
    collection_mint_active_model.signature = Set(Some(tx_signature.clone()));
    collection_mint_active_model.update(db.get()).await?;

    let purchase = Purchases::find()
        .filter(purchases::Column::MintId.eq(collection_mint_id))
        .one(db.get())
        .await
        .context("failed to load purchase from db")?
        .context("purchase not found in db")?;

    let mut purchase_am: purchases::ActiveModel = purchase.into();

    purchase_am.status = Set(tx_status.try_into()?);
    purchase_am.tx_signature = Set(Some(tx_signature));
    purchase_am.update(db.get()).await?;

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

impl TryFrom<TransactionStatus> for CreationStatus {
    type Error = Error;

    fn try_from(i: TransactionStatus) -> Result<Self> {
        match i {
            TransactionStatus::Unspecified => Err(anyhow!("Invalid enum variant")),
            TransactionStatus::Blocked => Ok(Self::Blocked),
            TransactionStatus::Failed => Ok(Self::Failed),
            TransactionStatus::Confirming => Ok(Self::Created),
            TransactionStatus::Cancelled => Ok(Self::Canceled),
            TransactionStatus::Rejected => Ok(Self::Rejected),
            _ => Ok(Self::Pending),
        }
    }
}
