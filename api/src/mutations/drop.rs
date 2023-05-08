use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::{anyhow, chrono::Utc, credits::CreditsClient, producer::Producer};
use mpl_token_metadata::state::Creator;
use sea_orm::{prelude::*, JoinType, ModelTrait, QuerySelect, Set, TransactionTrait};
use serde::{Deserialize, Serialize};

use crate::{
    blockchains::{
        solana::{CreateDropRequest, RetryDropRequest, Solana, UpdateEditionRequest},
        Edition, TransactionResponse,
    },
    collection::Collection,
    db::Connection,
    entities::{
        collection_creators, collections, drops, metadata_jsons,
        prelude::{Collections, Drops},
        project_wallets,
        sea_orm_active_enums::{Blockchain as BlockchainEnum, CreationStatus},
    },
    metadata_json::MetadataJson,
    objects::{CollectionCreator, Drop, MetadataJsonInput},
    proto::{self, nft_events, NftEventKey, NftEvents},
    Actions, AppContext, NftStorageClient, OrganizationId, UserID,
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
        let AppContext {
            db,
            user_id,
            organization_id,
            balance,
            ..
        } = ctx.data::<AppContext>()?;
        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;
        let conn = db.get();
        let producer = ctx.data::<Producer<NftEvents>>()?;
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let solana = ctx.data::<Solana>()?;
        let nft_storage = ctx.data::<NftStorageClient>()?;

        let user_id = id.ok_or_else(|| Error::new("X-USER-ID header not found"))?;
        let org_id = org.ok_or_else(|| Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or_else(|| Error::new("X-CREDIT-BALANCE header not found"))?;

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
        let seller_fee_basis_points = input.seller_fee_basis_points.unwrap_or_default();

        let collection = Collection::new(collections::ActiveModel {
            blockchain: Set(input.blockchain),
            supply: Set(input.supply.map(TryFrom::try_from).transpose()?),
            creation_status: Set(CreationStatus::Pending),
            seller_fee_basis_points: Set(seller_fee_basis_points.try_into()?),
            ..Default::default()
        })
        .creators(input.creators.clone())
        .save(db)
        .await?;

        let metadata_json = MetadataJson::new(input.metadata_json)
            .upload(nft_storage)
            .await?
            .save(collection.id, db)
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
                    .edition()
                    .create(CreateDropRequest {
                        owner_address,
                        creators: input
                            .creators
                            .into_iter()
                            .map(TryInto::try_into)
                            .collect::<Result<Vec<Creator>>>()?,
                        collection: collection.clone(),
                        metadata_json,
                    })
                    .await?
            },
            BlockchainEnum::Polygon | BlockchainEnum::Ethereum => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        let mut collection_am: collections::ActiveModel = collection.clone().try_into()?;

        collection_am.address = Set(Some(collection_address.to_string()));

        let collection = collection_am.update(conn).await?;

        let drop = drops::ActiveModel {
            project_id: Set(input.project),
            collection_id: Set(collection.id),
            creation_status: Set(CreationStatus::Pending),
            start_time: Set(input.start_time),
            end_time: Set(input.end_time),
            price: Set(input.price.unwrap_or_default().try_into()?),
            created_by: Set(user_id),
            created_at: Set(Utc::now().into()),
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
            input.blockchain,
        )
        .await?;

        submit_pending_deduction(
            credits,
            db,
            balance,
            user_id,
            org_id,
            drop_model.id,
            input.blockchain,
            Actions::CreateDrop,
        )
        .await?;

        Ok(CreateDropPayload {
            drop: Drop::new(drop_model, collection),
        })
    }

    /// This mutation retries an existing drop.
    /// The drop returns immediately with a creation status of CREATING.
    /// You can [set up a webhook](https://docs.holaplex.dev/hub/For%20Developers/webhooks-overview) to receive a notification when the drop is ready to be minted.
    /// Errors
    /// The mutation will fail if the drop and its related collection cannot be located,
    /// if the transaction response cannot be built,
    /// or if the transaction event cannot be emitted.
    pub async fn retry_drop(
        &self,
        ctx: &Context<'_>,
        input: RetryDropInput,
    ) -> Result<CreateDropPayload> {
        let AppContext {
            db,
            user_id,
            organization_id,
            balance,
            ..
        } = ctx.data::<AppContext>()?;
        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;
        let producer = ctx.data::<Producer<NftEvents>>()?;
        let solana = ctx.data::<Solana>()?;
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let user_id = id.ok_or_else(|| Error::new("X-USER-ID header not found"))?;
        let org_id = org.ok_or_else(|| Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or_else(|| Error::new("X-ORGANIZATION-BALANCE header not found"))?;
        let (drop, collection) = Drops::find_by_id(input.drop)
            .find_also_related(Collections)
            .one(db.get())
            .await?
            .ok_or_else(|| Error::new("drop not found"))?;

        let collection = collection.ok_or_else(|| Error::new("collection not found"))?;

        if drop.creation_status == CreationStatus::Created {
            return Err(Error::new("drop already created"));
        }

        let (
            collection_address,
            TransactionResponse {
                serialized_message,
                signed_message_signatures,
            },
        ) = match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .edition()
                    .retry_drop(RetryDropRequest {
                        collection: collection.clone(),
                    })
                    .await?
            },
            BlockchainEnum::Polygon | BlockchainEnum::Ethereum => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        let mut collection_am: collections::ActiveModel = collection.clone().try_into()?;
        collection_am.address = Set(Some(collection_address.to_string()));
        let collection = collection_am.update(db.get()).await?;

        emit_retry_drop_event(
            producer,
            input.drop,
            user_id,
            serialized_message,
            signed_message_signatures,
            drop.project_id,
            collection.blockchain,
        )
        .await?;

        submit_pending_deduction(
            credits,
            db,
            balance,
            user_id,
            org_id,
            drop.id,
            collection.blockchain,
            Actions::RetryDrop,
        )
        .await?;

        Ok(CreateDropPayload {
            drop: Drop::new(drop, collection),
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

        drops_active_model.paused_at = Set(Some(Utc::now().into()));
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

        drops_active_model.shutdown_at = Set(Some(Utc::now().into()));

        let drop_model = drops_active_model.update(db.get()).await?;

        Ok(ShutdownDropPayload {
            drop: Drop::new(drop_model, collection_model),
        })
    }

    /// This mutation allows updating a drop and it's associated collection by ID.
    /// It returns an error if it fails to reach the database, emit update events or assemble the on-chain transaction.
    /// Returns the `PatchDropPayload` object on success.
    pub async fn patch_drop(
        &self,
        ctx: &Context<'_>,
        input: PatchDropInput,
    ) -> Result<PatchDropPayload> {
        let PatchDropInput {
            id,
            price,
            start_time,
            end_time,
            seller_fee_basis_points,
            metadata_json,
            creators,
        } = input;

        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let conn = db.get();
        let producer = ctx.data::<Producer<NftEvents>>()?;
        let nft_storage = ctx.data::<NftStorageClient>()?;
        let solana = ctx.data::<Solana>()?;

        let user_id = user_id
            .0
            .ok_or_else(|| Error::new("X-USER-ID header not found"))?;

        let (drop_model, collection_model) = Drops::find()
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .select_also(Collections)
            .filter(drops::Column::Id.eq(id))
            .one(conn)
            .await?
            .ok_or_else(|| Error::new("drop not found"))?;

        let collection = collection_model.ok_or_else(|| Error::new("collection not found"))?;

        let mut collection_am: collections::ActiveModel = collection.into();

        if let Some(seller_fee_basis_points) = seller_fee_basis_points {
            collection_am.seller_fee_basis_points = Set(seller_fee_basis_points.try_into()?);
        }

        let collection = collection_am.update(conn).await?;

        let current_creators = collection_creators::Entity::find()
            .filter(collection_creators::Column::CollectionId.eq(collection.id))
            .all(conn)
            .await?;

        let mut drop_am = drops::ActiveModel::from(drop_model.clone());

        if let Some(price) = price {
            drop_am.price = Set(price.try_into()?);
        }

        drop_am.start_time = Set(Some(start_time.map_or(Utc::now().into(), |t| t)));
        drop_am.end_time = Set(end_time
            .map(|t| {
                if t > Utc::now() {
                    Ok(t)
                } else {
                    Err(Error::new("end time must be in the future"))
                }
            })
            .transpose()?);

        if creators.clone().is_some() {
            let creators = creators
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|creator| {
                    Ok(collection_creators::ActiveModel {
                        collection_id: Set(collection.id),
                        address: Set(creator.address),
                        verified: Set(creator.verified.unwrap_or_default()),
                        share: Set(creator.share.try_into()?),
                    })
                })
                .collect::<anyhow::Result<Vec<collection_creators::ActiveModel>>>()?;

            conn.transaction::<_, (), DbErr>(|txn| {
                Box::pin(async move {
                    collection_creators::Entity::delete_many()
                        .filter(collection_creators::Column::CollectionId.eq(collection.id))
                        .exec(txn)
                        .await?;

                    collection_creators::Entity::insert_many(creators)
                        .exec(txn)
                        .await?;

                    Ok(())
                })
            })
            .await?;
        }

        drop_am.update(conn).await?;

        let owner_address = project_wallets::Entity::find()
            .filter(
                project_wallets::Column::ProjectId
                    .eq(drop_model.project_id)
                    .and(project_wallets::Column::Blockchain.eq(BlockchainEnum::Solana)),
            )
            .one(conn)
            .await?
            .ok_or_else(|| Error::new("no project wallet found"))?
            .wallet_address;

        let metadata_json_model = metadata_jsons::Entity::find()
            .filter(metadata_jsons::Column::Id.eq(collection.id))
            .one(conn)
            .await?
            .ok_or_else(|| Error::new("metadata json not found"))?;

        let metadata_json_model = if let Some(metadata_json) = metadata_json {
            metadata_json_model.clone().delete(conn).await?;

            MetadataJson::new(metadata_json.clone())
                .upload(nft_storage)
                .await?
                .save(collection.id, db)
                .await?
        } else {
            metadata_json_model
        };

        let (
            _,
            TransactionResponse {
                serialized_message,
                signed_message_signatures,
            },
        ) = match collection.blockchain {
            BlockchainEnum::Solana => {
                let creators = if creators.clone().is_some() {
                    creators
                        .unwrap_or_default()
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<Vec<Creator>, _>>()?
                } else {
                    current_creators
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<Vec<Creator>, _>>()?
                };

                solana
                    .edition()
                    .update(UpdateEditionRequest {
                        collection: collection.id,
                        owner_address,
                        seller_fee_basis_points,
                        name: metadata_json_model.name,
                        symbol: metadata_json_model.symbol,
                        uri: metadata_json_model.uri,
                        creators,
                    })
                    .await?
            },
            _ => {
                return Err(Error::new("blockchain not supported yet"));
            },
        };

        emit_update_metadata_transaction_event(
            producer,
            collection.id,
            user_id,
            serialized_message,
            signed_message_signatures,
            drop_model.id,
            drop_model.project_id,
            collection.blockchain,
        )
        .await?;

        Ok(PatchDropPayload {
            drop: Drop::new(drop_model, collection),
        })
    }
}

