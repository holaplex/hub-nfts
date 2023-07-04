use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::credits::CreditsClient;
use sea_orm::{prelude::*, JoinType, QuerySelect, Set};
use serde::{Deserialize, Serialize};

use super::drop::{validate_evm_address, validate_solana_address};
use crate::{
    blockchains::{polygon::Polygon, solana::Solana, Event},
    db::Connection,
    entities::{
        collection_mints::{self, CollectionMint},
        collections, customer_wallets, drops, nft_transfers,
        prelude::{Collections, CustomerWallets, Drops},
        sea_orm_active_enums::{Blockchain, CreationStatus},
    },
    proto::{self, NftEventKey, TransferPolygonAsset},
    Actions, AppContext, OrganizationId, UserID,
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
        let AppContext {
            db,
            user_id,
            organization_id,
            balance,
            ..
        } = ctx.data::<AppContext>()?;
        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;

        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = org.ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-CREDIT-BALANCE header not found"))?;

        let conn = db.get();
        let credits = ctx.data::<CreditsClient<Actions>>()?;

        let TransferAssetInput { id, recipient } = input.clone();

        let (collection_mint_model, collection) = collection_mints::Entity::find_by_id(id)
            .find_also_related(Collections)
            .one(conn)
            .await?
            .ok_or(Error::new("mint not found"))?;

        if collection_mint_model.creation_status != CreationStatus::Created {
            return Err(Error::new("NFT is not minted"));
        }

        let collection = collection.ok_or(Error::new("collection not found"))?;
        input.validate_recipient_address(collection.blockchain)?;

        let drop = Drops::find()
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .filter(collections::Column::Id.eq(collection.id))
            .one(conn)
            .await?
            .ok_or(Error::new("drop not found"))?;

        let owner_address = collection_mint_model.owner.clone();

        CustomerWallets::find()
            .filter(customer_wallets::Column::Address.eq(owner_address.clone()))
            .one(conn)
            .await?
            .ok_or(Error::new("Sender wallet is not managed by Hub"))?;

        let nft_transfer_am = nft_transfers::ActiveModel {
            tx_signature: Set(None),
            collection_mint_id: Set(collection_mint_model.id),
            sender: Set(owner_address.clone()),
            recipient: Set(recipient.clone()),
            ..Default::default()
        };

        let nft_transfer_model = nft_transfer_am.insert(conn).await?;
        let event_key = NftEventKey {
            id: nft_transfer_model.id.to_string(),
            user_id: user_id.to_string(),
            project_id: drop.project_id.to_string(),
        };

        let collection_mint_id = collection_mint_model.id.to_string();
        let recipient_address = recipient.to_string();

        match collection.blockchain {
            Blockchain::Solana => {
                let solana = ctx.data::<Solana>()?;

                solana
                    .event()
                    .transfer_asset(event_key, proto::TransferMetaplexAssetTransaction {
                        recipient_address,
                        owner_address,
                        collection_mint_id,
                    })
                    .await?;
            },
            Blockchain::Polygon => {
                let polygon = ctx.data::<Polygon>()?;
                polygon
                    .event()
                    .transfer_asset(event_key, TransferPolygonAsset {
                        collection_mint_id,
                        owner_address,
                        recipient_address,
                        amount: 1,
                    })
                    .await?;
            },
            Blockchain::Ethereum => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        submit_pending_deduction(
            credits,
            db,
            balance,
            org_id,
            user_id,
            nft_transfer_model.id,
            collection.blockchain,
        )
        .await?;

        Ok(TransferAssetPayload {
            mint: collection_mint_model.into(),
        })
    }
}

async fn submit_pending_deduction(
    credits: &CreditsClient<Actions>,
    db: &Connection,
    balance: u64,
    org_id: Uuid,
    user_id: Uuid,
    transfer_id: Uuid,
    blockchain: Blockchain,
) -> Result<()> {
    let id = match blockchain {
        Blockchain::Solana | Blockchain::Polygon => {
            credits
                .submit_pending_deduction(
                    org_id,
                    user_id,
                    Actions::TransferAsset,
                    blockchain.into(),
                    balance,
                )
                .await?
        },
        Blockchain::Ethereum => {
            return Err(Error::new("blockchain not supported yet"));
        },
    };

    let deduction_id = id.ok_or(Error::new("Organization does not have enough credits"))?;

    let nft_transfer_model = nft_transfers::Entity::find_by_id(transfer_id)
        .one(db.get())
        .await?
        .ok_or(Error::new("drop not found"))?;

    let mut nft_transfer: nft_transfers::ActiveModel = nft_transfer_model.into();
    nft_transfer.credits_deduction_id = Set(Some(deduction_id.0));
    nft_transfer.update(db.get()).await?;

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct TransferAssetInput {
    pub id: Uuid,
    pub recipient: String,
}

impl TransferAssetInput {
    fn validate_recipient_address(&self, blockchain: Blockchain) -> Result<()> {
        match blockchain {
            Blockchain::Ethereum => Err(Error::new("Blockchain not supported yet")),
            Blockchain::Polygon => validate_evm_address(&self.recipient),
            Blockchain::Solana => validate_solana_address(&self.recipient),
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct TransferAssetPayload {
    pub mint: CollectionMint,
}
