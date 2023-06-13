use hub_core::{
    credits::{CreditsClient, TransactionId},
    prelude::*,
    uuid::Uuid,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, JoinType, QueryFilter, QuerySelect, RelationTrait,
    Set,
};

use crate::{
    db::Connection,
    entities::{
        collection_mints, collections, drops, nft_transfers,
        prelude::Purchases,
        project_wallets, purchases,
        sea_orm_active_enums::{Blockchain, CreationStatus},
    },
    proto::{
        solana_nft_events::Event as SolanaNftsEvent,
        treasury_events::{
            Blockchain as ProtoBlockchainEnum, Event as TreasuryEvent, ProjectWallet,
            TransactionStatus,
        },
        SolanaCompletedTransaction, SolanaNftEventKey,
    },
    Actions, Services,
};

/// Process the given message for various services.
///
/// # Errors
/// This function can return an error if it fails to process any event
pub async fn process(msg: Services, db: Connection, credits: CreditsClient<Actions>) -> Result<()> {
    // match topics
    match msg {
        Services::Treasury(_key, e) => match e.event {
            Some(TreasuryEvent::ProjectWalletCreated(payload)) => {
                process_project_wallet_created_event(db, payload).await
            },
            None | Some(_) => Ok(()),
        },
        Services::Solana(SolanaNftEventKey { id, .. }, e) => match e.event {
            Some(SolanaNftsEvent::CreateDropSubmitted(SolanaCompletedTransaction {
                signature,
            })) => process_drop_created_event(db, credits, id, signature).await,
            Some(SolanaNftsEvent::MintDropSubmitted(SolanaCompletedTransaction { signature })) => {
                process_drop_minted_event(db, credits, id, signature).await
            },
            Some(SolanaNftsEvent::TransferAssetSubmitted(SolanaCompletedTransaction {
                signature,
            })) => process_mint_transferred_event(db, credits, id, signature).await,
            Some(SolanaNftsEvent::RetryMintDropSubmitted(SolanaCompletedTransaction {
                signature,
            })) => process_drop_minted_event(db, credits, id, signature).await,
            Some(SolanaNftsEvent::RetryCreateDropSubmitted(SolanaCompletedTransaction {
                signature,
            })) => process_drop_created_event(db, credits, id, signature).await,
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
    credits: CreditsClient<Actions>,
    drop_id: String,
    signature: String,
) -> Result<()> {
    let conn = db.get();
    let status = CreationStatus::Created;

    let drop_id = Uuid::from_str(&drop_id)?;

    let (drop, collection_model) = drops::Entity::find_by_id(drop_id)
        .select_also(collections::Entity)
        .join(JoinType::InnerJoin, drops::Relation::Collections.def())
        .one(conn)
        .await
        .context("failed to load drop from db")?
        .context("drop not found in db")?;

    let collection = collection_model.context("failed to get collection from db")?;
    let mut collection_active_model: collections::ActiveModel = collection.into();
    collection_active_model.signature = Set(Some(signature));
    collection_active_model.creation_status = Set(status);
    collection_active_model.update(conn).await?;

    let mut drops_active_model: drops::ActiveModel = drop.clone().into();

    drops_active_model.creation_status = Set(status);
    drops_active_model.update(conn).await?;

    let deduction_id = drop
        .credits_deduction_id
        .context("drop has no deduction id")?;
    credits
        .confirm_deduction(TransactionId(deduction_id))
        .await?;

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
    credits: CreditsClient<Actions>,
    collection_mint_id: String,
    signature: String,
) -> Result<()> {
    let collection_mint_id = Uuid::from_str(&collection_mint_id)?;
    let status = CreationStatus::Created;

    let collection_mint = collection_mints::Entity::find_by_id(collection_mint_id)
        .one(db.get())
        .await
        .context("failed to load collection mint from db")?
        .context("collection mint not found in db")?;

    let mut collection_mint_active_model: collection_mints::ActiveModel =
        collection_mint.clone().into();

    collection_mint_active_model.creation_status = Set(status);
    collection_mint_active_model.signature = Set(Some(signature.clone()));
    collection_mint_active_model.update(db.get()).await?;

    let purchase = Purchases::find()
        .filter(purchases::Column::MintId.eq(collection_mint_id))
        .one(db.get())
        .await
        .context("failed to load purchase from db")?
        .context("purchase not found in db")?;

    let mut purchase_am: purchases::ActiveModel = purchase.into();

    purchase_am.status = Set(status);
    purchase_am.tx_signature = Set(Some(signature));
    purchase_am.update(db.get()).await?;

    let deduction_id = collection_mint
        .credits_deduction_id
        .context("deduction id not found")?;

    credits
        .confirm_deduction(TransactionId(deduction_id))
        .await?;

    Ok(())
}

/// Processes a `MintTransfered` event and updates the corresponding entities in the database.
///
/// # Arguments
///
/// * `db` - A database connection object used to interact with the database.
/// * `key` - A `TreasuryEventKey` representing the key of the event.
/// * `payload` - A `MintTransfered` struct representing the payload of the event.
/// # Errors
///
/// This function returns an error if it fails to update the mint owner or
/// if it fails to inserts the nft transfer

pub async fn process_mint_transferred_event(
    db: Connection,
    credits: CreditsClient<Actions>,
    transfer_id: String,
    signature: String,
) -> Result<()> {
    let conn = db.get();
    let transfer_id = Uuid::from_str(&transfer_id)?;

    let (nft_transfer, collection_mint) = nft_transfers::Entity::find_by_id(transfer_id)
        .find_also_related(collection_mints::Entity)
        .one(conn)
        .await?
        .context("failed to load nft transfer from db")?;

    let collection_mint = collection_mint.ok_or(anyhow!("collection mint not found in db"))?;

    let mut collection_mint_am: collection_mints::ActiveModel = collection_mint.into();
    collection_mint_am.owner = Set(nft_transfer.recipient.clone());
    collection_mint_am.update(db.get()).await?;

    let mut nft_transfer_am: nft_transfers::ActiveModel = nft_transfer.clone().into();
    nft_transfer_am.tx_signature = Set(Some(signature));

    nft_transfer_am.insert(db.get()).await?;

    let deduction_id = nft_transfer
        .credits_deduction_id
        .context("deduction id not found")?;

    credits
        .confirm_deduction(TransactionId(deduction_id))
        .await?;

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
            TransactionStatus::Completed => Ok(Self::Created),
            TransactionStatus::Cancelled => Ok(Self::Canceled),
            TransactionStatus::Rejected => Ok(Self::Rejected),
            _ => Ok(Self::Pending),
        }
    }
}
