use std::ops::Add;

use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::{
    chrono::Utc,
    credits::{CreditsClient, DeductionErrorKind, TransactionId},
    producer::Producer,
};
use sea_orm::{prelude::*, JoinType, QuerySelect, Set};

use super::collection::{
    fetch_owner, validate_creators, validate_json, validate_solana_creator_verification,
};
use crate::{
    blockchains::{polygon::Polygon, solana::Solana, CollectionEvent, DropEvent},
    entities::{
        collection_mints, collections, drops, mint_creators, mint_histories,
        prelude::{Collections, Drops},
        project_wallets,
        sea_orm_active_enums::{Blockchain as BlockchainEnum, CreationStatus},
    },
    metadata_json::MetadataJson,
    objects::{Creator, MetadataJsonInput},
    proto::{
        self, nft_events::Event as NftEvent, CreationStatus as NftCreationStatus, MetaplexMetadata,
        MintCollectionCreation, MintCreation, NftEventKey, NftEvents,
    },
    Actions, AppContext, NftStorageClient, OrganizationId, UserID,
};

#[derive(Default)]
pub struct Mutation;

#[Object(name = "MintMutation")]
impl Mutation {
    /// This mutation mints an NFT edition for a specific drop ID. The mint returns immediately with a creation status of CREATING. You can [set up a webhook](https://docs.holaplex.dev/hub/For%20Developers/webhooks-overview) to receive a notification when the mint is accepted by the blockchain.
    /// # Errors
    /// If the mint cannot be saved to the database or fails to be emitted for submission to the desired blockchain, the mutation will result in an error.
    pub async fn mint_edition(
        &self,
        ctx: &Context<'_>,
        input: MintDropInput,
    ) -> Result<MintEditionPayload> {
        let AppContext {
            db,
            user_id,
            organization_id,
            balance,
            ..
        } = ctx.data::<AppContext>()?;
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let conn = db.get();
        let solana = ctx.data::<Solana>()?;
        let polygon = ctx.data::<Polygon>()?;
        let nfts_producer = ctx.data::<Producer<NftEvents>>()?;

        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;

        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = org.ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;

        let drop_model = Drops::find()
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .select_also(Collections)
            .filter(drops::Column::Id.eq(input.drop))
            .one(conn)
            .await?;

        let (drop_model, collection_model) = drop_model.ok_or(Error::new("drop not found"))?;

        // Call check_drop_status to check that drop is currently running
        check_drop_status(&drop_model)?;

        let collection = collection_model.ok_or(Error::new("collection not found"))?;

        if collection.supply == Some(collection.total_mints) {
            return Err(Error::new("Collection is sold out"));
        }

        let edition = collection.total_mints.add(1);

        // Fetch the project wallet address which will sign the transaction by hub-treasuries
        let wallet = project_wallets::Entity::find()
            .filter(
                project_wallets::Column::ProjectId
                    .eq(drop_model.project_id)
                    .and(project_wallets::Column::Blockchain.eq(collection.blockchain)),
            )
            .one(conn)
            .await?;

        let owner_address = wallet
            .ok_or(Error::new(format!(
                "no project wallet found for {} blockchain",
                collection.blockchain
            )))?
            .wallet_address;

        let TransactionId(credits_deduction_id) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                Actions::MintEdition,
                collection.blockchain.into(),
                balance,
            )
            .await
            .map_err(|e| match e.kind() {
                DeductionErrorKind::InsufficientBalance { available, cost } => Error::new(format!(
                    "insufficient balance: available: {available}, cost: {cost}"
                )),
                DeductionErrorKind::MissingItem => Error::new("action not supported at this time"),
                DeductionErrorKind::InvalidCost(_) => Error::new("invalid cost"),
                DeductionErrorKind::Send(_) => {
                    Error::new("unable to send credit deduction request")
                },
            })?;

        // insert a collection mint record into database
        let collection_mint_active_model = collection_mints::ActiveModel {
            collection_id: Set(collection.id),
            owner: Set(input.recipient.clone()),
            creation_status: Set(CreationStatus::Pending),
            seller_fee_basis_points: Set(collection.seller_fee_basis_points),
            created_by: Set(user_id),
            edition: Set(edition),
            credits_deduction_id: Set(Some(credits_deduction_id)),
            ..Default::default()
        };

