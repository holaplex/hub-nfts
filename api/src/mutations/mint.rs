use std::ops::Add;

use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::{
    chrono::Utc,
    credits::{CreditsClient, TransactionId},
    prelude::*,
    producer::Producer,
};
use sea_orm::{prelude::*, JoinType, QuerySelect, Set, TransactionTrait};

use super::collection::{
    fetch_owner, validate_creators, validate_json, validate_solana_creator_verification,
};
use crate::{
    blockchains::{polygon::Polygon, solana::Solana, CollectionEvent, DropEvent},
    entities::{
        collection_mints, collections, drops, mint_creators, mint_histories,
        prelude::{CollectionMints, Collections, Drops},
        project_wallets,
        sea_orm_active_enums::{Blockchain as BlockchainEnum, CreationStatus},
        update_histories,
    },
    metadata_json::{self, MetadataJson},
    objects::{CollectionMint, Creator, MetadataJsonInput},
    prepared_creator::PreparedCreator,
    proto::{
        self, nft_events::Event as NftEvent, CreationStatus as NftCreationStatus, MetaplexMetadata,
        MintCollectionCreation, MintCreation, NftEventKey, NftEvents, RetryUpdateSolanaMintPayload,
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
            .await?;

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
                    .save(collection_mint_model.id, db, None)
                    .await?;

                solana
                    .event()
                    .mint_drop(
                        event_key,
                        proto::MintMetaplexEditionTransaction {
                            recipient_address: input.recipient.to_string(),
                            owner_address: owner_address.to_string(),
                            edition,
                            collection_id: collection.id.to_string(),
                        },
                    )
                    .await?;
            },
            BlockchainEnum::Polygon => {
                polygon
                    .event()
                    .mint_drop(
                        event_key,
                        proto::MintEditionTransaction {
                            receiver: input.recipient.to_string(),
                            amount: 1,
                            collection_id: collection.id.to_string(),
                        },
                    )
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
                        proto::MintMetaplexEditionTransaction {
                            recipient_address: recipient.to_string(),
                            owner_address: owner_address.to_string(),
                            edition,
                            collection_id: collection.id.to_string(),
                        },
                    )
                    .await?;
            },
            BlockchainEnum::Polygon => {
                polygon
                    .event()
                    .retry_mint_drop(
                        event_key,
                        proto::MintEditionTransaction {
                            receiver: recipient.to_string(),
                            amount: 1,
                            collection_id: collection.id.to_string(),
                        },
                    )
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
        let job_runner = ctx.data::<metadata_json::JobRunner>()?;

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

        for creator in creators.clone() {
            let am = mint_creators::ActiveModel {
                collection_mint_id: Set(collection_mint_model.id),
                address: Set(creator.address),
                verified: Set(creator.verified.unwrap_or_default()),
                share: Set(creator.share.try_into()?),
            };

            am.insert(conn).await?;
        }

        let mut collection_am = collections::ActiveModel::from(collection.clone());
        collection_am.total_mints = Set(collection.total_mints.add(1));
        collection_am.update(conn).await?;

        let mint_history_am = mint_histories::ActiveModel {
            mint_id: Set(collection_mint_model.id),
            wallet: Set(input.recipient.clone()),
            collection_id: Set(collection.id),
            tx_signature: Set(None),
            status: Set(CreationStatus::Pending),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        };

        mint_history_am.insert(conn).await?;

        MetadataJson::new(input.metadata_json)
            .save(
                collection_mint_model.id,
                db,
                (
                    job_runner,
                    metadata_json::Continuation::MintToCollection(FinishMintToCollectionArgs {
                        user_id,
                        project_id: collection.project_id,
                        collection_id: collection.id,
                        collection_mint_id: collection_mint_model.id,
                        blockchain,
                        owner_address,
                        recipient: input.recipient,
                        seller_fee_basis_points: input
                            .seller_fee_basis_points
                            .map_or_else(Default::default, Into::into),
                        compressed,
                        creators: creators
                            .into_iter()
                            .map(proto::Creator::try_from)
                            .map(|r| r.map(Into::into))
                            .collect::<Result<_>>()?,
                    }),
                ),
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
        let job_runner = ctx.data::<metadata_json::JobRunner>()?;

        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;

        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;
        let org_id = org.ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;

        let creators = input.creators;

        let (mint, collection) = CollectionMints::find()
            .find_also_related(Collections)
            .filter(collection_mints::Column::Id.eq(input.id))
            .one(conn)
            .await?
            .ok_or(Error::new("Mint not found"))?;

        if mint.creation_status != CreationStatus::Created {
            return Err(Error::new("Mint not created"));
        }

        if mint.edition > 0 {
            return Err(Error::new("Mint is an edition and cannot be updated"));
        }

        if mint.compressed {
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

        let deduction_id = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                Actions::UpdateMint,
                collection.blockchain.into(),
                balance,
            )
            .await?;

        let mut mint_am: collection_mints::ActiveModel = mint.clone().into();

        let update_history_am = update_histories::ActiveModel {
            mint_id: Set(mint.id),
            txn_signature: Set(None),
            credit_deduction_id: Set(deduction_id.0),
            created_by: Set(user_id),
            status: Set(CreationStatus::Pending),
            ..Default::default()
        };

        let update_history = update_history_am.insert(db.get()).await?;
        conn.transaction::<_, (), DbErr>(|txn| {
            Box::pin(async move {
                mint_creators::Entity::delete_many()
                    .filter(mint_creators::Column::CollectionMintId.eq(mint.id))
                    .exec(txn)
                    .await?;

                mint_creators::Entity::insert_many(creators_am)
                    .exec(txn)
                    .await?;

                if let Some(sfbp) = input.seller_fee_basis_points {
                    mint_am.seller_fee_basis_points = Set(sfbp.try_into().unwrap_or_default());
                    mint_am.update(txn).await?;
                }

                Ok(())
            })
        })
        .await?;

        MetadataJson::new(input.metadata_json)
            .save(
                mint.id,
                db,
                (
                    job_runner,
                    metadata_json::Continuation::UpdateMint(FinishUpdateMintArgs {
                        user_id,
                        project_id: collection.project_id,
                        collection_id: collection.id,
                        mint_id: mint.id,
                        update_history_id: update_history.id,
                        blockchain,
                        owner_address,
                        seller_fee_basis_points: mint.seller_fee_basis_points.try_into()?,
                        creators: creators
                            .into_iter()
                            .map(proto::Creator::try_from)
                            .map(|r| r.map(Into::into))
                            .collect::<Result<_>>()?,
                    }),
                ),
            )
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

        if update_history.status == CreationStatus::Created {
            return Err(Error::new("Mint already updated"));
        }

        let mut update_history_am = update_histories::ActiveModel::from(update_history.clone());
        update_history_am.status = Set(CreationStatus::Pending);
        update_history_am.update(db.get()).await?;

        let collection = collection.ok_or(Error::new("Collection not found"))?;

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

    // Retries a mint which failed by passing its ID.
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

        let recipient = collection_mint_model.owner.clone();
        let project_id = collection.project_id;
        let blockchain = collection.blockchain;

        let owner_address = fetch_owner(conn, project_id, blockchain).await?;

        let MetadataJson {
            metadata_json,
            upload,
        } = MetadataJson::fetch(collection_mint_model.id, db).await?;
        let upload = upload.context("Metadata JSON found but not uploaded")?;

        let event_key = NftEventKey {
            id: collection_mint_model.id.to_string(),
            user_id: user_id.to_string(),
            project_id: project_id.to_string(),
        };

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

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .retry_mint_to_collection(
                        event_key,
                        proto::MintMetaplexMetadataTransaction {
                            metadata: Some(MetaplexMetadata {
                                owner_address,
                                name: metadata_json.name,
                                symbol: metadata_json.symbol,
                                metadata_uri: upload.uri,
                                seller_fee_basis_points: collection_mint_model
                                    .seller_fee_basis_points
                                    .into(),
                                creators: creators.into_iter().map(Into::into).collect(),
                            }),
                            recipient_address: recipient.to_string(),
                            compressed: collection_mint_model.compressed,
                            collection_id: collection_mint_model.collection_id.to_string(),
                        },
                    )
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FinishMintToCollectionArgs {
    user_id: Uuid,
    project_id: Uuid,
    collection_id: Uuid,
    collection_mint_id: Uuid,
    blockchain: BlockchainEnum,
    owner_address: String,
    recipient: String,
    seller_fee_basis_points: i32,
    compressed: bool,
    creators: Vec<PreparedCreator>,
}

pub async fn finish_mint_to_collection(
    ctx: &metadata_json::JobContext<'_>,
    args: FinishMintToCollectionArgs,
) -> metadata_json::JobResult {
    let FinishMintToCollectionArgs {
        user_id,
        project_id,
        collection_id,
        collection_mint_id,
        blockchain,
        owner_address,
        recipient,
        seller_fee_basis_points,
        compressed,
        creators,
    } = args;

    let event_key = NftEventKey {
        id: collection_mint_id.to_string(),
        user_id: user_id.to_string(),
        project_id: project_id.to_string(),
    };

    match blockchain {
        BlockchainEnum::Solana => {
            ctx.solana
                .event()
                .mint_to_collection(
                    event_key.clone(),
                    proto::MintMetaplexMetadataTransaction {
                        metadata: Some(MetaplexMetadata {
                            owner_address,
                            name: ctx.metadata_json.name.clone(),
                            symbol: ctx.metadata_json.symbol.clone(),
                            metadata_uri: ctx.upload.uri.clone(),
                            seller_fee_basis_points,
                            creators: creators.into_iter().map(Into::into).collect(),
                        }),
                        recipient_address: recipient,
                        compressed,
                        collection_id: collection_id.to_string(),
                    },
                )
                .await?;
        },
        BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
            bail!("Blockchain not supported currently");
        },
    };

    ctx.nfts_producer
        .send(
            Some(&NftEvents {
                event: Some(NftEvent::MintedToCollection(MintCollectionCreation {
                    collection_id: collection_id.to_string(),
                    status: NftCreationStatus::InProgress as i32,
                })),
            }),
            Some(&event_key),
        )
        .await?;

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FinishUpdateMintArgs {
    user_id: Uuid,
    project_id: Uuid,
    collection_id: Uuid,
    mint_id: Uuid,
    update_history_id: Uuid,
    blockchain: BlockchainEnum,
    owner_address: String,
    seller_fee_basis_points: i32,
    creators: Vec<PreparedCreator>,
}

pub async fn finish_update_mint(
    ctx: &metadata_json::JobContext<'_>,
    args: FinishUpdateMintArgs,
) -> metadata_json::JobResult {
    let FinishUpdateMintArgs {
        user_id,
        project_id,
        collection_id,
        mint_id,
        update_history_id,
        blockchain,
        owner_address,
        seller_fee_basis_points,
        creators,
    } = args;

    match blockchain {
        BlockchainEnum::Solana => {
            ctx.solana
                .event()
                .update_collection_mint(
                    NftEventKey {
                        id: update_history_id.to_string(),
                        project_id: project_id.to_string(),
                        user_id: user_id.to_string(),
                    },
                    proto::UpdateSolanaMintPayload {
                        metadata: Some(MetaplexMetadata {
                            owner_address,
                            name: ctx.metadata_json.name.clone(),
                            symbol: ctx.metadata_json.symbol.clone(),
                            metadata_uri: ctx.upload.uri.clone(),
                            seller_fee_basis_points,
                            creators: creators.into_iter().map(Into::into).collect(),
                        }),
                        collection_id: collection_id.to_string(),
                        mint_id: mint_id.to_string(),
                    },
                )
                .await?;
        },
        BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
            bail!("Blockchain not supported currently");
        },
    }

    Ok(())
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
