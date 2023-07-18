use std::str::FromStr;

use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::{credits::CreditsClient, producer::Producer};
use reqwest::Url;
use sea_orm::{prelude::*, ModelTrait, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use solana_program::pubkey::Pubkey;

use crate::{
    blockchains::{polygon::Polygon, solana::Solana, CollectionEvent},
    collection::Collection,
    db::Connection,
    entities::{
        collection_creators, collections, metadata_jsons,
        prelude::{CollectionCreators, Collections, MetadataJsons},
        project_wallets,
        sea_orm_active_enums::{Blockchain as BlockchainEnum, CreationStatus},
    },
    metadata_json::MetadataJson,
    objects::{Collection as CollectionObject, Creator, MetadataJsonInput},
    proto::{
        nft_events::Event as NftEvent, CreationStatus as NftCreationStatus,
        Creator as ProtoCreator, DropCreation, MetaplexCertifiedCollectionTransaction,
        MetaplexMetadata, NftEventKey, NftEvents,
    },
    Actions, AppContext, NftStorageClient, OrganizationId, UserID,
};

#[derive(Default)]
pub struct Mutation;

#[Object(name = "CollectionMutation")]
impl Mutation {
    /// This mutation creates a new NFT drop and its associated collection. The drop returns immediately with a creation status of CREATING. You can [set up a webhook](https://docs.holaplex.dev/hub/For%20Developers/webhooks-overview) to receive a notification when the drop is ready to be minted.
    /// Error
    /// If the drop cannot be saved to the database or fails to be emitted for submission to the desired blockchain, the mutation will result in an error.
    pub async fn create_collection(
        &self,
        ctx: &Context<'_>,
        input: CreateCollectionInput,
    ) -> Result<CreateCollectionPayload> {
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
        let _polygon = ctx.data::<Polygon>()?;
        let nft_storage = ctx.data::<NftStorageClient>()?;
        let nfts_producer = ctx.data::<Producer<NftEvents>>()?;

        let owner_address = fetch_owner(conn, input.project, input.blockchain).await?;

        input.validate()?;

        if input.blockchain == BlockchainEnum::Solana {
            validate_solana_creator_verification(&owner_address, &input.creators)?;
        }

        let collection_am = collections::ActiveModel {
            blockchain: Set(input.blockchain),
            supply: Set(Some(0)),
            creation_status: Set(CreationStatus::Pending),
            project_id: Set(input.project),
            created_by: Set(user_id),
            ..Default::default()
        };

        let collection = Collection::new(collection_am)
            .creators(input.creators.clone())
            .save(db)
            .await?;

        let metadata_json = MetadataJson::new(input.metadata_json)
            .upload(nft_storage)
            .await?
            .save(collection.id, db)
            .await?;

        let event_key = NftEventKey {
            id: collection.id.to_string(),
            user_id: user_id.to_string(),
            project_id: input.project.to_string(),
        };

        match input.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .create_collection(event_key, MetaplexCertifiedCollectionTransaction {
                        metadata: Some(MetaplexMetadata {
                            owner_address,
                            name: metadata_json.name,
                            symbol: metadata_json.symbol,
                            metadata_uri: metadata_json.uri,
                            seller_fee_basis_points: 0,
                            creators: input
                                .creators
                                .into_iter()
                                .map(TryFrom::try_from)
                                .collect::<Result<_>>()?,
                        }),
                    })
                    .await?;
            },
            BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        submit_pending_deduction(credits, db, DeductionParams {
            user_id,
            org_id,
            balance,
            collection: collection.id,
            blockchain: input.blockchain,
            action: Actions::CreateDrop,
        })
        .await?;

        // TODO: separate event for collection creation
        nfts_producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::DropCreated(DropCreation {
                        status: NftCreationStatus::InProgress as i32,
                    })),
                }),
                Some(&NftEventKey {
                    id: collection.id.to_string(),
                    project_id: input.project.to_string(),
                    user_id: user_id.to_string(),
                }),
            )
            .await?;

        Ok(CreateCollectionPayload {
            collection: collection.into(),
        })
    }

    /// This mutation retries an existing drop.
    /// The drop returns immediately with a creation status of CREATING.
    /// You can [set up a webhook](https://docs.holaplex.dev/hub/For%20Developers/webhooks-overview) to receive a notification when the drop is ready to be minted.
    /// Errors
    /// The mutation will fail if the drop and its related collection cannot be located,
    /// if the transaction response cannot be built,
    /// or if the transaction event cannot be emitted.
    pub async fn retry_collection(
        &self,
        ctx: &Context<'_>,
        input: RetryCollectionInput,
    ) -> Result<CreateCollectionPayload> {
        let AppContext {
            db,
            user_id,
            organization_id,
            balance,
            ..
        } = ctx.data::<AppContext>()?;
        let UserID(id) = user_id;
        let OrganizationId(org) = organization_id;
        let conn = db.get();
        let solana = ctx.data::<Solana>()?;
        let _polygon = ctx.data::<Polygon>()?;
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let user_id = id.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = org.ok_or(Error::new("X-ORGANIZATION-ID header not found"))?;
        let balance = balance
            .0
            .ok_or(Error::new("X-ORGANIZATION-BALANCE header not found"))?;
        let collection = Collections::find()
            .filter(collections::Column::Id.eq(input.id))
            .one(db.get())
            .await?
            .ok_or(Error::new("collection not found"))?;

        if collection.creation_status == CreationStatus::Created {
            return Err(Error::new("collection already created"));
        }

        let metadata_json = MetadataJsons::find_by_id(collection.id)
            .one(conn)
            .await?
            .ok_or(Error::new("metadata json not found"))?;
        let creators = CollectionCreators::find()
            .filter(collection_creators::Column::CollectionId.eq(collection.id))
            .all(conn)
            .await?;

        let owner_address = fetch_owner(conn, collection.project_id, collection.blockchain).await?;

        let event_key = NftEventKey {
            id: collection.id.to_string(),
            user_id: user_id.to_string(),
            project_id: collection.project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .retry_create_collection(event_key, MetaplexCertifiedCollectionTransaction {
                        metadata: Some(MetaplexMetadata {
                            owner_address,
                            name: metadata_json.name,
                            symbol: metadata_json.symbol,
                            metadata_uri: metadata_json.uri,
                            seller_fee_basis_points: 0,
                            creators: creators
                                .into_iter()
                                .map(|c| ProtoCreator {
                                    address: c.address,
                                    verified: c.verified,
                                    share: c.share,
                                })
                                .collect(),
                        }),
                    })
                    .await?;
            },
            BlockchainEnum::Polygon | BlockchainEnum::Ethereum => {
                return Err(Error::new("blockchain not supported as this time"));
            },
        };

        submit_pending_deduction(credits, db, DeductionParams {
            balance,
            user_id,
            org_id,
            collection: collection.id,
            blockchain: collection.blockchain,
            action: Actions::RetryDrop,
        })
        .await?;

        Ok(CreateCollectionPayload {
            collection: collection.into(),
        })
    }

    /// This mutation allows updating a drop and it's associated collection by ID.
    /// It returns an error if it fails to reach the database, emit update events or assemble the on-chain transaction.
    /// Returns the `PatchDropPayload` object on success.
    pub async fn patch_collection(
        &self,
        ctx: &Context<'_>,
        input: PatchCollectionInput,
    ) -> Result<PatchCollectionPayload> {
        let PatchCollectionInput {
            id: _,
            metadata_json,
            creators,
        } = input;

        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let conn = db.get();
        let nft_storage = ctx.data::<NftStorageClient>()?;
        let solana = ctx.data::<Solana>()?;
        let _polygon = ctx.data::<Polygon>()?;

        let user_id = user_id.0.ok_or(Error::new("X-USER-ID header not found"))?;

        let collection = Collections::find()
            .filter(collections::Column::Id.eq(input.id))
            .one(db.get())
            .await?
            .ok_or(Error::new("collection not found"))?;

        let owner_address = fetch_owner(conn, collection.project_id, collection.blockchain).await?;

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

        if let Some(creators) = creators.clone() {
            let creators = creators
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

            conn.transaction::<_, (), DbErr>(|txn| {
                Box::pin(async move {
                    collection_creators::Entity::delete_many()
                        .filter(collection_creators::Column::CollectionId.eq(collection.id))
                        .exec(txn)
                        .await?;

                    collection_creators::Entity::insert_many(creators)
                        .exec(txn)
                        .await?;

                    Ok(())
                })
            })
            .await?;
        }

        let metadata_json_model = metadata_jsons::Entity::find()
            .filter(metadata_jsons::Column::Id.eq(collection.id))
            .one(conn)
            .await?
            .ok_or(Error::new("metadata json not found"))?;

        let metadata_json_model = if let Some(metadata_json) = metadata_json {
            metadata_json_model.clone().delete(conn).await?;

            MetadataJson::new(metadata_json.clone())
                .upload(nft_storage)
                .await?
                .save(collection.id, db)
                .await?
        } else {
            metadata_json_model
        };

        let event_key = NftEventKey {
            id: collection.id.to_string(),
            user_id: user_id.to_string(),
            project_id: collection.project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                let creators = if let Some(creators) = creators.clone() {
                    creators
                        .into_iter()
                        .map(TryFrom::try_from)
                        .collect::<Result<_>>()?
                } else {
                    current_creators.into_iter().map(Into::into).collect()
                };

                solana
                    .event()
                    .update_collection(event_key, MetaplexCertifiedCollectionTransaction {
                        metadata: Some(MetaplexMetadata {
                            owner_address,
                            name: metadata_json_model.name,
                            symbol: metadata_json_model.symbol,
                            metadata_uri: metadata_json_model.uri,
                            seller_fee_basis_points: 0,
                            creators,
                        }),
                    })
                    .await?;
            },
            BlockchainEnum::Polygon | BlockchainEnum::Ethereum => {
                return Err(Error::new("blockchain not supported yet"));
            },
        };

        Ok(PatchCollectionPayload {
            collection: collection.into(),
        })
    }
}

