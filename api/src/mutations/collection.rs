use std::str::FromStr;

use async_graphql::{Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::{
    chrono::Utc,
    credits::{CreditsClient, TransactionId},
    producer::Producer,
    util::ValidateAddress,
};
use reqwest::Url;
use sea_orm::{prelude::*, ModelTrait, Set, TransactionTrait};
use serde::{Deserialize, Serialize};

use crate::{
    background_worker::{
        job_queue::JobQueue,
        tasks::{
            MetadataJsonUploadCaller, MetadataJsonUploadCreateCollection,
            MetadataJsonUploadPatchCollection, MetadataJsonUploadTask,
        },
    },
    blockchains::{solana::Solana, CollectionEvent},
    entities::{
        collection_creators, collection_mints, collections, metadata_jsons,
        prelude::{CollectionCreators, CollectionMints, Collections, Drops, MetadataJsons},
        project_wallets,
        sea_orm_active_enums::{Blockchain, Blockchain as BlockchainEnum, CreationStatus},
        switch_collection_histories,
    },
    objects::{Collection as CollectionObject, CollectionMint, Creator, MetadataJsonInput},
    proto::{
        nft_events::Event as NftEvent, CollectionCreation, CollectionImport,
        CreationStatus as NftCreationStatus, Creator as ProtoCreator, MasterEdition,
        MetaplexMasterEditionTransaction, NftEventKey, NftEvents,
    },
    Actions, AppContext,
};

#[derive(Default)]
pub struct Mutation;

#[Object(name = "CollectionMutation")]
impl Mutation {
    /// This mutation creates a new NFT collection. The collection returns immediately with a creation status of CREATING. You can [set up a webhook](https://docs.holaplex.dev/hub/For%20Developers/webhooks-overview) to receive a notification when the collection is ready to be minted.
    /// For Solana, the collection is a sized Metaplex certified collection.
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
        let nfts_producer = ctx.data::<Producer<NftEvents>>()?;
        let metadata_json_upload_job_queue = ctx.data::<JobQueue>()?;

        let owner_address = fetch_owner(conn, input.project, input.blockchain).await?;

        input.validate()?;

        if input.blockchain == BlockchainEnum::Solana {
            validate_solana_creator_verification(&owner_address, &input.creators)?;
        }

        let TransactionId(credits_deduction_id) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                Actions::CreateCollection,
                input.blockchain.into(),
                balance,
            )
            .await?;

        let tx = conn.begin().await?;

        let collection_am = collections::ActiveModel {
            blockchain: Set(input.blockchain),
            supply: Set(Some(0)),
            creation_status: Set(CreationStatus::Pending),
            project_id: Set(input.project),
            created_by: Set(user_id),
            seller_fee_basis_points: Set(0),
            credits_deduction_id: Set(Some(credits_deduction_id)),
            ..Default::default()
        };

        let collection = collection_am.insert(&tx).await?;

        for creator in input.creators {
            let am = collection_creators::ActiveModel {
                collection_id: Set(collection.id),
                address: Set(creator.address),
                verified: Set(creator.verified.unwrap_or_default()),
                share: Set(creator.share.try_into()?),
            };

            am.insert(&tx).await?;
        }

        input.metadata_json.save(collection.id, &tx).await?;

        tx.commit().await?;

        metadata_json_upload_job_queue
            .enqueue(MetadataJsonUploadTask {
                metadata_json: input.metadata_json,
                caller: MetadataJsonUploadCaller::CreateCollection(
                    MetadataJsonUploadCreateCollection {
                        collection_id: collection.id,
                    },
                ),
            })
            .await?;

        nfts_producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::CollectionCreated(CollectionCreation {
                        status: NftCreationStatus::InProgress as i32,
                    })),
                }),
                Some(&NftEventKey {
                    id: collection.id.to_string(),
                    project_id: collection.project_id.to_string(),
                    user_id: user_id.to_string(),
                }),
            )
            .await?;

        Ok(CreateCollectionPayload {
            collection: collection.into(),
        })
    }

    /// This mutation tries to re-create a failed collection.
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

        let collection = Collections::find()
            .filter(collections::Column::Id.eq(input.id))
            .one(conn)
            .await?
            .ok_or(Error::new("collection not found"))?;

        if collection.creation_status != CreationStatus::Failed {
            return Err(Error::new("only failed collections can be retried"));
        }

        let metadata_json = MetadataJsons::find_by_id(collection.id)
            .one(conn)
            .await?
            .ok_or(Error::new("metadata json not found"))?;
        let metadata_uri = metadata_json
            .uri
            .ok_or(Error::new("metadata uri not found"))?;

        let creators = CollectionCreators::find()
            .filter(collection_creators::Column::CollectionId.eq(collection.id))
            .all(conn)
            .await?;

        let owner_address = fetch_owner(conn, collection.project_id, collection.blockchain).await?;

        let TransactionId(_) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                Actions::RetryCollection,
                collection.blockchain.into(),
                balance,
            )
            .await?;

        let event_key = NftEventKey {
            id: collection.id.to_string(),
            user_id: user_id.to_string(),
            project_id: collection.project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                solana
                    .event()
                    .retry_create_collection(event_key, MetaplexMasterEditionTransaction {
                        master_edition: Some(MasterEdition {
                            owner_address,
                            name: metadata_json.name,
                            symbol: metadata_json.symbol,
                            metadata_uri,
                            seller_fee_basis_points: 0,
                            supply: Some(0),
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

        Ok(CreateCollectionPayload {
            collection: collection.into(),
        })
    }

    /// This mutation imports a Solana collection. See the [guide](https://docs.holaplex.com/hub/Guides/import-collection) for importing instructions.
    pub async fn import_solana_collection(
        &self,
        ctx: &Context<'_>,
        input: ImportCollectionInput,
    ) -> Result<ImportCollectionPayload> {
        let nfts_producer = ctx.data::<Producer<NftEvents>>()?;
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let user_id = user_id.0.ok_or(Error::new("X-USER-ID header not found"))?;

        let conn = db.get();

        validate_solana_address(&input.collection)?;

        let collection = Collections::find()
            .filter(
                collections::Column::Address
                    .eq(input.collection.clone())
                    .and(collections::Column::ProjectId.eq(input.project)),
            )
            .one(conn)
            .await?;

        if let Some(collection) = collection.clone() {
            let txn = conn.begin().await?;

            let mints = CollectionMints::find()
                .filter(collection_mints::Column::CollectionId.eq(collection.id))
                .all(&txn)
                .await?;

            let collection_json_response =
                MetadataJsons::find_by_id(collection.id).one(&txn).await?;

            if let Some(collection_json) = collection_json_response {
                collection_json.delete(&txn).await?;
            }

            let mint_ids = mints.iter().map(|m| m.id).collect::<Vec<_>>();

            let mint_jsons = MetadataJsons::find()
                .filter(metadata_jsons::Column::Id.is_in(mint_ids))
                .all(&txn)
                .await?;

            for json in mint_jsons {
                json.delete(&txn).await?;
            }

            collection.delete(&txn).await?;

            txn.commit().await?;
        }

        nfts_producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::StartedImportingSolanaCollection(
                        CollectionImport {
                            mint_address: input.collection,
                        },
                    )),
                }),
                Some(&NftEventKey {
                    id: collection.map_or(Uuid::new_v4(), |c| c.id).to_string(),
                    project_id: input.project.to_string(),
                    user_id: user_id.to_string(),
                }),
            )
            .await?;

        Ok(ImportCollectionPayload {
            status: CreationStatus::Pending,
        })
    }

    /// Update a collection attributes or creators.
    pub async fn patch_collection(
        &self,
        ctx: &Context<'_>,
        input: PatchCollectionInput,
    ) -> Result<PatchCollectionPayload> {
        let PatchCollectionInput {
            metadata_json,
            creators,
            ..
        } = input;
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;

        let solana = ctx.data::<Solana>()?;
        let metadata_json_upload_job_queue = ctx.data::<JobQueue>()?;

        let user_id = user_id.0.ok_or(Error::new("X-USER-ID header not found"))?;

        let conn = db.get();

        let collection = Collections::find()
            .filter(collections::Column::Id.eq(input.id))
            .one(conn)
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

        let metadata_json_model = metadata_jsons::Entity::find()
            .filter(metadata_jsons::Column::Id.eq(collection.id))
            .one(conn)
            .await?
            .ok_or(Error::new("metadata json not found"))?;

        let current_creators = collection_creators::Entity::find()
            .filter(collection_creators::Column::CollectionId.eq(collection.id))
            .all(conn)
            .await?;

        let tx = conn.begin().await?;

        let creators: Vec<ProtoCreator> = if let Some(creators) = creators {
            let creator_ams = creators
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

            collection_creators::Entity::delete_many()
                .filter(collection_creators::Column::CollectionId.eq(collection.id))
                .exec(&tx)
                .await?;

            if !creator_ams.is_empty() {
                collection_creators::Entity::insert_many(creator_ams)
                    .exec(&tx)
                    .await?;
            }

            creators
                .into_iter()
                .map(TryFrom::try_from)
                .collect::<Result<_>>()?
        } else {
            current_creators.into_iter().map(Into::into).collect()
        };

        if let Some(metadata_json) = metadata_json {
            metadata_json_model.delete(&tx).await?;

            metadata_json.save(collection.id, &tx).await?;

            metadata_json_upload_job_queue
                .enqueue(MetadataJsonUploadTask {
                    metadata_json,
                    caller: MetadataJsonUploadCaller::PatchCollection(
                        MetadataJsonUploadPatchCollection {
                            collection_id: collection.id,
                            updated_by_id: user_id,
                        },
                    ),
                })
                .await?;
        } else {
            let event_key = NftEventKey {
                id: collection.id.to_string(),
                user_id: user_id.to_string(),
                project_id: collection.project_id.to_string(),
            };

            let metadata_uri = metadata_json_model
                .uri
                .ok_or(Error::new("metadata uri not found"))?;

            match collection.blockchain {
                BlockchainEnum::Solana => {
                    solana
                        .event()
                        .update_collection(event_key, MetaplexMasterEditionTransaction {
                            master_edition: Some(MasterEdition {
                                owner_address,
                                supply: Some(0),
                                name: metadata_json_model.name,
                                symbol: metadata_json_model.symbol,
                                metadata_uri,
                                seller_fee_basis_points: 0,
                                creators,
                            }),
                        })
                        .await?;
                },
                BlockchainEnum::Polygon | BlockchainEnum::Ethereum => {
                    return Err(Error::new("blockchain not supported as this time"));
                },
            };
        }

        tx.commit().await?;

        Ok(PatchCollectionPayload {
            collection: collection.into(),
        })
    }

    /// This mutation allows you to change the collection to which a mint belongs.
    /// For Solana, the mint specified by `input` must already belong to a Metaplex Certified Collection.
    /// The collection you are aiming to switch to must also be Metaplex Certified Collection.

    pub async fn switch_collection(
        &self,
        ctx: &Context<'_>,
        input: SwitchCollectionInput,
    ) -> Result<SwitchCollectionPayload> {
        let SwitchCollectionInput {
            mint,
            collection_address,
        } = input;

        let AppContext {
            db,
            user_id,
            organization_id,
            balance,
            ..
        } = ctx.data::<AppContext>()?;
        let credits = ctx.data::<CreditsClient<Actions>>()?;
        let solana = ctx.data::<Solana>()?;
        let conn = db.get();

        let user_id = user_id.0.ok_or(Error::new("X-USER-ID header not found"))?;
        let org_id = organization_id
            .0
            .ok_or(Error::new("X-ORG-ID header not found"))?;
        let balance = balance.0.ok_or(Error::new("X-BALANCE header not found"))?;
        let (mint, collection) = CollectionMints::find_by_id_with_collection(mint)
            .one(conn)
            .await?
            .ok_or(Error::new("Mint not found"))?;

        let collection = collection.ok_or(Error::new("Collection not found"))?;

        let new_collection = Collections::find()
            .filter(collections::Column::Address.eq(collection_address.to_string()))
            .one(conn)
            .await?
            .ok_or(Error::new("Collection not found"))?;

        if collection.id == new_collection.id {
            return Err(Error::new("Collection already switched"));
        }

        if collection.project_id != new_collection.project_id {
            return Err(Error::new("New collection must belong to the same project"));
        }

        if new_collection
            .find_related(Drops)
            .one(conn)
            .await?
            .is_some()
        {
            return Err(Error::new("New collection must be Metaplex Certified"));
        }

        if Some(true) == mint.compressed {
            return Err(Error::new(
                "Switching collection is only supported for uncompressed mint",
            ));
        }

        let TransactionId(deduction_id) = credits
            .submit_pending_deduction(
                org_id,
                user_id,
                Actions::UpdateMint,
                collection.blockchain.into(),
                balance,
            )
            .await?;

        match collection.blockchain {
            Blockchain::Solana => {
                validate_solana_address(&collection_address)?;
                let history_am = switch_collection_histories::ActiveModel {
                    collection_mint_id: Set(mint.id),
                    collection_id: Set(new_collection.id),
                    credit_deduction_id: Set(deduction_id),
                    signature: Set(None),
                    status: Set(CreationStatus::Pending),
                    initiated_by: Set(user_id),
                    created_at: Set(Utc::now().naive_utc()),
                    ..Default::default()
                };

                let history = history_am.insert(conn).await?;

                solana
                    .event()
                    .switch_collection(
                        NftEventKey {
                            id: history.id.to_string(),
                            project_id: collection.project_id.to_string(),
                            user_id: user_id.to_string(),
                        },
                        crate::proto::SwitchCollectionPayload {
                            mint_id: mint.id.to_string(),
                            collection_id: new_collection.id.to_string(),
                        },
                    )
                    .await?;

                Ok(SwitchCollectionPayload {
                    collection_mint: mint.into(),
                })
            },
            _ => Err(Error::new("Blockchain not supported")),
        }
    }
}

