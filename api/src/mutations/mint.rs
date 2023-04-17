use std::ops::Add;

use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::{chrono::Utc, producer::Producer};
use sea_orm::{prelude::*, JoinType, QuerySelect, Set};

use crate::{
    blockchains::{
        solana::{CreateEditionRequest, Solana},
        Edition, TransactionResponse,
    },
    entities::{
        collection_mints, collections, drops,
        prelude::{Collections, Drops},
        project_wallets, purchases,
        sea_orm_active_enums::{Blockchain as BlockchainEnum, CreationStatus},
    },
    metadata_json::MetadataJson,
    proto::{self, nft_events, MintTransaction, NftEventKey, NftEvents, Transaction},
    AppContext, UserID,
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
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let producer = ctx.data::<Producer<NftEvents>>()?;
        let conn = db.get();
        let solana = ctx.data::<Solana>()?;

        let UserID(id) = user_id;
        let user_id = id.ok_or_else(|| Error::new("X-USER-ID header not found"))?;

        let drop_model = Drops::find()
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .select_also(Collections)
            .filter(drops::Column::Id.eq(input.drop))
            .one(conn)
            .await?;

        let (drop_model, collection_model) =
            drop_model.ok_or_else(|| Error::new("drop not found"))?;

        // Call check_drop_status to check that drop is currently running
        check_drop_status(&drop_model).await?;

        let collection = collection_model.ok_or_else(|| Error::new("collection not found"))?;

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
            .ok_or_else(|| {
                Error::new(format!(
                    "no project wallet found for {} blockchain",
                    collection.blockchain
                ))
            })?
            .wallet_address;

        let (
            mint_address,
            TransactionResponse {
                serialized_message,
                signed_message_signatures,
            },
        ) = match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .edition()
                    .mint(CreateEditionRequest {
                        collection: collection.id,
                        recipient: input.recipient.clone(),
                        owner_address,
                        edition: edition.try_into()?,
                    })
                    .await?
            },
            BlockchainEnum::Polygon | BlockchainEnum::Ethereum => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        // insert a collection mint record into database
        let collection_mint_active_model = collection_mints::ActiveModel {
            collection_id: Set(collection.id),
            address: Set(mint_address.to_string()),
            owner: Set(input.recipient.clone()),
            creation_status: Set(CreationStatus::Pending),
            seller_fee_basis_points: Set(collection.seller_fee_basis_points),
            created_by: Set(user_id),
            edition: Set(edition),
            ..Default::default()
        };

        let collection_mint_model = collection_mint_active_model.insert(conn).await?;

        let mut collection_am = collections::ActiveModel::from(collection.clone());
        collection_am.total_mints = Set(edition);
        collection_am.update(conn).await?;

        let proto_blockchain_enum: proto::Blockchain = collection.blockchain.into();

        MetadataJson::fetch(collection.id, db)
            .await?
            .save(collection_mint_model.id, db)
            .await?;

        // inserts a purchase record in the database
        let purchase_am = purchases::ActiveModel {
            mint_id: Set(collection_mint_model.id),
            wallet: Set(input.recipient),
            spent: Set(drop_model.price),
            drop_id: Set(Some(drop_model.id)),
            tx_signature: Set(None),
            status: Set(CreationStatus::Pending),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        purchase_am.insert(conn).await?;

        // emit `MintDrop` event
        let event = NftEvents {
            event: Some(nft_events::Event::MintDrop(MintTransaction {
                transaction: Some(Transaction {
                    serialized_message,
                    signed_message_signatures,
                    blockchain: proto_blockchain_enum as i32,
                }),
                project_id: drop_model.project_id.to_string(),
                drop_id: drop_model.id.to_string(),
            })),
        };
        let key = NftEventKey {
            id: collection_mint_model.id.to_string(),
            user_id: user_id.to_string(),
        };

        producer.send(Some(&event), Some(&key)).await?;

        Ok(MintEditionPayload {
            collection_mint: collection_mint_model.into(),
        })
    }

    /// This mutation retries a mint which failed or is in pending state. The mint returns immediately with a creation status of CREATING. You can [set up a webhook](https://docs.holaplex.dev/hub/For%20Developers/webhooks-overview) to receive a notification when the mint is accepted by the blockchain.
    /// # Errors
    /// If the mint cannot be saved to the database or fails to be emitted for submission to the desired blockchain, the mutation will result in an error.
    pub async fn retry_mint(
        &self,
        ctx: &Context<'_>,
        input: RetryMintInput,
    ) -> Result<RetryMintPayload> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let producer = ctx.data::<Producer<NftEvents>>()?;
        let conn = db.get();
        let solana = ctx.data::<Solana>()?;

        let UserID(id) = user_id;
        let user_id = id.ok_or_else(|| Error::new("X-USER-ID header not found"))?;

        let (collection_mint_model, drop) = collection_mints::Entity::find()
            .join(
                JoinType::InnerJoin,
                collection_mints::Relation::Collections.def(),
            )
            .join(JoinType::InnerJoin, collections::Relation::Drops.def())
            .select_also(drops::Entity)
            .filter(collection_mints::Column::Id.eq(input.id))
            .one(conn)
            .await?
            .ok_or_else(|| Error::new("collection mint not found"))?;

        if collection_mint_model.creation_status == CreationStatus::Created {
            return Err(Error::new("mint is already created"));
        }

        let collection = collections::Entity::find()
            .filter(collections::Column::Id.eq(collection_mint_model.collection_id))
            .one(conn)
            .await?
            .ok_or_else(|| Error::new("collection not found"))?;

        let drop_model = drop.ok_or_else(|| Error::new("drop not found"))?;

        let recipient = collection_mint_model.owner.clone();
        let edition = collection_mint_model.edition;

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
            .ok_or_else(|| {
                Error::new(format!(
                    "no project wallet found for {} blockchain",
                    collection.blockchain
                ))
            })?
            .wallet_address;

        let (
            _,
            TransactionResponse {
                serialized_message,
                signed_message_signatures,
            },
        ) = match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .edition()
                    .mint(CreateEditionRequest {
                        collection: collection.id,
                        recipient,
                        owner_address,
                        edition: edition.try_into()?,
                    })
                    .await?
            },
            BlockchainEnum::Polygon | BlockchainEnum::Ethereum => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        let proto_blockchain_enum: proto::Blockchain = collection.blockchain.into();

        // emit `MintDrop` event
        let event = NftEvents {
            event: Some(nft_events::Event::MintDrop(MintTransaction {
                transaction: Some(Transaction {
                    serialized_message,
                    signed_message_signatures,
                    blockchain: proto_blockchain_enum as i32,
                }),
                project_id: drop_model.project_id.to_string(),
                drop_id: drop_model.id.to_string(),
            })),
        };
        let key = NftEventKey {
            id: collection_mint_model.id.clone().to_string(),
            user_id: user_id.to_string(),
        };

        producer.send(Some(&event), Some(&key)).await?;

        Ok(RetryMintPayload {
            collection_mint: collection_mint_model.into(),
        })
    }
}
/// Checks the status of a drop by verifying if it is currently running based on its start time, end time, and pause/shutdown status.
/// # Errors
///
/// This function returns an error if the drop is paused,
/// shutdown, has not yet started, or has already ended based
async fn check_drop_status(drop_model: &drops::Model) -> Result<(), Error> {
    drop_model
        .paused_at
        .map_or(Ok(()), |_| Err(Error::new("Drop status is paused")))?;

    drop_model
        .shutdown_at
        .map_or(Ok(()), |_| Err(Error::new("Drop status is shutdown")))?;

    drop_model.start_time.map_or(Ok(()), |start_time| {
        if start_time <= Utc::now().naive_utc() {
            Ok(())
        } else {
            Err(Error::new("Drop has not yet started"))
        }
    })?;

    drop_model.end_time.map_or(Ok(()), |end_time| {
        if end_time > Utc::now().naive_utc() {
            Ok(())
        } else {
            Err(Error::new("Drop has already ended"))
        }
    })?;

    Ok(())
}

/// Represents input data for `mint_edition` mutation with a UUID and recipient as fields
#[derive(Debug, Clone, InputObject)]
pub struct MintDropInput {
    drop: Uuid,
    recipient: String,
}

/// Represents payload data for the `mint_edition` mutation
#[derive(Debug, Clone, SimpleObject)]
pub struct MintEditionPayload {
    collection_mint: collection_mints::CollectionMint,
}

/// Represents input data for `retry_mint` mutation with an ID as a field of type UUID
#[derive(Debug, Clone, InputObject)]
pub struct RetryMintInput {
    id: Uuid,
}

/// Represents payload data for `retry_mint` mutation
#[derive(Debug, Clone, SimpleObject)]
pub struct RetryMintPayload {
    collection_mint: collection_mints::CollectionMint,
}
