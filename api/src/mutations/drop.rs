use async_graphql::{Context, Error, InputObject, Object, Result};
use chrono::{DateTime, Local, Utc};
use hub_core::producer::Producer;
use mpl_token_metadata::state::Creator;
use sea_orm::{prelude::*, Set};
use serde::{Deserialize, Serialize};

use crate::{
    blockchains::{
        solana::{CreateDropPayload, Solana},
        Blockchain, TransactionResponse,
    },
    collection::Collection,
    entities::{
        collections, drops, project_wallets,
        sea_orm_active_enums::{Blockchain as BlockchainEnum, CreationStatus},
    },
    metadata_json::MetadataJson,
    objects::{CollectionCreator, MetadataJsonInput},
    proto::{self, nft_events, NftEventKey, NftEvents},
    AppContext, NftStorageClient, UserID,
};

#[derive(Default)]
pub struct Mutation;

#[Object(name = "DropMutation")]
impl Mutation {
    /// Res
    ///
    /// # Errors
    /// This function fails if ...
    pub async fn create_drop(
        &self,
        ctx: &Context<'_>,
        input: CreateDropInput,
    ) -> Result<drops::Model> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let UserID(id) = user_id;
        let producer = ctx.data::<Producer<NftEvents>>()?;
        let solana = ctx.data::<Solana>()?;
        let nft_storage = ctx.data::<NftStorageClient>()?;

        let user_id = id.ok_or_else(|| Error::new("X-USER-ID header not found"))?;

        let wallet = project_wallets::Entity::find()
            .filter(
                project_wallets::Column::ProjectId
                    .eq(input.project_id)
                    .and(project_wallets::Column::Blockchain.eq(input.blockchain)),
            )
            .one(db.get())
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

        let TransactionResponse {
            serialized_message,
            signed_message_signatures,
        } = match input.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .drop(CreateDropPayload {
                        owner_address,
                        creators: input
                            .creators
                            .into_iter()
                            .map(TryInto::try_into)
                            .collect::<Result<Vec<Creator>>>()?,
                        name: metadata_json_model.name,
                        symbol: metadata_json_model.symbol,
                        seller_fee_basis_points: input.seller_fee_basis_points,
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

        let drop = drops::ActiveModel {
            project_id: Set(input.project_id),
            collection_id: Set(collection.id),
            creation_status: Set(CreationStatus::Pending),
            start_time: Set(input.start_time.naive_utc()),
            end_time: Set(input.end_time.naive_utc()),
            price: Set(input.price.try_into()?),
            created_by: Set(user_id),
            created_at: Set(Local::now().naive_utc()),
            ..Default::default()
        };

        let drop_model = drop.insert(db.get()).await?;

        emit_drop_transaction_event(
            producer,
            drop_model.id,
            user_id,
            serialized_message,
            signed_message_signatures,
            input.project_id,
        )
        .await?;

        Ok(drop_model)
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

#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct CreateDropInput {
    pub project_id: Uuid,
    pub price: u64,
    pub seller_fee_basis_points: u16,
    pub supply: Option<u64>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
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