struct DeductionParams {
    balance: u64,
    user_id: Uuid,
    org_id: Uuid,
    collection: Uuid,
    blockchain: BlockchainEnum,
    action: Actions,
}

async fn submit_pending_deduction(
    credits: &CreditsClient<Actions>,
    db: &Connection,
    params: DeductionParams,
) -> Result<()> {
    let DeductionParams {
        balance,
        user_id,
        org_id,
        collection,
        blockchain,
        action,
    } = params;

    let collection_model = Collections::find()
        .filter(collections::Column::Id.eq(collection))
        .one(db.get())
        .await?
        .ok_or(Error::new("drop not found"))?;

    if collection_model.credits_deduction_id.is_some() {
        return Ok(());
    }

    let id = match blockchain {
        BlockchainEnum::Solana => {
            credits
                .submit_pending_deduction(org_id, user_id, action, blockchain.into(), balance)
                .await?
        },
        BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
            return Err(Error::new("blockchain not supported yet"));
        },
    };

    let deduction_id = id.ok_or(Error::new("Organization does not have enough credits"))?;

    let mut collection_am: collections::ActiveModel = collection_model.into();
    collection_am.credits_deduction_id = Set(Some(deduction_id.0));
    collection_am.update(db.get()).await?;

    Ok(())
}

