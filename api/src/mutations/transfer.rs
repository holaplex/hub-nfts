use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::credits::{CreditsClient, TransactionId};
use sea_orm::{prelude::*, Set};
use serde::{Deserialize, Serialize};

use super::collection::{validate_evm_address, validate_solana_address};
use crate::{
    blockchains::{polygon::Polygon, solana::Solana, TransferEvent},
    entities::{
        collection_mints::{self, CollectionMint},
        prelude::CustomerWallets,
        sea_orm_active_enums::{Blockchain, CreationStatus},
        transfer_charges,
    },
    proto::{self, NftEventKey, TransferPolygonAsset},
    Actions, AppContext, OrganizationId, UserID,
};

#[derive(Default)]
pub struct Mutation;

#[Object(name = "TransferAssetMutation")]
impl Mutation {
    /// Transfers an asset from one user to another on a supported blockchain network.
    /// The mutation supports transferring standard or compressed NFTs.
    /// The mutation is rejected if the wallet address is not managed by HUB.
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

        let (collection_mint_model, collection) =
            collection_mints::Entity::find_by_id_with_collection(id)
                .one(conn)
                .await?
                .ok_or(Error::new("mint not found"))?;

        if collection_mint_model.creation_status != CreationStatus::Created {
            return Err(Error::new("NFT is not minted"));
        }

        let collection = collection.ok_or(Error::new("collection not found"))?;
        input.validate_recipient_address(collection.blockchain)?;

        let owner_address = collection_mint_model.owner.clone();

        CustomerWallets::find_by_address(owner_address.clone())
            .one(conn)
            .await?
            .ok_or(Error::new("Sender wallet is not managed by HUB"))?;

        let TransactionId(credits_deduction_id) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                Actions::TransferAsset,
                collection.blockchain.into(),
                balance,
            )
            .await?;

        let transfer_charges_am = transfer_charges::ActiveModel {
            credits_deduction_id: Set(Some(credits_deduction_id)),
            ..Default::default()
        };

        let transfer_charge_model = transfer_charges_am.insert(conn).await?;
        let event_key = NftEventKey {
            id: transfer_charge_model.id.to_string(),
            user_id: user_id.to_string(),
            project_id: collection.project_id.to_string(),
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
