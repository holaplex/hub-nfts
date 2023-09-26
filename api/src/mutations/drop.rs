use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::{
    chrono::Utc,
    credits::{CreditsClient, TransactionId},
    producer::Producer,
};
use sea_orm::{prelude::*, JoinType, ModelTrait, QuerySelect, Set, TransactionTrait};
use serde::{Deserialize, Serialize};

use super::collection::{validate_creators, validate_json, validate_solana_creator_verification};
use crate::{
    background_worker::{
        job_queue::JobQueue,
        tasks::{
            MetadataJsonUploadCaller, MetadataJsonUploadCreateDrop, MetadataJsonUploadPatchDrop,
            MetadataJsonUploadTask,
        },
    },
    blockchains::{polygon::Polygon, solana::Solana, DropEvent},
    entities::{
        collection_creators, collections, drops, metadata_jsons,
        prelude::{CollectionCreators, Collections, Drops, MetadataJsons},
        project_wallets,
        sea_orm_active_enums::{Blockchain as BlockchainEnum, CreationStatus, DropType},
    },
    objects::{Creator, Drop, MetadataJsonInput},
    proto::{
        self, nft_events::Event as NftEvent, CreationStatus as NftCreationStatus, EditionInfo,
        NftEventKey, NftEvents,
    },
    Actions, AppContext,
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

        let user_id = user_id.0.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = organization_id
            .0
            .ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;

        let conn = db.get();
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let nfts_producer = ctx.data::<Producer<NftEvents>>()?;
        let metadata_json_upload_job_queue = ctx.data::<JobQueue>()?;

        let owner_address = fetch_owner(conn, input.project, input.blockchain).await?;
        let supply = if input.drop_type == DropType::Open {
            Some(0)
        } else {
            input.supply.map(TryInto::try_into).transpose()?
        };
        input.validate()?;

        if input.blockchain == BlockchainEnum::Solana {
            validate_solana_creator_verification(&owner_address, &input.creators)?;
        }

        let TransactionId(credits_deduction_id) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                Actions::CreateDrop,
                input.blockchain.into(),
                balance,
            )
            .await?;

        let seller_fee_basis_points = input.seller_fee_basis_points.unwrap_or_default();

        let tx = conn.begin().await?;

        let collection_am = collections::ActiveModel {
            blockchain: Set(input.blockchain),
            supply: Set(supply),
            creation_status: Set(CreationStatus::Pending),
            seller_fee_basis_points: Set(seller_fee_basis_points.try_into()?),
            created_by: Set(user_id),
            project_id: Set(input.project),
            credits_deduction_id: Set(Some(credits_deduction_id)),
            ..Default::default()
        };

        let collection = collection_am.insert(&tx).await?;

        for creator in input.creators {
            let am = collection_creators::ActiveModel {
                collection_id: Set(collection.id),
                address: Set(creator.address),
                verified: Set(creator.verified.unwrap_or_default()),
                share: Set(creator.share.try_into()?),
            };

            am.insert(&tx).await?;
        }

        input.metadata_json.save(collection.id, &tx).await?;

        let drop = drops::ActiveModel {
            project_id: Set(input.project),
            collection_id: Set(collection.id),
            creation_status: Set(CreationStatus::Pending),
            start_time: Set(input.start_time),
            end_time: Set(input.end_time),
            price: Set(input.price.unwrap_or_default().try_into()?),
            created_by: Set(user_id),
            created_at: Set(Utc::now().into()),
            credits_deduction_id: Set(Some(credits_deduction_id)),
            drop_type: Set(input.drop_type),
            ..Default::default()
        };

        let drop_model = drop.insert(&tx).await?;

        tx.commit().await?;

        metadata_json_upload_job_queue
            .enqueue(MetadataJsonUploadTask::new(
                input.metadata_json,
                MetadataJsonUploadCaller::CreateDrop(MetadataJsonUploadCreateDrop {
                    drop_id: drop_model.id,
                }),
            ))
            .await?;

        nfts_producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::DropCreated(proto::DropCreation {
                        status: NftCreationStatus::InProgress as i32,
                        collection_id: collection.id.to_string(),
                    })),
                }),
                Some(&NftEventKey {
                    id: drop_model.id.to_string(),
                    project_id: input.project.to_string(),
                    user_id: user_id.to_string(),
                }),
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

        let user_id = user_id.0.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = organization_id
            .0
            .ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;

        let conn = db.get();
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let solana = ctx.data::<Solana>()?;
        let polygon = ctx.data::<Polygon>()?;

        let (drop, collection) = Drops::find_by_id(input.drop)
            .find_also_related(Collections)
            .one(conn)
            .await?
            .ok_or(Error::new("drop not found"))?;

        let collection = collection.ok_or(Error::new("collection not found"))?;

        if drop.creation_status == CreationStatus::Created {
            return Err(Error::new("drop already created"));
        }

        let metadata_json = MetadataJsons::find_by_id(collection.id)
            .one(conn)
            .await?
            .ok_or(Error::new("metadata json not found"))?;

        let metadata_uri = metadata_json
            .uri
            .ok_or(Error::new("metadata uri not found"))?;
        let creators = CollectionCreators::find()
            .filter(collection_creators::Column::CollectionId.eq(collection.id))
            .all(conn)
            .await?;

        let owner_address = fetch_owner(conn, drop.project_id, collection.blockchain).await?;

        let TransactionId(_) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                Actions::RetryDrop,
                collection.blockchain.into(),
                balance,
            )
            .await?;

        let event_key = NftEventKey {
            id: collection.id.to_string(),
            user_id: user_id.to_string(),
            project_id: drop.project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .retry_create_drop(
                        drop.drop_type,
                        event_key,
                        proto::MetaplexMasterEditionTransaction {
                            master_edition: Some(proto::MasterEdition {
                                owner_address,
                                supply: collection.supply.map(TryInto::try_into).transpose()?,
                                name: metadata_json.name,
                                symbol: metadata_json.symbol,
                                metadata_uri,
                                seller_fee_basis_points: collection.seller_fee_basis_points.into(),
                                creators: creators
                                    .into_iter()
                                    .map(|c| proto::Creator {
                                        address: c.address,
                                        verified: c.verified,
                                        share: c.share,
                                    })
                                    .collect(),
                            }),
                        },
                    )
                    .await?;
            },
            BlockchainEnum::Polygon => {
                let amount = collection
                    .supply
                    .ok_or(Error::new("Supply is null for polygon edition in db"))?;

                polygon
                    .event()
                    .retry_create_drop(drop.drop_type, event_key, proto::CreateEditionTransaction {
                        edition_info: None,
                        amount,
                        fee_receiver: owner_address,
                        fee_numerator: collection.seller_fee_basis_points.into(),
                    })
                    .await?;
            },
            BlockchainEnum::Ethereum => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        let mut drop_am: drops::ActiveModel = drop.into();
        drop_am.creation_status = Set(CreationStatus::Pending);
        let drop = drop_am.update(conn).await?;

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
            .ok_or(Error::new("drop not found"))?;

        let collection_model = collection.ok_or(Error::new(format!(
            "no collection found for drop {}",
            input.drop
        )))?;

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
            .ok_or(Error::new("drop not found"))?;

        let collection_model = collection.ok_or(Error::new(format!(
            "no collection found for drop {}",
            input.drop
        )))?;

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
            .ok_or(Error::new("drop not found"))?;

        let collection_model = collection.ok_or(Error::new(format!(
            "no collection found for drop {}",
            input.drop
        )))?;

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
        let metadata_json_upload_job_queue = ctx.data::<JobQueue>()?;
        let solana = ctx.data::<Solana>()?;
        let polygon = ctx.data::<Polygon>()?;

        let user_id = user_id.0.ok_or(Error::new("X-USER-ID header not found"))?;

        let (drop_model, collection_model) = drops::Entity::find_by_id_with_collection(id)
            .one(conn)
            .await?
            .ok_or(Error::new("drop not found"))?;

        let collection = collection_model.ok_or(Error::new("collection not found"))?;

        let owner_address = fetch_owner(conn, drop_model.project_id, collection.blockchain).await?;

        validate_end_time(&input.end_time.clone())?;

        if let Some(creators) = &creators {
            validate_creators(collection.blockchain, creators)?;

            if collection.blockchain == BlockchainEnum::Solana {
                validate_solana_creator_verification(&owner_address, creators)?;
            }
        }
        if let Some(metadata_json) = &metadata_json {
            validate_json(collection.blockchain, metadata_json)?;
        }

        let current_creators = collection_creators::Entity::find()
            .filter(collection_creators::Column::CollectionId.eq(collection.id))
            .all(conn)
            .await?;

        let metadata_json_model = metadata_jsons::Entity::find()
            .filter(metadata_jsons::Column::Id.eq(collection.id))
            .one(conn)
            .await?
            .ok_or(Error::new("metadata json not found"))?;

        let tx = conn.begin().await?;

        let mut collection_am: collections::ActiveModel = collection.into();
        if let Some(seller_fee_basis_points) = seller_fee_basis_points {
            collection_am.seller_fee_basis_points = Set(seller_fee_basis_points.try_into()?);
        }

        let collection = collection_am.update(&tx).await?;

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

        let creators = if let Some(creators) = creators {
            let creator_ams = creators
                .clone()
                .into_iter()
                .map(|creator| {
                    Ok(collection_creators::ActiveModel {
                        collection_id: Set(collection.id),
                        address: Set(creator.address),
                        verified: Set(creator.verified.unwrap_or_default()),
                        share: Set(creator.share.try_into()?),
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            collection_creators::Entity::delete_many()
                .filter(collection_creators::Column::CollectionId.eq(collection.id))
                .exec(&tx)
                .await?;

            collection_creators::Entity::insert_many(creator_ams)
                .exec(&tx)
                .await?;

            creators
                .into_iter()
                .map(TryFrom::try_from)
                .collect::<Result<_>>()?
        } else {
            current_creators.into_iter().map(Into::into).collect()
        };

        let drop_model = drop_am.update(&tx).await?;

        if let Some(metadata_json) = metadata_json {
            metadata_json_model.delete(&tx).await?;

            metadata_json.save(collection.id, &tx).await?;

            metadata_json_upload_job_queue
                .enqueue(MetadataJsonUploadTask {
                    metadata_json,
                    caller: MetadataJsonUploadCaller::PatchDrop(MetadataJsonUploadPatchDrop {
                        drop_id: drop_model.id,
                        updated_by_id: user_id,
                    }),
                })
                .await?;
        } else {
            let event_key = NftEventKey {
                id: collection.id.to_string(),
                user_id: user_id.to_string(),
                project_id: drop_model.project_id.to_string(),
            };

            let metadata_uri = metadata_json_model
                .uri
                .ok_or(Error::new("metadata uri not found"))?;

            match collection.blockchain {
                BlockchainEnum::Solana => {
                    solana
                        .event()
                        .update_drop(
                            drop_model.drop_type,
                            event_key,
                            proto::MetaplexMasterEditionTransaction {
                                master_edition: Some(proto::MasterEdition {
                                    owner_address,
                                    supply: collection.supply.map(TryInto::try_into).transpose()?,
                                    name: metadata_json_model.name,
                                    symbol: metadata_json_model.symbol,
                                    metadata_uri,
                                    seller_fee_basis_points: collection
                                        .seller_fee_basis_points
                                        .into(),
                                    creators,
                                }),
                            },
                        )
                        .await?;
                },
                BlockchainEnum::Polygon => {
                    polygon
                        .event()
                        .update_drop(
                            drop_model.drop_type,
                            event_key,
                            proto::UpdateEdtionTransaction {
                                edition_info: Some(EditionInfo {
                                    description: metadata_json_model.description,
                                    image_uri: metadata_json_model.image,
                                    collection: metadata_json_model.name,
                                    uri: metadata_uri,
                                    creator: creators
                                        .get(0)
                                        .ok_or(Error::new("no creator found"))?
                                        .address
                                        .clone(),
                                }),
                            },
                        )
                        .await?;
                },
                BlockchainEnum::Ethereum => {
                    return Err(Error::new("blockchain not supported yet"));
                },
            };
        }

        tx.commit().await?;

        Ok(PatchDropPayload {
            drop: Drop::new(drop_model, collection),
        })
    }
}

async fn fetch_owner(
    conn: &DatabaseConnection,
    project: Uuid,
    blockchain: BlockchainEnum,
) -> Result<String> {
    let wallet = project_wallets::Entity::find()
        .filter(
            project_wallets::Column::ProjectId
                .eq(project)
                .and(project_wallets::Column::Blockchain.eq(blockchain)),
        )
        .one(conn)
        .await?;

    let owner = wallet
        .ok_or(Error::new(format!(
            "no project wallet found for {blockchain:?} blockchain"
        )))?
        .wallet_address;
    Ok(owner)
}

#[derive(Debug, Clone, SimpleObject)]
pub struct CreateDropPayload {
    drop: Drop,
}

#[derive(Debug, Clone, InputObject)]
pub struct CreateDropInput {
    pub project: Uuid,
    pub price: Option<u64>,
    pub seller_fee_basis_points: Option<u16>,
    pub supply: Option<u64>,
    pub start_time: Option<DateTimeWithTimeZone>,
    pub end_time: Option<DateTimeWithTimeZone>,
    pub blockchain: BlockchainEnum,
    pub creators: Vec<Creator>,
    pub metadata_json: MetadataJsonInput,
    #[graphql(name = "type", default)]
    pub drop_type: DropType,
}

impl CreateDropInput {
    /// This function is used to validate the data of a new NFT drop before it is saved or submitted to the blockchain.
    /// Validation Steps:
    ///  Validate the addresses of the creators. Each creator's address should be a valid address.
    ///  Ensure that the supply is greater than 0 or undefined.
    ///  Check if the end time (if provided) is in the future.
    ///  Validates the metadata JSON.
    ///
    /// # Returns:
    /// - Ok(()) if all validations pass successfully.
    /// # Errors
    /// - Err with an appropriate error message if any validation fails.
    pub fn validate(&self) -> Result<()> {
        if self.supply == Some(0) && self.drop_type == DropType::Edition {
            return Err(Error::new("Supply must be greater than 0 or undefined"));
        };

        validate_end_time(&self.end_time)?;
        validate_creators(self.blockchain, &self.creators)?;
        validate_json(self.blockchain, &self.metadata_json)?;
        validate_polygon_supply(self.blockchain, self.supply)?;
        validate_polygon_creator(self.blockchain, &self.creators)?;

        Ok(())
    }
}

/// Validates the creators for polygon drops.
/// # Returns
/// - Ok(()) if the creators are provided for polygon drops.
/// # Errors
/// - Err with an appropriate error message if the creators are not provided for polygon drops.
fn validate_polygon_creator(blockchain: BlockchainEnum, creators: &[Creator]) -> Result<()> {
    if blockchain == BlockchainEnum::Polygon && creators.len() > 1 {
        return Err(Error::new("Only one creator is allowed for polygon drops"));
    }

    Ok(())
}

/// Validates the supply for polygon drops.
/// # Returns
/// - Ok(()) if the supply is provided for polygon drops.
/// # Errors
/// - Err with an appropriate error message if the supply is not provided for polygon drops.
fn validate_polygon_supply(blockchain: BlockchainEnum, supply: Option<u64>) -> Result<()> {
    if blockchain == BlockchainEnum::Polygon && supply.is_none() {
        return Err(Error::new("Supply is required for polygon drops"));
    }

    Ok(())
}
/// Validates the end time of the NFT drop.
/// # Returns
/// - Ok(()) if the end time is in the future or if it's not provided.
/// # Errors
/// - Err with an appropriate error message if the end time is in the past.
fn validate_end_time(end_time: &Option<DateTimeWithTimeZone>) -> Result<()> {
    end_time.map_or(Ok(()), |end_time| {
        if end_time > Utc::now() {
            Ok(())
        } else {
            Err(Error::new("End time must be in the future"))
        }
    })?;

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct RetryDropInput {
    pub drop: Uuid,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct RetryDropPayload {
    drop: Drop,
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
    pub creators: Option<Vec<Creator>>,
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
