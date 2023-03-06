use std::ops::Add;

use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::producer::Producer;
use sea_orm::{prelude::*, JoinType, QuerySelect, Set};

use crate::{
    blockchains::{
        solana::{CreateEditionRequest, Solana},
        Blockchain, TransactionResponse,
    },
    entities::{
        collection_mints, drops,
        prelude::{Collections, Drops},
        project_wallets,
        sea_orm_active_enums::{Blockchain as BlockchainEnum, CreationStatus},
    },
    proto::{nft_events, MintTransaction, NftEventKey, NftEvents, Transaction},
    AppContext, UserID,
};

#[derive(Default)]
pub struct Mutation;

#[Object(name = "MintMutation")]
impl Mutation {
    /// Res
    ///
    /// # Errors
    /// This function fails if ...
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
            .select_also(Collections)
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .filter(drops::Column::Id.eq(input.drop))
            .one(conn)
            .await?;

        let (drop_model, collection_model) =
            drop_model.ok_or_else(|| Error::new("drop not found"))?;

        let collection = collection_model.ok_or_else(|| Error::new("collection not found"))?;

        let edition = collection_mints::Entity::find()
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .filter(drops::Column::CollectionId.eq(collection.id))
            .count(conn)
            .await?;

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
                    .edition(CreateEditionRequest {
                        collection: collection.id,
                        recipient: input.recipient.clone(),
                        owner_address,
                        edition: edition.add(1),
                    })
                    .await?
            },
            BlockchainEnum::Polygon | BlockchainEnum::Ethereum => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        let collection_mint_active_model = collection_mints::ActiveModel {
            collection_id: Set(collection.id),
            address: Set(mint_address.to_string()),
            owner: Set(input.recipient),
            creation_status: Set(CreationStatus::Pending),
            created_by: Set(user_id),
            ..Default::default()
        };

        let collection_mint_model = collection_mint_active_model.insert(conn).await?;

        let event = NftEvents {
            event: Some(nft_events::Event::MintDrop(MintTransaction {
                transaction: Some(Transaction {
                    serialized_message,
                    signed_message_signatures,
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
            collection_mint: collection_mint_model,
        })
    }
}
#[derive(Debug, Clone, InputObject)]
pub struct MintDropInput {
    drop: Uuid,
    recipient: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct MintEditionPayload {
    collection_mint: collection_mints::Model,
}
