use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::producer::Producer;
use sea_orm::{prelude::*, JoinType, QuerySelect};
use serde::{Deserialize, Serialize};

use crate::{
    blockchains::{
        solana::{Solana, TransferAssetRequest},
        Edition, TransactionResponse,
    },
    entities::{
        collection_mints::{self, CollectionMint},
        collections, drops,
        prelude::{Collections, Drops},
        sea_orm_active_enums::Blockchain,
    },
    proto::{self, nft_events, NftEventKey, NftEvents, TransferMintTransaction},
    AppContext, UserID,
};

#[derive(Default)]
pub struct Mutation;

#[Object(name = "TransferAssetMutation")]
impl Mutation {
    /// Transfers an asset from one user to another on a supported blockchain network.
    ///
    /// # Arguments
    ///
    /// * `self` - A reference to the current instance of the struct.
    /// * `ctx` - A context object containing application context data.
    /// * `input` - A TransferAssetInput struct containing the input data for the asset transfer.
    ///
    /// # Returns
    ///
    /// Returns a Result containing a TransferAssetPayload struct with the updated mint information upon success.
    ///
    /// # Errors
    /// This function returns an error :
    /// If the specified blockchain is not currently supported.
    /// If the specified mint does not exist.
    ///  If there is an error while making a transfer request to the Solana blockchain.
    ///If there is an error while sending the TransferAsset event to the event producer.

    pub async fn transfer_asset(
        &self,
        ctx: &Context<'_>,
        input: TransferAssetInput,
    ) -> Result<TransferAssetPayload> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let UserID(id) = user_id;
        let user_id = id.ok_or_else(|| Error::new("X-USER-ID header not found"))?;
        let conn = db.get();
        let producer = ctx.data::<Producer<NftEvents>>()?;

        let TransferAssetInput { id, recipient } = input;

        let (collection_mint_model, collection) = collection_mints::Entity::find_by_id(id)
            .find_also_related(Collections)
            .one(conn)
            .await?
            .ok_or_else(|| Error::new("mint not found"))?;

        let collection = collection.ok_or_else(|| Error::new("collection not found"))?;

        let drop = Drops::find()
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .filter(collections::Column::Id.eq(collection.id))
            .one(conn)
            .await?
            .ok_or_else(|| Error::new("drop not found"))?;

        let proto_blockchain_enum: proto::Blockchain = collection.blockchain.into();

        let (
            _,
            TransactionResponse {
                serialized_message,
                signed_message_signatures,
            },
        ) = match collection.blockchain {
            Blockchain::Solana => {
                let solana = ctx.data::<Solana>()?;
                solana
                    .transfer(TransferAssetRequest {
                        sender: collection_mint_model.owner.clone(),
                        recipient: recipient.clone(),
                        mint_address: collection_mint_model.address.clone(),
                    })
                    .await?
            },
            Blockchain::Polygon | Blockchain::Ethereum => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        // emit `TransferAsset` event
        let event = NftEvents {
            event: Some(nft_events::Event::TransferMint(TransferMintTransaction {
                transaction: Some(proto::Transaction {
                    serialized_message,
                    signed_message_signatures,
                    blockchain: proto_blockchain_enum as i32,
                }),
                address: collection_mint_model.address.to_string(),
                sender: collection_mint_model.owner.to_string(),
                recipient: recipient.to_string(),
                project_id: drop.project_id.to_string(),
            })),
        };
        let key = NftEventKey {
            id: id.to_string(),
            user_id: user_id.to_string(),
        };

        producer.send(Some(&event), Some(&key)).await?;

        Ok(TransferAssetPayload {
            mint: collection_mint_model.into(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct TransferAssetInput {
    pub id: Uuid,
    pub recipient: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct TransferAssetPayload {
    pub mint: CollectionMint,
}