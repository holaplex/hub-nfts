use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::{
    chrono::{DateTime, Utc},
    producer::Producer,
};
use mpl_token_metadata::state::Creator;
use sea_orm::{prelude::*, JoinType, QuerySelect, Set};
use serde::{Deserialize, Serialize};

use crate::{
    blockchains::{
        solana::{CreateDropRequest, Solana},
        Blockchain, TransactionResponse,
    },
    collection::Collection,
    entities::{
        collections, drops,
        prelude::{Collections, Drops},
        project_wallets,
        sea_orm_active_enums::{Blockchain as BlockchainEnum, CreationStatus},
    },
    metadata_json::MetadataJson,
    objects::{CollectionCreator, Drop, MetadataJsonInput},
    proto::{self, nft_events, NftEventKey, NftEvents},
    AppContext, NftStorageClient, UserID,
};

#[derive(Default)]
pub struct Mutation;

#[Object(name = "DropMutation")]
impl Mutation {
    /// This mutation creates a new NFT drop and its associated collection. The drop returns immediately with a creation status of CREATING. You can [set up a webhook](https://docs.holaplex.dev/hub/For%20Developers/webhooks-overview) to receive a notification when the drop is ready to be minted.
    /// Error
    /// If the drop cannot be saved to the database or fails to be emitted for submission to the desired blockchain, the mutation will result in an error.
    pub async fn create_drop(
        &self,
        ctx: &Context<'_>,
        input: CreateDropInput,
    ) -> Result<CreateDropPayload> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let UserID(id) = user_id;
        let conn = db.get();
        let producer = ctx.data::<Producer<NftEvents>>()?;
        let solana = ctx.data::<Solana>()?;
        let nft_storage = ctx.data::<NftStorageClient>()?;

        let user_id = id.ok_or_else(|| Error::new("X-USER-ID header not found"))?;

        let wallet = project_wallets::Entity::find()
            .filter(
                project_wallets::Column::ProjectId
                    .eq(input.project)
                    .and(project_wallets::Column::Blockchain.eq(input.blockchain)),
            )
            .one(conn)
            .await?;

        let owner_address = wallet
            .ok_or_else(|| {
                Error::new(format!(
                    "no project wallet found for {} blockchain",
                    input.blockchain
                ))
            })?
            .wallet_address;

        let collection = Collection::new(collections::ActiveModel {
            blockchain: Set(input.blockchain),
            supply: Set(input.supply.map(|s| s.try_into().unwrap_or_default())),
            creation_status: Set(CreationStatus::Pending),
            ..Default::default()
        })
        .creators(input.creators.clone())
        .save(db)
        .await?;

        let metadata_json_model = MetadataJson::new(collection.id, input.metadata_json)
            .upload(nft_storage)
            .await?
            .save(db)
            .await?;