        let collection_mint_model = collection_mint_active_model.insert(conn).await?;
        let event_key = NftEventKey {
            id: collection_mint_model.id.to_string(),
            user_id: user_id.to_string(),
            project_id: drop_model.project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                MetadataJson::fetch(collection.id, db)
                    .await?
                    .save(collection_mint_model.id, db)
                    .await?;

                solana
                    .event()
                    .mint_drop(event_key, proto::MintMetaplexEditionTransaction {
                        recipient_address: input.recipient.to_string(),
                        owner_address: owner_address.to_string(),
                        edition,
                        collection_id: collection.id.to_string(),
                    })
                    .await?;
            },
            BlockchainEnum::Polygon => {
                polygon
                    .event()
                    .mint_drop(event_key, proto::MintEditionTransaction {
                        receiver: input.recipient.to_string(),
                        amount: 1,
                        collection_id: collection.id.to_string(),
                    })
                    .await?;
            },
            BlockchainEnum::Ethereum => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        let mut collection_am = collections::ActiveModel::from(collection.clone());
        collection_am.total_mints = Set(edition);
        collection_am.update(conn).await?;

        // inserts a mint histories record in the database
        let purchase_am = mint_histories::ActiveModel {
            mint_id: Set(collection_mint_model.id),
            wallet: Set(input.recipient),
            collection_id: Set(collection.id),
            tx_signature: Set(None),
            status: Set(CreationStatus::Pending),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        };

        purchase_am.insert(conn).await?;