///  Fetches the owner's wallet address for a given project and blockchain.
/// # Returns
/// - Returns a `Result<String>` containing the wallet address of the owner if the operation is successful.
/// # Errors
/// - Returns an error if no project wallet is found for the specified blockchain.
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
            "no project wallet found for {blockchain:?} blockchain"
        )))?
        .wallet_address;
    Ok(owner)
}

/// Result of a successful create collection mutation.
#[derive(Debug, Clone, SimpleObject)]
pub struct CreateCollectionPayload {
    collection: CollectionObject,
}

/// Input object for creating a collection.
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

/// Validates the Solana creator verification based on project treasury wallet address and the list of creators.
/// # Errors
/// - Returns an error if any of the creators are verified but their address does not match
///   the project treasury wallet address.
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

/// Validates a Solana address
/// # Errors
/// - Returns an error if the provided address is not a valid Solana address.
pub fn validate_solana_address(address: &str) -> Result<()> {
    if !ValidateAddress::is_solana_address(&address) {
        return Err(Error::new(format!(
            "{address} is not a valid Solana address"
        )));
    }

    Ok(())
}

/// Validates an EVM (Ethereum Virtual Machine) address format.
/// # Errors
/// - Returns an error  if the provided address does not match the required EVM address format.
pub fn validate_evm_address(address: &str) -> Result<()> {
    if !ValidateAddress::is_evm_address(&address) {
        return Err(Error::new(format!("{address} is not a valid EVM address")));
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

/// Input object for retrying a collection by ID.
#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct RetryCollectionInput {
    pub id: Uuid,
}

/// The patched collection.
#[derive(Debug, Clone, SimpleObject)]
pub struct RetryCollectionPayload {
    collection: CollectionObject,
}

/// Input object for patching a collection by ID.
#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct PatchCollectionInput {
    /// The unique identifier of the drop
    pub id: Uuid,
    /// The new metadata JSON for the drop
    pub metadata_json: Option<MetadataJsonInput>,
    /// The creators of the drop
    pub creators: Option<Vec<Creator>>,
}

/// Represents the result of a successful patch collection mutation.
#[derive(Debug, Clone, SimpleObject)]
pub struct PatchCollectionPayload {
    /// The collection that has been patched.
    collection: CollectionObject,
}

/// Input object for importing a collection.
#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct ImportCollectionInput {
    project: Uuid,
    // Mint address of Metaplex Certified Collection NFT
    collection: String,
}

/// Represents the result of a successful import collection mutation.
#[derive(Debug, Clone, SimpleObject)]
pub struct ImportCollectionPayload {
    /// The status of the collection import.
    status: CreationStatus,
}

/// Input object for switching a mint's collection.
#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct SwitchCollectionInput {
    mint: Uuid,
    collection_address: String,
}

/// Represents the result of a successful switch collection mutation.
#[derive(Debug, Clone, SimpleObject)]
pub struct SwitchCollectionPayload {
    collection_mint: CollectionMint,
}