/// This functions emits the retry drop transaction event
/// # Errors
/// This function fails if producer is unable to send the event
async fn emit_retry_drop_event(
    producer: &Producer<NftEvents>,
    id: Uuid,
    user_id: Uuid,
    serialized_message: Vec<u8>,
    signatures: Vec<String>,
    project_id: Uuid,
    blockchain: BlockchainEnum,
) -> Result<()> {
    let proto_blockchain_enum: proto::Blockchain = blockchain.into();

    let event = NftEvents {
        event: Some(nft_events::Event::RetryDrop(proto::DropTransaction {
            transaction: Some(proto::Transaction {
                serialized_message,
                signed_message_signatures: signatures,
                blockchain: proto_blockchain_enum as i32,
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
    blockchain: BlockchainEnum,
) -> Result<()> {
    let proto_blockchain_enum: proto::Blockchain = blockchain.into();

    let event = NftEvents {
        event: Some(nft_events::Event::CreateDrop(proto::DropTransaction {
            transaction: Some(proto::Transaction {
                serialized_message,
                signed_message_signatures: signatures,
                blockchain: proto_blockchain_enum as i32,
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

#[allow(clippy::too_many_arguments)]
async fn emit_update_metadata_transaction_event(
    producer: &Producer<NftEvents>,
    id: Uuid,
    user_id: Uuid,
    serialized_message: Vec<u8>,
    signatures: Vec<String>,
    project_id: Uuid,
    drop_id: Uuid,
    blockchain: BlockchainEnum,
) -> Result<()> {
    let proto_blockchain_enum: proto::Blockchain = blockchain.into();

    let event = NftEvents {
        event: Some(nft_events::Event::UpdateMetadata(
            proto::UpdateMetadataTransaction {
                transaction: Some(proto::Transaction {
                    serialized_message,
                    signed_message_signatures: signatures,
                    blockchain: proto_blockchain_enum as i32,
                }),
                drop_id: drop_id.to_string(),
                project_id: project_id.to_string(),
            },
        )),
    };
    let key = NftEventKey {
        id: id.to_string(),
        user_id: user_id.to_string(),
    };

    producer.send(Some(&event), Some(&key)).await?;

    Ok(())
}

async fn submit_pending_deduction(
    credits: &CreditsClient<Actions>,
    db: &Connection,
    balance: u64,
    user_id: Uuid,
    org_id: Uuid,
    drop: Uuid,
    blockchain: BlockchainEnum,
    action: Actions,
) -> Result<()> {
    let id = match blockchain {
        BlockchainEnum::Solana => {
            credits
                .submit_pending_deduction(
                    org_id,
                    user_id,
                    action,
                    hub_core::credits::Blockchain::Solana,
                    balance,
                )
                .await?
        },
        _ => {
            return Err(Error::new("blockchain not supported yet"));
        },
    };

    let deduction_id = id.ok_or_else(|| Error::new("failed to generate credits deduction id"))?;

    let drop_model = drops::Entity::find_by_id(drop)
        .one(db.get())
        .await?
        .ok_or_else(|| Error::new("drop not found"))?;

    let mut drop: drops::ActiveModel = drop_model.into();
    drop.credits_deduction_id = Set(Some(deduction_id.0));
    drop.update(db.get()).await?;

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
    pub start_time: Option<DateTimeWithTimeZone>,
    pub end_time: Option<DateTimeWithTimeZone>,
    pub blockchain: BlockchainEnum,
    pub creators: Vec<CollectionCreator>,
    pub metadata_json: MetadataJsonInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct RetryDropInput {
    pub drop: Uuid,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct RetryDropPayload {
    drop: Drop,
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

/// Input object for patching a drop and associated collection by ID
#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct PatchDropInput {
    /// The unique identifier of the drop
    pub id: Uuid,
    /// The new price for the drop in the native token of the blockchain
    pub price: Option<u64>,
    /// The new start time for the drop in UTC
    pub start_time: Option<DateTimeWithTimeZone>,
    /// The new end time for the drop in UTC
    pub end_time: Option<DateTimeWithTimeZone>,
    /// The new seller fee basis points for the drop
    pub seller_fee_basis_points: Option<u16>,
    /// The new metadata JSON for the drop
    pub metadata_json: Option<MetadataJsonInput>,
    /// The creators of the drop
    pub creators: Option<Vec<CollectionCreator>>,
}

/// Represents the result of a successful patch drop mutation.
#[derive(Debug, Clone, SimpleObject)]
pub struct PatchDropPayload {
    /// The drop that has been patched.
    drop: Drop,
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

impl From<BlockchainEnum> for proto::Blockchain {
    fn from(v: BlockchainEnum) -> Self {
        match v {
            BlockchainEnum::Ethereum => Self::Ethereum,
            BlockchainEnum::Polygon => Self::Polygon,
            BlockchainEnum::Solana => Self::Solana,
        }
    }
}