        nfts_producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::DropMinted(MintCreation {
                        drop_id: drop_model.id.to_string(),
                        status: NftCreationStatus::InProgress as i32,
                    })),
                }),
                Some(&NftEventKey {
                    id: collection_mint_model.id.to_string(),
                    project_id: drop_model.project_id.to_string(),
                    user_id: user_id.to_string(),
                }),
            )
            .await?;

        Ok(MintEditionPayload {
            collection_mint: collection_mint_model.into(),
        })
    }

    /// This mutation retries a mint which failed or is in pending state. The mint returns immediately with a creation status of CREATING. You can [set up a webhook](https://docs.holaplex.dev/hub/For%20Developers/webhooks-overview) to receive a notification when the mint is accepted by the blockchain.
    /// # Errors
    /// If the mint cannot be saved to the database or fails to be emitted for submission to the desired blockchain, the mutation will result in an error.
    pub async fn retry_mint_edition(
        &self,
        ctx: &Context<'_>,
        input: RetryMintEditionInput,
    ) -> Result<RetryMintEditionPayload> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let conn = db.get();
        let solana = ctx.data::<Solana>()?;
        let polygon = ctx.data::<Polygon>()?;

        let UserID(id) = user_id;
        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;

        let (collection_mint_model, drop) = collection_mints::Entity::find()
            .join(
                JoinType::InnerJoin,
                collection_mints::Relation::Collections.def(),
            )
            .join(JoinType::InnerJoin, collections::Relation::Drop.def())
            .select_also(drops::Entity)
            .filter(collection_mints::Column::Id.eq(input.id))
            .one(conn)
            .await?
            .ok_or(Error::new("collection mint not found"))?;

        if collection_mint_model.creation_status == CreationStatus::Created {
            return Err(Error::new("mint is already created"));
        }

        let collection = collections::Entity::find()
            .filter(collections::Column::Id.eq(collection_mint_model.collection_id))
            .one(conn)
            .await?
            .ok_or(Error::new("collection not found"))?;

        let drop_model = drop.ok_or(Error::new("drop not found"))?;

        let recipient = collection_mint_model.owner.clone();
        let edition = collection_mint_model.edition;
        let project_id = drop_model.project_id;

        // Fetch the project wallet address which will sign the transaction by hub-treasuries
        let wallet = project_wallets::Entity::find()
            .filter(
                project_wallets::Column::ProjectId
                    .eq(project_id)
                    .and(project_wallets::Column::Blockchain.eq(collection.blockchain)),
            )
            .one(conn)
            .await?;

        let owner_address = wallet
            .ok_or(Error::new(format!(
                "no project wallet found for {} blockchain",
                collection.blockchain
            )))?
            .wallet_address;

        let event_key = NftEventKey {
            id: collection_mint_model.id.to_string(),
            user_id: user_id.to_string(),
            project_id: project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .retry_mint_drop(event_key, proto::MintMetaplexEditionTransaction {
                        recipient_address: recipient.to_string(),
                        owner_address: owner_address.to_string(),
                        edition,
                        collection_id: collection.id.to_string(),
                    })
                    .await?;
            },
            BlockchainEnum::Polygon => {
                polygon
                    .event()
                    .retry_mint_drop(event_key, proto::MintEditionTransaction {
                        receiver: recipient.to_string(),
                        amount: 1,
                        collection_id: collection.id.to_string(),
                    })
                    .await?;
            },
            BlockchainEnum::Ethereum => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        let mut mint_am: collection_mints::ActiveModel = collection_mint_model.into();
        mint_am.creation_status = Set(CreationStatus::Pending);
        let collection_mint_model = mint_am.update(conn).await?;

        Ok(RetryMintEditionPayload {
            collection_mint: collection_mint_model.into(),
        })
    }

    /// This mutation mints either a compressed or standard NFT to a collection.
    /// For Solana, the mint is verified and the collection size incremented.
    pub async fn mint_to_collection(
        &self,
        ctx: &Context<'_>,
        input: MintToCollectionInput,
    ) -> Result<MintToCollectionPayload> {
        let AppContext {
            db,
            user_id,
            organization_id,
            balance,
            ..
        } = ctx.data::<AppContext>()?;
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let conn = db.get();
        let solana = ctx.data::<Solana>()?;
        let nfts_producer = ctx.data::<Producer<NftEvents>>()?;
        let nft_storage = ctx.data::<NftStorageClient>()?;

        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;

        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = org.ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;

        let creators = input.creators;

        let collection = Collections::find()
            .filter(collections::Column::Id.eq(input.collection))
            .one(conn)
            .await?;

        let collection = collection.ok_or(Error::new("collection not found"))?;
        let blockchain = collection.blockchain;
        let compressed = input.compressed.unwrap_or_default();

        validate_creators(blockchain, &creators)?;
        validate_json(blockchain, &input.metadata_json)?;
        check_collection_status(&collection)?;
        validate_compress(blockchain, compressed)?;

        let seller_fee_basis_points = input.seller_fee_basis_points.unwrap_or_default();

        let owner_address = fetch_owner(conn, collection.project_id, blockchain).await?;

        if collection.blockchain == BlockchainEnum::Solana {
            validate_solana_creator_verification(&owner_address, &creators)?;
        }

        let action = if compressed {
            Actions::MintCompressed
        } else {
            Actions::Mint
        };

        let TransactionId(credits_deduction_id) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                action,
                collection.blockchain.into(),
                balance,
            )
            .await
            .map_err(|e| match e.kind() {
                DeductionErrorKind::InsufficientBalance { available, cost } => Error::new(format!(
                    "insufficient balance: available: {}, cost: {}",
                    available, cost
                )),
                DeductionErrorKind::MissingItem => Error::new("action not supported at this time"),
                DeductionErrorKind::InvalidCost(_) => Error::new("invalid cost"),
                DeductionErrorKind::Send(_) => {
                    Error::new("unable to send credit deduction request")
                },
            })?;

        // insert a collection mint record into database
        let collection_mint_active_model = collection_mints::ActiveModel {
            collection_id: Set(collection.id),
            owner: Set(input.recipient.clone()),
            creation_status: Set(CreationStatus::Pending),
            seller_fee_basis_points: Set(collection.seller_fee_basis_points),
            created_by: Set(user_id),
            compressed: Set(compressed),
            credits_deduction_id: Set(Some(credits_deduction_id)),
            ..Default::default()
        };

        let collection_mint_model = collection_mint_active_model.insert(conn).await?;

        let metadata_json = MetadataJson::new(input.metadata_json)
            .upload(nft_storage)
            .await?
            .save(collection_mint_model.id, db)
            .await?;

        for creator in creators.clone() {
            let am = mint_creators::ActiveModel {
                collection_mint_id: Set(collection_mint_model.id),
                address: Set(creator.address),
                verified: Set(creator.verified.unwrap_or_default()),
                share: Set(creator.share.try_into()?),
            };

            am.insert(conn).await?;
        }

        let event_key = NftEventKey {
            id: collection_mint_model.id.to_string(),
            user_id: user_id.to_string(),
            project_id: collection.project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .mint_to_collection(event_key, proto::MintMetaplexMetadataTransaction {
                        metadata: Some(MetaplexMetadata {
                            owner_address,
                            name: metadata_json.name,
                            symbol: metadata_json.symbol,
                            metadata_uri: metadata_json.uri,
                            seller_fee_basis_points: seller_fee_basis_points.into(),
                            creators: creators
                                .into_iter()
                                .map(TryFrom::try_from)
                                .collect::<Result<_>>()?,
                        }),
                        recipient_address: input.recipient.to_string(),
                        compressed,
                        collection_id: collection.id.to_string(),
                    })
                    .await?;
            },
            BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        let mut collection_am = collections::ActiveModel::from(collection.clone());
        collection_am.total_mints = Set(collection.total_mints.add(1));
        collection_am.update(conn).await?;

        let mint_history_am = mint_histories::ActiveModel {
            mint_id: Set(collection_mint_model.id),
            wallet: Set(input.recipient),
            collection_id: Set(collection.id),
            tx_signature: Set(None),
            status: Set(CreationStatus::Pending),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        };

        mint_history_am.insert(conn).await?;

        nfts_producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::MintedToCollection(MintCollectionCreation {
                        collection_id: collection.id.to_string(),
                        status: NftCreationStatus::InProgress as i32,
                    })),
                }),
                Some(&NftEventKey {
                    id: collection_mint_model.id.to_string(),
                    project_id: collection.project_id.to_string(),
                    user_id: user_id.to_string(),
                }),
            )
            .await?;

        Ok(MintToCollectionPayload {
            collection_mint: collection_mint_model.into(),
        })
    }

    // Retries a mint which failed by passing its ID.
    pub async fn retry_mint_to_collection(
        &self,
        ctx: &Context<'_>,
        input: RetryMintEditionInput,
    ) -> Result<RetryMintEditionPayload> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let conn = db.get();
        let solana = ctx.data::<Solana>()?;

        let UserID(id) = user_id;

        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;

        let (collection_mint_model, collection) =
            collection_mints::Entity::find_by_id_with_collection(input.id)
                .one(conn)
                .await?
                .ok_or(Error::new("collection mint not found"))?;

        if collection_mint_model.creation_status != CreationStatus::Failed {
            return Err(Error::new("only failed mints can be retried"));
        }

        let collection = collection.ok_or(Error::new("collection  not found"))?;

        let recipient = collection_mint_model.owner.clone();
        let project_id = collection.project_id;
        let blockchain = collection.blockchain;

        let owner_address = fetch_owner(conn, project_id, blockchain).await?;

        let MetadataJson {
            metadata_json, uri, ..
        } = MetadataJson::fetch(collection_mint_model.id, db).await?;

        let event_key = NftEventKey {
            id: collection_mint_model.id.to_string(),
            user_id: user_id.to_string(),
            project_id: project_id.to_string(),
        };

        let creators = mint_creators::Entity::find_by_collection_mint_id(collection_mint_model.id)
            .all(conn)
            .await?;

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .retry_mint_to_collection(event_key, proto::MintMetaplexMetadataTransaction {
                        metadata: Some(MetaplexMetadata {
                            owner_address,
                            name: metadata_json.name,
                            symbol: metadata_json.symbol,
                            metadata_uri: uri.ok_or(Error::new("metadata uri not found"))?,
                            seller_fee_basis_points: collection_mint_model
                                .seller_fee_basis_points
                                .into(),
                            creators: creators.into_iter().map(Into::into).collect(),
                        }),
                        recipient_address: recipient.to_string(),
                        compressed: collection_mint_model.compressed,
                        collection_id: collection_mint_model.collection_id.to_string(),
                    })
                    .await?;
            },
            BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        Ok(RetryMintEditionPayload {
            collection_mint: collection_mint_model.into(),
        })
    }
}