        let (
            collection_address,
            TransactionResponse {
                serialized_message,
                signed_message_signatures,
            },
        ) = match input.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .drop(CreateDropRequest {
                        owner_address,
                        creators: input
                            .creators
                            .into_iter()
                            .map(TryInto::try_into)
                            .collect::<Result<Vec<Creator>>>()?,
                        name: metadata_json_model.name,
                        symbol: metadata_json_model.symbol,
                        seller_fee_basis_points: input.seller_fee_basis_points.unwrap_or_default(),
                        supply: input.supply,
                        metadata_json_uri: metadata_json_model.uri,
                        collection: collection.id,
                    })
                    .await?
            },
            BlockchainEnum::Polygon | BlockchainEnum::Ethereum => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        let mut collection_am: collections::ActiveModel = collection.clone().into();

        collection_am.address = Set(Some(collection_address.to_string()));

        let collection = collection_am.update(conn).await?;

        let drop = drops::ActiveModel {
            project_id: Set(input.project),
            collection_id: Set(collection.id),
            creation_status: Set(CreationStatus::Pending),
            start_time: Set(input.start_time.map(|start_date| start_date.naive_utc())),
            end_time: Set(input.end_time.map(|end_date| end_date.naive_utc())),
            price: Set(input.price.unwrap_or_default().try_into()?),
            created_by: Set(user_id),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        let drop_model = drop.insert(conn).await?;

        emit_drop_transaction_event(
            producer,
            drop_model.id,
            user_id,
            serialized_message,
            signed_message_signatures,
            input.project,
        )
        .await?;

        Ok(CreateDropPayload {
            drop: Drop::new(drop_model, collection),
        })
    }

    /// This mutation allows for the temporary blocking of the minting of editions and can be resumed by calling the resumeDrop mutation.
    pub async fn pause_drop(
        &self,
        ctx: &Context<'_>,
        input: PauseDropInput,
    ) -> Result<PauseDropPayload> {
        let AppContext { db, .. } = ctx.data::<AppContext>()?;
        let conn = db.get();

        let (drop, collection) = Drops::find()
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .select_also(Collections)
            .filter(drops::Column::Id.eq(input.drop))
            .one(conn)
            .await?
            .ok_or_else(|| Error::new("drop not found"))?;

        let collection_model = collection
            .ok_or_else(|| Error::new(format!("no collection found for drop {}", input.drop)))?;

        let mut drops_active_model: drops::ActiveModel = drop.into();

        drops_active_model.paused_at = Set(Some(Utc::now().naive_utc()));
        let drop_model = drops_active_model.update(db.get()).await?;

        Ok(PauseDropPayload {
            drop: Drop::new(drop_model, collection_model),
        })
    }

    /// This mutation resumes a paused drop, allowing minting of editions to be restored
    pub async fn resume_drop(
        &self,
        ctx: &Context<'_>,
        input: ResumeDropInput,
    ) -> Result<ResumeDropPayload> {
        let AppContext { db, .. } = ctx.data::<AppContext>()?;
        let conn = db.get();

        let (drop, collection) = Drops::find()
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .select_also(Collections)
            .filter(drops::Column::Id.eq(input.drop))
            .one(conn)
            .await?
            .ok_or_else(|| Error::new("drop not found"))?;

        let collection_model = collection
            .ok_or_else(|| Error::new(format!("no collection found for drop {}", input.drop)))?;

        let mut drops_active_model: drops::ActiveModel = drop.into();

        drops_active_model.paused_at = Set(None);

        let drop_model = drops_active_model.update(db.get()).await?;

        Ok(ResumeDropPayload {
            drop: Drop::new(drop_model, collection_model),
        })
    }

    /// Shuts down a drop by writing the current UTC timestamp to the shutdown_at field of drop record.
    /// Returns the `Drop` object on success.
    ///
    /// # Errors
    /// Fails if the drop or collection is not found, or if updating the drop record fails.
    pub async fn shutdown_drop(
        &self,
        ctx: &Context<'_>,
        input: ShutdownDropInput,
    ) -> Result<ShutdownDropPayload> {
        let AppContext { db, .. } = ctx.data::<AppContext>()?;
        let conn = db.get();

        let (drop, collection) = Drops::find()
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .select_also(Collections)
            .filter(drops::Column::Id.eq(input.drop))
            .one(conn)
            .await?
            .ok_or_else(|| Error::new("drop not found"))?;

        let collection_model = collection
            .ok_or_else(|| Error::new(format!("no collection found for drop {}", input.drop)))?;

        let mut drops_active_model: drops::ActiveModel = drop.into();

        drops_active_model.shutdown_at = Set(Some(Utc::now().naive_utc()));

        let drop_model = drops_active_model.update(db.get()).await?;

        Ok(ShutdownDropPayload {
            drop: Drop::new(drop_model, collection_model),
        })
    }
}

/// This functions emits the drop transaction event
/// # Errors
/// This function fails if producer is unable to sent the event
async fn emit_drop_transaction_event(
    producer: &Producer<NftEvents>,
    id: Uuid,
    user_id: Uuid,
    serialized_message: Vec<u8>,
    signatures: Vec<String>,
    project_id: Uuid,
) -> Result<()> {
    let event = NftEvents {
        event: Some(nft_events::Event::CreateDrop(proto::DropTransaction {
            transaction: Some(proto::Transaction {
                serialized_message,
                signed_message_signatures: signatures,
            }),
            project_id: project_id.to_string(),
        })),
    };
    let key = NftEventKey {
        id: id.to_string(),
        user_id: user_id.to_string(),
    };

    producer.send(Some(&event), Some(&key)).await?;

    Ok(())
}

#[derive(Debug, Clone, SimpleObject)]
pub struct CreateDropPayload {
    drop: Drop,
}

#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct CreateDropInput {
    pub project: Uuid,
    pub price: Option<u64>,
    pub seller_fee_basis_points: Option<u16>,
    pub supply: Option<u64>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub blockchain: BlockchainEnum,
    pub creators: Vec<CollectionCreator>,
    pub metadata_json: MetadataJsonInput,
}

impl TryFrom<CollectionCreator> for Creator {
    type Error = Error;

    fn try_from(
        CollectionCreator {
            address,
            verified,
            share,
        }: CollectionCreator,
    ) -> Result<Self> {
        Ok(Self {
            address: address.parse()?,
            verified: verified.unwrap_or_default(),
            share,
        })
    }
}

/// Represents input fields for pausing a drop.
#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct PauseDropInput {
    pub drop: Uuid,
}
/// Represents the result of a successful pause drop mutation.
#[derive(Debug, Clone, SimpleObject)]
pub struct PauseDropPayload {
    /// The drop that has been paused.
    drop: Drop,
}
/// Represents input fields for resuming a paused drop.
#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct ResumeDropInput {
    pub drop: Uuid,
}
/// Represents the result of a successful resume drop mutation.
#[derive(Debug, Clone, SimpleObject)]
pub struct ResumeDropPayload {
    /// The drop that has been resumed.
    drop: Drop,
}

/// Represents the input fields for shutting down a drop
#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct ShutdownDropInput {
    pub drop: Uuid,
}

/// Represents the result of a successful shutdown drop mutation
#[derive(Debug, Clone, SimpleObject)]
pub struct ShutdownDropPayload {
    /// Drop that has been shutdown
    drop: Drop,
}