pub async fn fetch_owner(
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
            "no project wallet found for {blockchain} blockchain"
        )))?
        .wallet_address;
    Ok(owner)
}

#[derive(Debug, Clone, SimpleObject)]
pub struct CreateCollectionPayload {
    collection: CollectionObject,
}

#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct CreateCollectionInput {
    pub project: Uuid,
    pub blockchain: BlockchainEnum,
    pub creators: Vec<Creator>,
    pub metadata_json: MetadataJsonInput,
}

impl CreateCollectionInput {
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
        validate_creators(self.blockchain, &self.creators)?;
        validate_json(self.blockchain, &self.metadata_json)?;

        Ok(())
    }
}

pub fn validate_solana_creator_verification(
    project_treasury_wallet_address: &str,
    creators: &Vec<Creator>,
) -> Result<()> {
    for creator in creators {
        if creator.verified.unwrap_or_default()
            && creator.address != project_treasury_wallet_address
        {
            return Err(Error::new(format!(
                "Only the project treasury wallet of {project_treasury_wallet_address} can be verified in the mutation. Other creators must be verified independently. See the Metaplex documentation for more details."
            )));
        }
    }

    Ok(())
}

/// Validates the addresses of the creators for a given blockchain.
/// # Returns
/// - Ok(()) if all creator addresses are valid blockchain addresses.
///
/// # Errors
/// - Err with an appropriate error message if any creator address is not a valid address.
/// - Err if the blockchain is not supported.
pub fn validate_creators(blockchain: BlockchainEnum, creators: &Vec<Creator>) -> Result<()> {
    let royalty_share = creators.iter().map(|c| c.share).sum::<u8>();

    if royalty_share != 100 {
        return Err(Error::new(
            "The sum of all creator shares must be equal to 100",
        ));
    }

    match blockchain {
        BlockchainEnum::Solana => {
            if creators.len() > 5 {
                return Err(Error::new(
                    "Maximum number of creators is 5 for Solana Blockchain",
                ));
            }

            for creator in creators {
                validate_solana_address(&creator.address)?;
            }
        },
        BlockchainEnum::Polygon => {
            if creators.len() != 1 {
                return Err(Error::new(
                    "Only one creator is allowed for Polygon Blockchain",
                ));
            }

            let address = &creators[0].clone().address;
            validate_evm_address(address)?;
        },
        BlockchainEnum::Ethereum => return Err(Error::new("Blockchain not supported yet")),
    }

    Ok(())
}