fn validate_compress(blockchain: BlockchainEnum, compressed: bool) -> Result<(), Error> {
    if blockchain != BlockchainEnum::Solana && compressed {
        return Err(Error::new("compression is only supported on Solana"));
    }

    Ok(())
}
/// Checks the status of a drop by verifying if it is currently running based on its start time, end time, and pause/shutdown status.
/// # Errors
///
/// This function returns an error if the drop is not yet created, paused,
/// shutdown, has not yet started, or has already ended based
fn check_drop_status(drop_model: &drops::Model) -> Result<(), Error> {
    if drop_model.creation_status != CreationStatus::Created {
        return Err(Error::new("Drop has not been created"));
    }

    drop_model
        .paused_at
        .map_or(Ok(()), |_| Err(Error::new("Drop status is paused")))?;

    drop_model
        .shutdown_at
        .map_or(Ok(()), |_| Err(Error::new("Drop status is shutdown")))?;

    drop_model.start_time.map_or(Ok(()), |start_time| {
        if start_time <= Utc::now() {
            Ok(())
        } else {
            Err(Error::new("Drop has not yet started"))
        }
    })?;

    drop_model.end_time.map_or(Ok(()), |end_time| {
        if end_time > Utc::now() {
            Ok(())
        } else {
            Err(Error::new("Drop has already ended"))
        }
    })?;

    Ok(())
}

