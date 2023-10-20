use std::ops::Add;

use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::{
    chrono::Utc,
    credits::{CreditsClient, TransactionId},
    producer::Producer,
};
use redis::AsyncCommands;
use sea_orm::{prelude::*, JoinType, Order, QueryOrder, QuerySelect, Set, TransactionTrait};

use super::collection::{
    fetch_owner, validate_creators, validate_json, validate_solana_creator_verification,
};
use crate::{
    background_worker::{
        job_queue::JobQueue,
        tasks::{
            MetadataJsonUploadCaller, MetadataJsonUploadMintToCollection,
            MetadataJsonUploadQueueMintToDrop, MetadataJsonUploadTask,
            MetadataJsonUploadUpdateMint,
        },
    },
    blockchains::{
        polygon::Polygon,
        solana::{MintDropTransaction, Solana},
        CollectionEvent, DropEvent,
    },
    entities::{
        collection_creators, collection_mints, collections, drops, metadata_json_attributes,
        metadata_json_files, metadata_jsons, mint_creators, mint_histories,
        prelude::{CollectionCreators, CollectionMints, Collections},
        sea_orm_active_enums::{Blockchain as BlockchainEnum, CreationStatus},
        update_histories,
    },
    objects::{CollectionMint, Creator, MetadataJsonInput},
    proto::{
        self, nft_events::Event as NftEvent, CreationStatus as NftCreationStatus, MetaplexMetadata,
        MintCollectionCreation, MintCreation, MintOpenDropTransaction, NftEventKey, NftEvents,
        RetryUpdateSolanaMintPayload, SolanaMintOpenDropBatchedPayload,
    },
    Actions, AppContext, OrganizationId, UserID,
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
            redis,
            ..
        } = ctx.data::<AppContext>()?;
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let conn = db.get();
        let mut redis_conn = redis.get_async_connection().await?;
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

        let (drop_model, collection) = drops::Entity::find_by_id_with_collection(input.drop)
            .one(conn)
            .await?
            .ok_or(Error::new("drop not found"))?;

        let collection = collection.ok_or(Error::new("collection not found"))?;

        // Call check_drop_status to check that drop is currently running
        check_drop_status(&drop_model)?;

        let total_mints = collection_mints::Entity::filter_by_collection(collection.id)
            .count(conn)
            .await?;

        let total_mints = i64::try_from(total_mints)?;

        if collection.supply == Some(total_mints) {
            return Err(Error::new("Collection is sold out"));
        }

        let edition = total_mints.add(1);

        let owner_address = fetch_owner(conn, collection.project_id, collection.blockchain).await?;

        let TransactionId(credits_deduction_id) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                Actions::MintEdition,
                collection.blockchain.into(),
                balance,
            )
            .await?;

        let tx = conn.begin().await?;

        // insert a collection mint record into database
        let collection_mint_active_model = collection_mints::ActiveModel {
            collection_id: Set(collection.id),
            owner: Set(Some(input.recipient.clone())),
            creation_status: Set(CreationStatus::Pending),
            seller_fee_basis_points: Set(collection.seller_fee_basis_points),
            created_by: Set(user_id),
            edition: Set(edition),
            credits_deduction_id: Set(Some(credits_deduction_id)),
            ..Default::default()
        };

        let collection_mint_model = collection_mint_active_model.insert(conn).await?;

        // inserts a mint histories record in the database
        let mint_history_am = mint_histories::ActiveModel {
            mint_id: Set(collection_mint_model.id),
            wallet: Set(input.recipient.clone()),
            collection_id: Set(collection.id),
            tx_signature: Set(None),
            status: Set(CreationStatus::Pending),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        };

        mint_history_am.insert(&tx).await?;

        if collection.blockchain == BlockchainEnum::Solana {
            let collection_metadata_json = metadata_jsons::Entity::find_by_id(collection.id)
                .one(conn)
                .await?
                .ok_or(Error::new("metadata json not found"))?;

            let creators = collection_creators::Entity::find()
                .filter(collection_creators::Column::CollectionId.eq(collection.id))
                .all(conn)
                .await?;

            let files = metadata_json_files::Entity::find()
                .filter(metadata_json_files::Column::MetadataJsonId.eq(collection_metadata_json.id))
                .all(conn)
                .await?;
            let attributes = metadata_json_attributes::Entity::find()
                .filter(
                    metadata_json_attributes::Column::MetadataJsonId
                        .eq(collection_metadata_json.id),
                )
                .all(conn)
                .await?;

            let mut metadata_json_am = metadata_jsons::ActiveModel::from(collection_metadata_json);

            metadata_json_am.id = Set(collection_mint_model.id);

            let collection_mint_metadata_json = metadata_json_am.insert(&tx).await?;

            let creators: Vec<mint_creators::ActiveModel> = creators
                .into_iter()
                .map(|creator| mint_creators::ActiveModel {
                    collection_mint_id: Set(collection_mint_model.id),
                    address: Set(creator.address),
                    verified: Set(creator.verified),
                    share: Set(creator.share),
                })
                .collect();

            if !creators.is_empty() {
                mint_creators::Entity::insert_many(creators)
                    .exec(&tx)
                    .await?;
            }

            let files: Vec<metadata_json_files::ActiveModel> = files
                .into_iter()
                .map(|file| metadata_json_files::ActiveModel {
                    metadata_json_id: Set(collection_mint_metadata_json.id),
                    uri: Set(file.uri),
                    file_type: Set(file.file_type),
                    ..Default::default()
                })
                .collect();

            if !files.is_empty() {
                metadata_json_files::Entity::insert_many(files)
                    .exec(&tx)
                    .await?;
            }

            let attributes: Vec<metadata_json_attributes::ActiveModel> = attributes
                .into_iter()
                .map(|attribute| metadata_json_attributes::ActiveModel {
                    metadata_json_id: Set(collection_mint_metadata_json.id),
                    trait_type: Set(attribute.trait_type),
                    value: Set(attribute.value),
                    ..Default::default()
                })
                .collect();

            if !attributes.is_empty() {
                metadata_json_attributes::Entity::insert_many(attributes)
                    .exec(&tx)
                    .await?;
            }
        }

        tx.commit().await?;

        redis_conn
            .del(format!("collection:{}:total_mints", collection.id))
            .await?;

        let event_key = NftEventKey {
            id: collection_mint_model.id.to_string(),
            user_id: user_id.to_string(),
            project_id: drop_model.project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .mint_drop(
                        event_key,
                        MintDropTransaction::Edition(proto::MintMetaplexEditionTransaction {
                            recipient_address: input.recipient.to_string(),
                            owner_address: owner_address.to_string(),
                            edition,
                            collection_id: collection.id.to_string(),
                        }),
                    )
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

        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;

        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = org.ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;

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

        let recipient = collection_mint_model
            .owner
            .clone()
            .ok_or(Error::new("collection mint does not have an owner"))?;

        let edition = collection_mint_model.edition;
        let project_id = drop_model.project_id;

        let owner_address = fetch_owner(conn, project_id, collection.blockchain).await?;

        let TransactionId(_) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                Actions::RetryMint,
                collection.blockchain.into(),
                balance,
            )
            .await?;

        let event_key = NftEventKey {
            id: collection_mint_model.id.to_string(),
            user_id: user_id.to_string(),
            project_id: project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .retry_mint_drop(
                        event_key,
                        MintDropTransaction::Edition(proto::MintMetaplexEditionTransaction {
                            recipient_address: recipient.to_string(),
                            owner_address: owner_address.to_string(),
                            edition,
                            collection_id: collection.id.to_string(),
                        }),
                    )
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
            redis,
            ..
        } = ctx.data::<AppContext>()?;
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let conn = db.get();
        let mut redis_conn = redis.get_async_connection().await?;
        let nfts_producer = ctx.data::<Producer<NftEvents>>()?;
        let metadata_json_upload_job_queue = ctx.data::<JobQueue>()?;

        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;

        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = org.ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;

        let creators = input.creators;

        let collection = Collections::find_by_id(input.collection)
            .one(conn)
            .await?
            .ok_or(Error::new("collection not found"))?;

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
            .await?;

        let tx = conn.begin().await?;

        // insert a collection mint record into database
        let collection_mint_active_model = collection_mints::ActiveModel {
            collection_id: Set(collection.id),
            owner: Set(Some(input.recipient.clone())),
            creation_status: Set(CreationStatus::Pending),
            seller_fee_basis_points: Set(seller_fee_basis_points.try_into()?),
            created_by: Set(user_id),
            compressed: Set(Some(compressed)),
            credits_deduction_id: Set(Some(credits_deduction_id)),
            ..Default::default()
        };

        let collection_mint_model = collection_mint_active_model.insert(&tx).await?;

        input
            .metadata_json
            .save(collection_mint_model.id, &tx)
            .await?;

        for creator in creators {
            let am = mint_creators::ActiveModel {
                collection_mint_id: Set(collection_mint_model.id),
                address: Set(creator.address),
                verified: Set(creator.verified.unwrap_or_default()),
                share: Set(creator.share.try_into()?),
            };

            am.insert(&tx).await?;
        }

        let mint_history_am = mint_histories::ActiveModel {
            mint_id: Set(collection_mint_model.id),
            wallet: Set(input.recipient),
            collection_id: Set(collection.id),
            tx_signature: Set(None),
            status: Set(CreationStatus::Pending),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        };

        mint_history_am.insert(&tx).await?;

        tx.commit().await?;

        redis_conn
            .del(format!("collection:{}:total_mints", collection.id))
            .await?;

        metadata_json_upload_job_queue
            .enqueue(MetadataJsonUploadTask {
                caller: MetadataJsonUploadCaller::MintToCollection(
                    MetadataJsonUploadMintToCollection {
                        collection_mint_id: collection_mint_model.id,
                    },
                ),
                metadata_json: input.metadata_json,
            })
            .await?;

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

    /// This mutation updates a mint.
    /// # Errors
    /// If the mint cannot be saved to the database or fails to be emitted for submission to the desired blockchain, the mutation will result in an error.
    pub async fn update_mint(
        &self,
        ctx: &Context<'_>,
        input: UpdateMintInput,
    ) -> Result<UpdateMintPayload> {
        let AppContext {
            db,
            user_id,
            organization_id,
            balance,
            ..
        } = ctx.data::<AppContext>()?;
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let conn = db.get();
        let metadata_json_upload_job_queue = ctx.data::<JobQueue>()?;

        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;

        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;
        let org_id = org.ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;

        let creators = input.creators;

        let (mint, collection) = CollectionMints::find_by_id_with_collection(input.id)
            .one(conn)
            .await?
            .ok_or(Error::new("Mint not found"))?;

        if mint.creation_status != CreationStatus::Created {
            return Err(Error::new("Mint not created"));
        }

        if mint.edition > 0 {
            return Err(Error::new("Mint is an edition and cannot be updated"));
        }

        if Some(true) == mint.compressed {
            return Err(Error::new("Mint is compressed and cannot be updated"));
        }

        let collection = collection.ok_or(Error::new("Collection not found"))?;
        let blockchain = collection.blockchain;

        validate_creators(blockchain, &creators)?;
        validate_json(blockchain, &input.metadata_json)?;

        let owner_address = fetch_owner(conn, collection.project_id, collection.blockchain).await?;

        if collection.blockchain == BlockchainEnum::Solana {
            validate_solana_creator_verification(&owner_address, &creators)?;
        }

        let creators_am = creators
            .clone()
            .into_iter()
            .map(|creator| {
                Ok(mint_creators::ActiveModel {
                    collection_mint_id: Set(mint.id),
                    address: Set(creator.address),
                    verified: Set(creator.verified.unwrap_or_default()),
                    share: Set(creator.share.try_into()?),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let TransactionId(deduction_id) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                Actions::UpdateMint,
                collection.blockchain.into(),
                balance,
            )
            .await?;

        let tx = conn.begin().await?;

        let mut mint_am: collection_mints::ActiveModel = mint.clone().into();

        let update_history_am = update_histories::ActiveModel {
            mint_id: Set(mint.id),
            txn_signature: Set(None),
            credit_deduction_id: Set(deduction_id),
            created_by: Set(user_id),
            status: Set(CreationStatus::Pending),
            ..Default::default()
        };

        let update_history = update_history_am.insert(&tx).await?;

        mint_creators::Entity::delete_many()
            .filter(mint_creators::Column::CollectionMintId.eq(mint.id))
            .exec(&tx)
            .await?;

        if !creators_am.is_empty() {
            mint_creators::Entity::insert_many(creators_am)
                .exec(&tx)
                .await?;
        }

        if let Some(sfbp) = input.seller_fee_basis_points {
            mint_am.seller_fee_basis_points = Set(sfbp.try_into().unwrap_or_default());
            mint_am.update(&tx).await?;
        }

        input.metadata_json.save(mint.id, &tx).await?;

        tx.commit().await?;

        metadata_json_upload_job_queue
            .enqueue(MetadataJsonUploadTask {
                caller: MetadataJsonUploadCaller::UpdateMint(MetadataJsonUploadUpdateMint {
                    update_history_id: update_history.id,
                }),
                metadata_json: input.metadata_json,
            })
            .await?;

        Ok(UpdateMintPayload {
            collection_mint: mint.into(),
        })
    }

    /// This mutation retries updating a mint that failed by providing the ID of the `update_history`.
    /// # Errors
    /// If the mint cannot be saved to the database or fails to be emitted for submission to the desired blockchain, the mutation will result in an error.
    pub async fn retry_update_mint(
        &self,
        ctx: &Context<'_>,
        input: RetryUpdateMintInput,
    ) -> Result<RetryUpdateMintPayload> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;

        let conn = db.get();
        let solana = ctx.data::<Solana>()?;

        let UserID(id) = user_id;
        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;

        let (update_history, collection) = update_histories::Entity::find_by_id(input.revision_id)
            .inner_join(CollectionMints)
            .join(
                JoinType::InnerJoin,
                collection_mints::Relation::Collections.def(),
            )
            .select_also(Collections)
            .one(conn)
            .await?
            .ok_or(Error::new("Update history not found"))?;

        let collection = collection.ok_or(Error::new("Collection not found"))?;

        if update_history.status == CreationStatus::Created {
            return Err(Error::new("Mint already updated"));
        }

        let mut update_history_am = update_histories::ActiveModel::from(update_history.clone());
        update_history_am.status = Set(CreationStatus::Pending);
        update_history_am.update(conn).await?;

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .retry_update_mint(
                        NftEventKey {
                            id: update_history.id.to_string(),
                            project_id: collection.project_id.to_string(),
                            user_id: user_id.to_string(),
                        },
                        RetryUpdateSolanaMintPayload {
                            mint_id: update_history.mint_id.to_string(),
                            collection_id: collection.id.to_string(),
                        },
                    )
                    .await?;
            },
            BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        Ok(RetryUpdateMintPayload {
            status: CreationStatus::Pending,
        })
    }

    /// Retries a mint which failed by passing its ID.
    /// # Errors
    pub async fn retry_mint_to_collection(
        &self,
        ctx: &Context<'_>,
        input: RetryMintEditionInput,
    ) -> Result<RetryMintEditionPayload> {
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

        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;

        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = org.ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;

        let (collection_mint_model, collection) =
            collection_mints::Entity::find_by_id_with_collection(input.id)
                .one(conn)
                .await?
                .ok_or(Error::new("collection mint not found"))?;

        if collection_mint_model.creation_status != CreationStatus::Failed {
            return Err(Error::new("only failed mints can be retried"));
        }

        let collection = collection.ok_or(Error::new("collection  not found"))?;

        let recipient = collection_mint_model
            .owner
            .clone()
            .ok_or(Error::new("collection mint does not have an owner"))?;

        let project_id = collection.project_id;
        let blockchain = collection.blockchain;
        let compressed = collection_mint_model.compressed.ok_or(Error::new(
            "collection mint does not have a compressed value",
        ))?;

        let owner_address = fetch_owner(conn, project_id, blockchain).await?;

        let metadata_json = metadata_jsons::Entity::find_by_id(collection_mint_model.id)
            .one(conn)
            .await?
            .ok_or(Error::new("metadata json not found"))?;

        let metadata_uri = metadata_json
            .uri
            .ok_or(Error::new("metadata uri not found"))?;

        let creators = mint_creators::Entity::find_by_collection_mint_id(collection_mint_model.id)
            .all(conn)
            .await?;

        let TransactionId(_) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                Actions::RetryMint,
                collection.blockchain.into(),
                balance,
            )
            .await?;

        let event_key = NftEventKey {
            id: collection_mint_model.id.to_string(),
            user_id: user_id.to_string(),
            project_id: project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .retry_mint_to_collection(event_key, proto::MintMetaplexMetadataTransaction {
                        metadata: Some(MetaplexMetadata {
                            owner_address,
                            name: metadata_json.name,
                            symbol: metadata_json.symbol,
                            metadata_uri,
                            seller_fee_basis_points: collection_mint_model
                                .seller_fee_basis_points
                                .into(),
                            creators: creators.into_iter().map(Into::into).collect(),
                        }),
                        recipient_address: recipient.to_string(),
                        compressed,
                        collection_id: collection_mint_model.collection_id.to_string(),
                    })
                    .await?;
            },
            BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        let mut mint_am: collection_mints::ActiveModel = collection_mint_model.into();
        mint_am.creation_status = Set(CreationStatus::Pending);
        let mint = mint_am.update(conn).await?;

        Ok(RetryMintEditionPayload {
            collection_mint: mint.into(),
        })
    }

    // Add a mint to the queue for a drop.
    // The queued mint can be minted by calling `mint_queued` mutation for specific mint
    // or `mint_random_queued_to_drop` for random mint.
    async fn queue_mint_to_drop(
        &self,
        ctx: &Context<'_>,
        input: QueueMintToDropInput,
    ) -> Result<QueueMintToDropPayload> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;

        let conn = db.get();

        let metadata_json_upload_job_queue = ctx.data::<JobQueue>()?;
        let nfts_producer = ctx.data::<Producer<NftEvents>>()?;

        let UserID(id) = user_id;
        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;

        let (drop, collection) = drops::Entity::find_by_id_with_collection(input.drop)
            .one(conn)
            .await?
            .ok_or(Error::new("drop not found"))?;

        let collection_model = collection.ok_or(Error::new("collection not found"))?;

        let creators = CollectionCreators::find()
            .filter(collection_creators::Column::CollectionId.eq(collection_model.id))
            .all(conn)
            .await?;

        let tx: sea_orm::DatabaseTransaction = conn.begin().await?;

        collections::Entity::update_many()
            .col_expr(
                collections::Column::Supply,
                Expr::value(Expr::col(collections::Column::Supply).add(Value::Int(Some(1)))),
            )
            .filter(collections::Column::Id.eq(collection_model.id))
            .exec(&tx)
            .await?;

        let mint = collection_mints::ActiveModel {
            collection_id: Set(drop.collection_id),
            owner: Set(None),
            creation_status: Set(CreationStatus::Queued),
            created_by: Set(user_id),
            compressed: Set(None),
            seller_fee_basis_points: Set(collection_model.seller_fee_basis_points),
            ..Default::default()
        };

        let mint_model = mint.insert(&tx).await?;

        input.metadata_json.save(mint_model.id, &tx).await?;

        let mint_creators: Vec<_> = creators
            .iter()
            .map(|creator| mint_creators::ActiveModel {
                collection_mint_id: Set(mint_model.id),
                address: Set(creator.address.clone()),
                verified: Set(creator.verified),
                share: Set(creator.share),
            })
            .collect();

        if !mint_creators.is_empty() {
            mint_creators::Entity::insert_many(mint_creators)
                .exec(&tx)
                .await?;
        }

        tx.commit().await?;

        metadata_json_upload_job_queue
            .enqueue(MetadataJsonUploadTask {
                caller: MetadataJsonUploadCaller::QueueMintToDrop(
                    MetadataJsonUploadQueueMintToDrop {
                        drop_id: drop.id,
                        collection_mint_id: mint_model.id,
                    },
                ),
                metadata_json: input.metadata_json,
            })
            .await?;

        nfts_producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::DropMinted(MintCreation {
                        drop_id: drop.id.to_string(),
                        status: NftCreationStatus::Queued as i32,
                    })),
                }),
                Some(&NftEventKey {
                    id: mint_model.id.to_string(),
                    project_id: drop.project_id.to_string(),
                    user_id: mint_model.created_by.to_string(),
                }),
            )
            .await?;

        Ok(QueueMintToDropPayload {
            collection_mint: mint_model.into(),
        })
    }

    /// This mutation mints a specific queued drop mint.
    async fn mint_queued(
        &self,
        ctx: &Context<'_>,
        input: MintQueuedInput,
    ) -> Result<MintQueuedPayload> {
        let AppContext {
            db,
            user_id,
            organization_id,
            balance,
            redis,
            ..
        } = ctx.data::<AppContext>()?;

        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let nfts_producer = ctx.data::<Producer<NftEvents>>()?;
        let solana = ctx.data::<Solana>()?;

        let conn = db.get();
        let mut redis_conn = redis.get_async_connection().await?;

        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;

        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = org.ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;

        let (mint, collection) = collection_mints::Entity::find_by_id_with_collection(input.mint)
            .one(conn)
            .await?
            .ok_or(Error::new("collection mint not found"))?;

        if mint.creation_status != CreationStatus::Queued {
            return Err(Error::new("mint is not queued"));
        }

        let collection = collection.ok_or(Error::new("collection not found"))?;

        let drop = drops::Entity::find()
            .filter(drops::Column::CollectionId.eq(collection.id))
            .one(conn)
            .await?
            .ok_or(Error::new("drop not found"))?;

        let project_id = collection.project_id;
        let blockchain = collection.blockchain;

        let owner_address = fetch_owner(conn, project_id, blockchain).await?;

        let metadata_json = metadata_jsons::Entity::find_by_id(mint.id)
            .one(conn)
            .await?
            .ok_or(Error::new("metadata json not found"))?;

        let metadata_uri = metadata_json
            .uri
            .ok_or(Error::new("No metadata json uri found"))?;

        let event_key = NftEventKey {
            id: mint.id.to_string(),
            user_id: user_id.to_string(),
            project_id: project_id.to_string(),
        };

        let creators = mint_creators::Entity::find_by_collection_mint_id(mint.id)
            .all(conn)
            .await?;

        let action = if input.compressed {
            Actions::MintCompressed
        } else {
            Actions::Mint
        };

        let TransactionId(deduction_id) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                action,
                collection.blockchain.into(),
                balance,
            )
            .await?;

        let tx = conn.begin().await?;

        let mut mint_am: collection_mints::ActiveModel = mint.into();

        mint_am.creation_status = Set(CreationStatus::Pending);
        mint_am.credits_deduction_id = Set(Some(deduction_id));
        mint_am.compressed = Set(Some(input.compressed));
        mint_am.owner = Set(Some(input.recipient.clone()));
        mint_am.seller_fee_basis_points = Set(collection.seller_fee_basis_points);

        let mint = mint_am.update(&tx).await?;

        let mint_history_am = mint_histories::ActiveModel {
            mint_id: Set(mint.id),
            wallet: Set(input.recipient.clone()),
            collection_id: Set(collection.id),
            tx_signature: Set(None),
            status: Set(CreationStatus::Pending),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        };

        mint_history_am.insert(&tx).await?;

        tx.commit().await?;

        redis_conn
            .del(format!("collection:{}:total_mints", collection.id))
            .await?;

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .mint_drop(
                        event_key,
                        MintDropTransaction::Open(proto::MintMetaplexMetadataTransaction {
                            metadata: Some(MetaplexMetadata {
                                owner_address,
                                name: metadata_json.name,
                                symbol: metadata_json.symbol,
                                metadata_uri,
                                seller_fee_basis_points: mint.seller_fee_basis_points.into(),
                                creators: creators.into_iter().map(Into::into).collect(),
                            }),
                            recipient_address: input.recipient.to_string(),
                            compressed: input.compressed,
                            collection_id: collection.id.to_string(),
                        }),
                    )
                    .await?;
            },
            BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        nfts_producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::DropMinted(MintCreation {
                        drop_id: drop.id.to_string(),
                        status: NftCreationStatus::InProgress as i32,
                    })),
                }),
                Some(&NftEventKey {
                    id: mint.id.to_string(),
                    project_id: drop.project_id.to_string(),
                    user_id: user_id.to_string(),
                }),
            )
            .await?;

        Ok(MintQueuedPayload {
            collection_mint: mint.into(),
        })
    }

    /// This mutation mints a random queued drop mint.
    async fn mint_random_queued_to_drop(
        &self,
        ctx: &Context<'_>,
        input: MintRandomQueuedInput,
    ) -> Result<MintQueuedPayload> {
        let AppContext {
            db,
            user_id,
            organization_id,
            balance,
            redis,
            ..
        } = ctx.data::<AppContext>()?;
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let conn = db.get();
        let mut redis_conn = redis.get_async_connection().await?;
        let solana = ctx.data::<Solana>()?;
        let nfts_producer = ctx.data::<Producer<NftEvents>>()?;

        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;

        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = org.ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;

        let drop = drops::Entity::find_by_id(input.drop)
            .one(conn)
            .await?
            .ok_or(Error::new("drop not found"))?;

        let mint = CollectionMints::find()
            .filter(collection_mints::Column::CollectionId.eq(drop.collection_id))
            .filter(collection_mints::Column::CreationStatus.eq(CreationStatus::Queued))
            .order_by(collection_mints::Column::RandomPick, Order::Asc)
            .one(conn)
            .await?
            .ok_or(Error::new("No Queued mint found for the drop"))?;

        let collection = collections::Entity::find_by_id(drop.collection_id)
            .one(conn)
            .await?
            .ok_or(Error::new("collection not found"))?;

        let project_id = collection.project_id;
        let blockchain = collection.blockchain;

        let owner_address = fetch_owner(conn, project_id, blockchain).await?;

        let metadata_json = metadata_jsons::Entity::find_by_id(mint.id)
            .one(conn)
            .await?
            .ok_or(Error::new("metadata json not found"))?;

        let metadata_uri = metadata_json
            .uri
            .ok_or(Error::new("No metadata json uri found"))?;

        let creators = mint_creators::Entity::find_by_collection_mint_id(mint.id)
            .all(conn)
            .await?;

        let action = if input.compressed {
            Actions::MintCompressed
        } else {
            Actions::Mint
        };

        let TransactionId(deduction_id) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                action,
                collection.blockchain.into(),
                balance,
            )
            .await?;

        let tx = conn.begin().await?;

        let mut mint_am: collection_mints::ActiveModel = mint.into();

        mint_am.creation_status = Set(CreationStatus::Pending);
        mint_am.credits_deduction_id = Set(Some(deduction_id));
        mint_am.compressed = Set(Some(input.compressed));
        mint_am.owner = Set(Some(input.recipient.clone()));
        mint_am.seller_fee_basis_points = Set(collection.seller_fee_basis_points);

        let mint = mint_am.update(&tx).await?;

        let mint_history_am = mint_histories::ActiveModel {
            mint_id: Set(mint.id),
            wallet: Set(input.recipient.clone()),
            collection_id: Set(collection.id),
            tx_signature: Set(None),
            status: Set(CreationStatus::Pending),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        };

        mint_history_am.insert(&tx).await?;

        tx.commit().await?;

        redis_conn
            .del(format!("collection:{}:total_mints", collection.id))
            .await?;

        let event_key = NftEventKey {
            id: mint.id.to_string(),
            user_id: user_id.to_string(),
            project_id: project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .mint_drop(
                        event_key,
                        MintDropTransaction::Open(proto::MintMetaplexMetadataTransaction {
                            metadata: Some(MetaplexMetadata {
                                owner_address,
                                name: metadata_json.name,
                                symbol: metadata_json.symbol,
                                metadata_uri,
                                seller_fee_basis_points: mint.seller_fee_basis_points.into(),
                                creators: creators.into_iter().map(Into::into).collect(),
                            }),
                            recipient_address: input.recipient.to_string(),
                            compressed: input.compressed,
                            collection_id: collection.id.to_string(),
                        }),
                    )
                    .await?;
            },
            BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        nfts_producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::DropMinted(MintCreation {
                        drop_id: drop.id.to_string(),
                        status: NftCreationStatus::InProgress as i32,
                    })),
                }),
                Some(&NftEventKey {
                    id: mint.id.to_string(),
                    project_id: drop.project_id.to_string(),
                    user_id: user_id.to_string(),
                }),
            )
            .await?;

        Ok(MintQueuedPayload {
            collection_mint: mint.into(),
        })
    }

    async fn mint_random_queued_to_drop_batched(
        &self,
        ctx: &Context<'_>,
        input: MintRandomQueuedBatchedInput,
    ) -> Result<MintRandomQueuedBatchedPayload> {
        let AppContext {
            db,
            user_id,
            organization_id,
            balance,
            ..
        } = ctx.data::<AppContext>()?;
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let conn = db.get();
        let nfts_producer = ctx.data::<Producer<NftEvents>>()?;

        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;

        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = org.ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;

        let batch_size = input.recipients.len();

        if batch_size == 0 {
            return Err(Error::new("No recipients provided"));
        }

        if batch_size > 250 {
            return Err(Error::new("Batch size cannot be greater than 250"));
        }

        let drop = drops::Entity::find_by_id(input.drop)
            .one(conn)
            .await?
            .ok_or(Error::new("drop not found"))?;

        let result = CollectionMints::find()
            .select_also(metadata_jsons::Entity)
            .join(
                JoinType::InnerJoin,
                collection_mints::Entity::belongs_to(metadata_jsons::Entity)
                    .from(collection_mints::Column::Id)
                    .to(metadata_jsons::Column::Id)
                    .into(),
            )
            .filter(collection_mints::Column::CollectionId.eq(drop.collection_id))
            .filter(collection_mints::Column::CreationStatus.eq(CreationStatus::Queued))
            .order_by(collection_mints::Column::RandomPick, Order::Asc)
            .limit(Some(batch_size.try_into()?))
            .all(conn)
            .await?;

        let (mints, _): (Vec<_>, Vec<_>) = result.iter().cloned().unzip();

        let creators = mints.load_many(mint_creators::Entity, conn).await?;

        if mints.len() != batch_size {
            return Err(Error::new("Not enough mints found for the drop"));
        }

        let collection = collections::Entity::find_by_id(drop.collection_id)
            .one(conn)
            .await?
            .ok_or(Error::new("collection not found"))?;

        let project_id = collection.project_id;
        let blockchain = collection.blockchain;

        if blockchain != BlockchainEnum::Solana {
            return Err(Error::new("Only Solana is supported at this time"));
        }

        let owner_address = fetch_owner(conn, project_id, blockchain).await?;

        let action = if input.compressed {
            Actions::MintCompressed
        } else {
            Actions::Mint
        };

        let event_key = NftEventKey {
            id: collection.id.to_string(),
            user_id: user_id.to_string(),
            project_id: project_id.to_string(),
        };

        let mut transactions = Vec::new();

        for (((mint, metadata_json), creators), recipient) in result
            .into_iter()
            .zip(creators.into_iter())
            .zip(input.recipients.into_iter())
        {
            let metadata_json = metadata_json.ok_or(Error::new("No metadata json found"))?;
            let metadata_uri = metadata_json
                .uri
                .ok_or(Error::new("No metadata json uri found"))?;

            let TransactionId(deduction_id) = credits
                .submit_pending_deduction(
                    org_id,
                    user_id,
                    action,
                    collection.blockchain.into(),
                    balance,
                )
                .await?;

            let tx = conn.begin().await?;

            let mut mint_am: collection_mints::ActiveModel = mint.into();

            mint_am.creation_status = Set(CreationStatus::Pending);
            mint_am.credits_deduction_id = Set(Some(deduction_id));
            mint_am.compressed = Set(Some(input.compressed));
            mint_am.owner = Set(Some(recipient.clone()));
            mint_am.seller_fee_basis_points = Set(collection.seller_fee_basis_points);

            let mint = mint_am.update(&tx).await?;

            let mint_history_am = mint_histories::ActiveModel {
                mint_id: Set(mint.id),
                wallet: Set(recipient.clone()),
                collection_id: Set(collection.id),
                tx_signature: Set(None),
                status: Set(CreationStatus::Pending),
                created_at: Set(Utc::now().into()),
                ..Default::default()
            };

            mint_history_am.insert(&tx).await?;

            tx.commit().await?;

            nfts_producer
                .send(
                    Some(&NftEvents {
                        event: Some(NftEvent::DropMinted(MintCreation {
                            drop_id: drop.id.to_string(),
                            status: NftCreationStatus::InProgress as i32,
                        })),
                    }),
                    Some(&NftEventKey {
                        id: mint.id.to_string(),
                        project_id: drop.project_id.to_string(),
                        user_id: user_id.to_string(),
                    }),
                )
                .await?;

            transactions.push(MintOpenDropTransaction {
                recipient_address: recipient,
                metadata: Some(MetaplexMetadata {
                    owner_address: owner_address.clone(),
                    name: metadata_json.name,
                    symbol: metadata_json.symbol,
                    metadata_uri,
                    seller_fee_basis_points: mint.seller_fee_basis_points.into(),
                    creators: creators.into_iter().map(Into::into).collect(),
                }),
                mint_id: mint.id.to_string(),
            });
        }

        nfts_producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::SolanaMintOpenDropBatched(
                        SolanaMintOpenDropBatchedPayload {
                            collection_id: collection.id.to_string(),
                            compressed: input.compressed,
                            mint_open_drop_transactions: transactions,
                        },
                    )),
                }),
                Some(&event_key),
            )
            .await?;

        Ok(MintRandomQueuedBatchedPayload {
            collection_mints: mints.into_iter().map(Into::into).collect(),
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
    collection_mint: CollectionMint,
}

/// Represents input data for `retry_mint` mutation with an ID as a field of type UUID
#[derive(Debug, Clone, InputObject)]
pub struct RetryMintEditionInput {
    id: Uuid,
}

/// Represents payload data for `retry_mint` mutation
#[derive(Debug, Clone, SimpleObject)]
pub struct RetryMintEditionPayload {
    collection_mint: CollectionMint,
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

#[derive(Debug, Clone, InputObject)]
pub struct UpdateMintInput {
    /// The ID of the mint to be updated
    id: Uuid,
    /// The metadata of the mint
    metadata_json: MetadataJsonInput,
    /// The optional seller fee basis points
    seller_fee_basis_points: Option<u16>,
    /// The creators to be assigned to the NFT.
    /// For Solana, this can be up to five creators. If the project treasury wallet is set as a creator and verified set to true the creator will be verified on chain.
    /// For Polygon, this can be only 1 creator.
    creators: Vec<Creator>,
}

/// Represents payload data for `mint_to_collection` mutation
#[derive(Debug, Clone, SimpleObject)]
pub struct MintToCollectionPayload {
    /// The minted NFT
    collection_mint: CollectionMint,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct UpdateMintPayload {
    collection_mint: CollectionMint,
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
    collection_mint: CollectionMint,
}

#[derive(Debug, Clone, InputObject)]
pub struct RetryUpdateMintInput {
    /// Update History ID
    revision_id: Uuid,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct RetryUpdateMintPayload {
    status: CreationStatus,
}

/// Represents input data for `queue_mint_to_drop` mutation
#[derive(Debug, Clone, InputObject)]
pub struct QueueMintToDropInput {
    drop: Uuid,
    metadata_json: MetadataJsonInput,
}

/// Represents payload data for `queue_mint_to_drop` mutation
#[derive(Debug, Clone, SimpleObject)]
pub struct QueueMintToDropPayload {
    collection_mint: CollectionMint,
}

/// Represents input data for `mint_queued` mutation
#[derive(Debug, Clone, InputObject)]
pub struct MintQueuedInput {
    mint: Uuid,
    recipient: String,
    compressed: bool,
}

/// Represents payload data for `mint_queued` mutation
#[derive(Debug, Clone, SimpleObject)]
pub struct MintQueuedPayload {
    collection_mint: CollectionMint,
}

/// Represents input data for `mint_random_queued` mutation
#[derive(Debug, Clone, InputObject)]
pub struct MintRandomQueuedInput {
    drop: Uuid,
    recipient: String,
    compressed: bool,
}

/// Represents input data for `mint_random_queued_batched` mutation
#[derive(Debug, Clone, InputObject)]
pub struct MintRandomQueuedBatchedInput {
    drop: Uuid,
    recipients: Vec<String>,
    compressed: bool,
}

/// Represents payload data for `mint_random_queued_batched` mutation
#[derive(Debug, Clone, SimpleObject)]
pub struct MintRandomQueuedBatchedPayload {
    collection_mints: Vec<CollectionMint>,
}