pub fn validate_solana_address(address: &str) -> Result<()> {
    if Pubkey::from_str(address).is_err() {
        return Err(Error::new(format!(
            "{address} is not a valid Solana address"
        )));
    }

    Ok(())
}

pub fn validate_evm_address(address: &str) -> Result<()> {
    let err = Err(Error::new(format!("{address} is not a valid EVM address")));

    // Ethereum address must start with '0x'
    if !address.starts_with("0x") {
        return err;
    }

    // Ethereum address must be exactly 40 characters long after removing '0x'
    if address.len() != 42 {
        return err;
    }

    // Check that the address contains only hexadecimal characters
    if !address[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        return err;
    }

    Ok(())
}

/// Validates the JSON metadata input for the NFT drop.
/// # Returns
/// - Ok(()) if all JSON fields are valid.
///
/// # Errors
/// - Err with an appropriate error message if any JSON field is invalid.
pub fn validate_json(blockchain: BlockchainEnum, json: &MetadataJsonInput) -> Result<()> {
    json.animation_url
        .as_ref()
        .map(|animation_url| Url::from_str(animation_url))
        .transpose()
        .map_err(|_| Error::new("Invalid animation url"))?;

    json.external_url
        .as_ref()
        .map(|external_url| Url::from_str(external_url))
        .transpose()
        .map_err(|_| Error::new("Invalid external url"))?;

    Url::from_str(&json.image).map_err(|_| Error::new("Invalid image url"))?;

    if blockchain != BlockchainEnum::Solana {
        return Ok(());
    }

    if json.name.chars().count() > 32 {
        return Err(Error::new("Name must be less than 32 characters"));
    }

    if json.symbol.chars().count() > 10 {
        return Err(Error::new("Symbol must be less than 10 characters"));
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct RetryCollectionInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct RetryCollectionPayload {
    collection: CollectionObject,
}

/// Input object for patching a drop and associated collection by ID
#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct PatchCollectionInput {
    /// The unique identifier of the drop
    pub id: Uuid,
    /// The new metadata JSON for the drop
    pub metadata_json: Option<MetadataJsonInput>,
    /// The creators of the drop
    pub creators: Option<Vec<Creator>>,
}

/// Represents the result of a successful patch drop mutation.
#[derive(Debug, Clone, SimpleObject)]
pub struct PatchCollectionPayload {
    /// The drop that has been patched.
    collection: CollectionObject,
}