fn check_collection_status(collection_model: &collections::Model) -> Result<(), Error> {
    if collection_model.creation_status != CreationStatus::Created {
        return Err(Error::new("Collection has not been created"));
    }

    Ok(())
}

/// Represents input data for `mint_edition` mutation with a UUID and recipient as fields
#[derive(Debug, Clone, InputObject)]
pub struct MintDropInput {
    /// The ID of the drop to mint to
    drop: Uuid,
    /// The recipient of the mint
    recipient: String,
}

/// Represents payload data for the `mint_edition` mutation
#[derive(Debug, Clone, SimpleObject)]
pub struct MintEditionPayload {
    collection_mint: collection_mints::CollectionMint,
}

/// Represents input data for `retry_mint` mutation with an ID as a field of type UUID
#[derive(Debug, Clone, InputObject)]
pub struct RetryMintEditionInput {
    id: Uuid,
}

/// Represents payload data for `retry_mint` mutation
#[derive(Debug, Clone, SimpleObject)]
pub struct RetryMintEditionPayload {
    collection_mint: collection_mints::CollectionMint,
}

/// Represents input data for `mint_to_collection` mutation with a collection ID, recipient, metadata, and optional seller fee basis points as fields
#[derive(Debug, Clone, InputObject)]
pub struct MintToCollectionInput {
    /// The ID of the collection to mint to
    collection: Uuid,
    /// The recipient of the mint
    recipient: String,
    /// The metadata of the mint
    metadata_json: MetadataJsonInput,
    /// The optional seller fee basis points
    seller_fee_basis_points: Option<u16>,
    /// The creators to be assigned to the NFT.
    /// For Solana, this can be up to five creators. If the project treasury wallet is set as a creator and verified set to true the creator will be verified on chain.
    /// For Polygon, this can be only 1 creator.
    creators: Vec<Creator>,
    compressed: Option<bool>,
}

/// Represents payload data for `mint_to_collection` mutation
#[derive(Debug, Clone, SimpleObject)]
pub struct MintToCollectionPayload {
    /// The minted NFT
    collection_mint: collection_mints::CollectionMint,
}

/// Represents input data for `retry_mint_to_collection` mutation with an ID as a field of type UUID
#[derive(Debug, Clone, InputObject)]
pub struct RetryMintToCollectionInput {
    /// The ID of the collection mint to retry
    id: Uuid,
}

/// Represents payload data for `retry_mint_to_collection` mutation
#[derive(Debug, Clone, SimpleObject)]
pub struct RetryMintToCollectionPayload {
    /// The retried minted NFT
    collection_mint: collection_mints::CollectionMint,
}
