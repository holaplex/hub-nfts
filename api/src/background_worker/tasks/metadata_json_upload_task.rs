use hub_core::anyhow::Result;
use sea_orm::{prelude::*, Set};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{BackgroundTask, BackgroundTaskError};
use crate::{
    blockchains::{polygon::Polygon, solana::Solana, CollectionEvent, DropEvent},
    db::Connection,
    entities::{
        collection_creators, collection_mints, collections, drops, metadata_jsons, mint_creators,
        sea_orm_active_enums::Blockchain as BlockchainEnum, update_histories,
    },
    hub_uploads::{HubUploadClient, UploadResponse},
    mutations::collection::fetch_owner,
    objects::MetadataJsonInput,
    proto::{
        CreateEditionTransaction, EditionInfo, MasterEdition, MetaplexMasterEditionTransaction,
        MetaplexMetadata, MintMetaplexMetadataTransaction, NftEventKey, UpdateEdtionTransaction,
        UpdateSolanaMintPayload,
    },
};

#[async_trait::async_trait]
trait After {
    async fn after(
        &self,
        db: Connection,
        context: Context,
        upload_response: UploadResponse,
    ) -> Result<(), BackgroundTaskError>;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateDrop {
    pub drop_id: Uuid,
}

#[async_trait::async_trait]
impl After for CreateDrop {
    async fn after(
        &self,
        db: Connection,
        context: Context,
        upload_response: UploadResponse,
    ) -> Result<(), BackgroundTaskError> {
        let conn = db.get();
        let (drop, collection) = drops::Entity::find_by_id_with_collection(self.drop_id)
            .one(conn)
            .await?
            .ok_or(BackgroundTaskError::RecordNotFound)?;

        let collection = collection.ok_or(BackgroundTaskError::RecordNotFound)?;
        let supply = collection.supply;
        let seller_fee_basis_points = collection.seller_fee_basis_points;

        let metadata_json = metadata_jsons::Entity::find_by_id(collection.id)
            .one(conn)
            .await?
            .ok_or(BackgroundTaskError::RecordNotFound)?;
        let creators = collection_creators::Entity::find()
            .filter(collection_creators::Column::CollectionId.eq(collection.id))
            .all(conn)
            .await?;

        let owner_address = fetch_owner(conn, collection.project_id, collection.blockchain)
            .await
            .map_err(|_| BackgroundTaskError::NoProjectWallet)?;

        let mut metadata_json_am: metadata_jsons::ActiveModel = metadata_json.clone().into();

        metadata_json_am.uri = Set(Some(upload_response.uri));
        metadata_json_am.identifier = Set(Some(upload_response.cid));

        let metadata_json = metadata_json_am.update(conn).await?;
        let metadata_uri = metadata_json
            .uri
            .ok_or(BackgroundTaskError::NoMetadataUri)?;

        let event_key = NftEventKey {
            id: collection.id.to_string(),
            user_id: collection.created_by.to_string(),
            project_id: collection.project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                context
                    .solana
                    .event()
                    .create_drop(
                        drop.drop_type,
                        event_key,
                        MetaplexMasterEditionTransaction {
                            master_edition: Some(MasterEdition {
                                owner_address,
                                supply,
                                metadata_uri,
                                name: metadata_json.name,
                                symbol: metadata_json.symbol,
                                seller_fee_basis_points: seller_fee_basis_points.into(),
                                creators: creators.into_iter().map(Into::into).collect(),
                            }),
                        },
                    )
                    .await?;
            },
            BlockchainEnum::Polygon => {
                context
                    .polygon
                    .create_drop(drop.drop_type, event_key, CreateEditionTransaction {
                        amount: supply.ok_or(BackgroundTaskError::NoSupply)?,
                        edition_info: Some(EditionInfo {
                            creator: creators
                                .get(0)
                                .ok_or(BackgroundTaskError::NoCreator)?
                                .address
                                .clone(),
                            collection: metadata_json.name,
                            uri: metadata_uri,
                            description: metadata_json.description,
                            image_uri: metadata_json.image,
                        }),
                        fee_receiver: owner_address,
                        fee_numerator: seller_fee_basis_points.into(),
                    })
                    .await?;
            },
            BlockchainEnum::Ethereum => {
                return Err(BackgroundTaskError::BlockchainNotSupported);
            },
        };

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MintToCollection {
    pub collection_mint_id: Uuid,
}

#[async_trait::async_trait]
impl After for MintToCollection {
    async fn after(
        &self,
        db: Connection,
        context: Context,
        upload_response: UploadResponse,
    ) -> Result<(), BackgroundTaskError> {
        let conn = db.get();
        let (collection_mint, collection) =
            collection_mints::Entity::find_by_id_with_collection(self.collection_mint_id)
                .one(conn)
                .await?
                .ok_or(BackgroundTaskError::RecordNotFound)?;

        let collection = collection.ok_or(BackgroundTaskError::RecordNotFound)?;
        let seller_fee_basis_points = collection.seller_fee_basis_points;

        let metadata_json = metadata_jsons::Entity::find_by_id(collection_mint.id)
            .one(conn)
            .await?
            .ok_or(BackgroundTaskError::RecordNotFound)?;
        let creators = mint_creators::Entity::find()
            .filter(mint_creators::Column::CollectionMintId.eq(collection_mint.id))
            .all(conn)
            .await?;

        let owner_address = fetch_owner(conn, collection.project_id, collection.blockchain)
            .await
            .map_err(|_| BackgroundTaskError::NoProjectWallet)?;

        let mut metadata_json_am: metadata_jsons::ActiveModel = metadata_json.clone().into();

        metadata_json_am.uri = Set(Some(upload_response.uri));
        metadata_json_am.identifier = Set(Some(upload_response.cid));

        let metadata_json = metadata_json_am.update(conn).await?;
        let metadata_uri = metadata_json
            .uri
            .ok_or(BackgroundTaskError::NoMetadataUri)?;

        let event_key = NftEventKey {
            id: collection_mint.id.to_string(),
            user_id: collection_mint.created_by.to_string(),
            project_id: collection.project_id.to_string(),
        };

        let recipient_address = collection_mint.owner.ok_or(BackgroundTaskError::NoOwner)?;
        let compressed = collection_mint.compressed.unwrap_or_default();

        match collection.blockchain {
            BlockchainEnum::Solana => {
                context
                    .solana
                    .event()
                    .mint_to_collection(event_key, MintMetaplexMetadataTransaction {
                        metadata: Some(MetaplexMetadata {
                            owner_address,
                            metadata_uri,
                            name: metadata_json.name,
                            symbol: metadata_json.symbol,
                            seller_fee_basis_points: seller_fee_basis_points.into(),
                            creators: creators.into_iter().map(Into::into).collect(),
                        }),
                        recipient_address,
                        compressed,
                        collection_id: collection.id.to_string(),
                    })
                    .await?;
            },
            BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
                return Err(BackgroundTaskError::BlockchainNotSupported);
            },
        };

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateCollection {
    pub collection_id: Uuid,
}

#[async_trait::async_trait]
impl After for CreateCollection {
    async fn after(
        &self,
        db: Connection,
        context: Context,
        upload_response: UploadResponse,
    ) -> Result<(), BackgroundTaskError> {
        let conn = db.get();

        let collection = collections::Entity::find_by_id(self.collection_id)
            .one(conn)
            .await?
            .ok_or(BackgroundTaskError::RecordNotFound)?;

        let seller_fee_basis_points = collection.seller_fee_basis_points;

        let metadata_json = metadata_jsons::Entity::find_by_id(collection.id)
            .one(conn)
            .await?
            .ok_or(BackgroundTaskError::RecordNotFound)?;
        let creators = collection_creators::Entity::find()
            .filter(collection_creators::Column::CollectionId.eq(collection.id))
            .all(conn)
            .await?;

        let owner_address = fetch_owner(conn, collection.project_id, collection.blockchain)
            .await
            .map_err(|_| BackgroundTaskError::NoProjectWallet)?;

        let mut metadata_json_am: metadata_jsons::ActiveModel = metadata_json.clone().into();

        metadata_json_am.uri = Set(Some(upload_response.uri));
        metadata_json_am.identifier = Set(Some(upload_response.cid));

        let metadata_json = metadata_json_am.update(conn).await?;
        let metadata_uri = metadata_json
            .uri
            .ok_or(BackgroundTaskError::NoMetadataUri)?;

        let event_key = NftEventKey {
            id: collection.id.to_string(),
            user_id: collection.created_by.to_string(),
            project_id: collection.project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                context
                    .solana
                    .event()
                    .create_collection(event_key, MetaplexMasterEditionTransaction {
                        master_edition: Some(MasterEdition {
                            owner_address,
                            metadata_uri,
                            supply: Some(0),
                            name: metadata_json.name,
                            symbol: metadata_json.symbol,
                            seller_fee_basis_points: seller_fee_basis_points.into(),
                            creators: creators.into_iter().map(Into::into).collect(),
                        }),
                    })
                    .await?;
            },
            BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
                return Err(BackgroundTaskError::BlockchainNotSupported);
            },
        };

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueueMintToDrop {
    pub drop_id: Uuid,
    pub collection_mint_id: Uuid,
}

#[async_trait::async_trait]
impl After for QueueMintToDrop {
    async fn after(
        &self,
        db: Connection,
        _context: Context,
        upload_response: UploadResponse,
    ) -> Result<(), BackgroundTaskError> {
        let conn = db.get();

        let metadata_json = metadata_jsons::Entity::find_by_id(self.collection_mint_id)
            .one(conn)
            .await?
            .ok_or(BackgroundTaskError::RecordNotFound)?;

        let mut metadata_json_am: metadata_jsons::ActiveModel = metadata_json.clone().into();

        metadata_json_am.uri = Set(Some(upload_response.uri));
        metadata_json_am.identifier = Set(Some(upload_response.cid));

        metadata_json_am.update(conn).await?;

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateMint {
    pub update_history_id: Uuid,
}

#[async_trait::async_trait]
impl After for UpdateMint {
    async fn after(
        &self,
        db: Connection,
        context: Context,
        upload_response: UploadResponse,
    ) -> Result<(), BackgroundTaskError> {
        let conn = db.get();

        let update_history = update_histories::Entity::find()
            .filter(update_histories::Column::Id.eq(self.update_history_id))
            .one(conn)
            .await?
            .ok_or(BackgroundTaskError::RecordNotFound)?;

        let (collection_mint, collection) =
            collection_mints::Entity::find_by_id_with_collection(update_history.mint_id)
                .one(conn)
                .await?
                .ok_or(BackgroundTaskError::RecordNotFound)?;

        let metadata_json = metadata_jsons::Entity::find_by_id(collection_mint.id)
            .one(conn)
            .await?
            .ok_or(BackgroundTaskError::RecordNotFound)?;
        let creators = mint_creators::Entity::find()
            .filter(mint_creators::Column::CollectionMintId.eq(collection_mint.id))
            .all(conn)
            .await?;

        let collection = collection.ok_or(BackgroundTaskError::RecordNotFound)?;

        let owner_address = fetch_owner(conn, collection.project_id, collection.blockchain)
            .await
            .map_err(|_| BackgroundTaskError::NoProjectWallet)?;

        let mut metadata_json_am: metadata_jsons::ActiveModel = metadata_json.clone().into();

        metadata_json_am.uri = Set(Some(upload_response.uri));
        metadata_json_am.identifier = Set(Some(upload_response.cid));

        let metadata_json = metadata_json_am.update(conn).await?;
        let metadata_uri = metadata_json
            .uri
            .ok_or(BackgroundTaskError::NoMetadataUri)?;

        match collection.blockchain {
            BlockchainEnum::Solana => {
                context
                    .solana
                    .event()
                    .update_collection_mint(
                        NftEventKey {
                            id: update_history.id.to_string(),
                            project_id: collection.project_id.to_string(),
                            user_id: update_history.created_by.to_string(),
                        },
                        UpdateSolanaMintPayload {
                            metadata: Some(MetaplexMetadata {
                                owner_address,
                                metadata_uri,
                                name: metadata_json.name,
                                symbol: metadata_json.symbol,
                                seller_fee_basis_points: collection.seller_fee_basis_points.into(),
                                creators: creators.into_iter().map(Into::into).collect(),
                            }),
                            collection_id: collection.id.to_string(),
                            mint_id: update_history.mint_id.to_string(),
                        },
                    )
                    .await?;
            },
            BlockchainEnum::Ethereum | BlockchainEnum::Polygon => {
                return Err(BackgroundTaskError::BlockchainNotSupported);
            },
        };

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PatchCollection {
    pub collection_id: Uuid,
    pub updated_by_id: Uuid,
}

#[async_trait::async_trait]
impl After for PatchCollection {
    async fn after(
        &self,
        db: Connection,
        context: Context,
        upload_response: UploadResponse,
    ) -> Result<(), BackgroundTaskError> {
        let conn = db.get();

        let collection = collections::Entity::find_by_id(self.collection_id)
            .one(conn)
            .await?
            .ok_or(BackgroundTaskError::RecordNotFound)?;

        let seller_fee_basis_points = collection.seller_fee_basis_points;

        let metadata_json = metadata_jsons::Entity::find_by_id(collection.id)
            .one(conn)
            .await?
            .ok_or(BackgroundTaskError::RecordNotFound)?;
        let creators = collection_creators::Entity::find()
            .filter(collection_creators::Column::CollectionId.eq(collection.id))
            .all(conn)
            .await?;

        let owner_address = fetch_owner(conn, collection.project_id, collection.blockchain)
            .await
            .map_err(|_| BackgroundTaskError::NoProjectWallet)?;

        let mut metadata_json_am: metadata_jsons::ActiveModel = metadata_json.clone().into();

        metadata_json_am.uri = Set(Some(upload_response.uri));
        metadata_json_am.identifier = Set(Some(upload_response.cid));

        let metadata_json = metadata_json_am.update(conn).await?;
        let metadata_uri = metadata_json
            .uri
            .ok_or(BackgroundTaskError::NoMetadataUri)?;

        let event_key = NftEventKey {
            id: collection.id.to_string(),
            user_id: self.updated_by_id.to_string(),
            project_id: collection.project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                context
                    .solana
                    .event()
                    .update_collection(event_key, MetaplexMasterEditionTransaction {
                        master_edition: Some(MasterEdition {
                            owner_address,
                            metadata_uri,
                            supply: Some(0),
                            name: metadata_json.name,
                            symbol: metadata_json.symbol,
                            seller_fee_basis_points: seller_fee_basis_points.into(),
                            creators: creators.into_iter().map(Into::into).collect(),
                        }),
                    })
                    .await?;
            },
            BlockchainEnum::Polygon | BlockchainEnum::Ethereum => {
                return Err(BackgroundTaskError::BlockchainNotSupported);
            },
        };

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PatchDrop {
    pub drop_id: Uuid,
    pub updated_by_id: Uuid,
}

#[async_trait::async_trait]
impl After for PatchDrop {
    async fn after(
        &self,
        db: Connection,
        context: Context,
        upload_response: UploadResponse,
    ) -> Result<(), BackgroundTaskError> {
        let conn = db.get();

        let (drop, collection) = drops::Entity::find_by_id_with_collection(self.drop_id)
            .one(conn)
            .await?
            .ok_or(BackgroundTaskError::RecordNotFound)?;

        let collection = collection.ok_or(BackgroundTaskError::RecordNotFound)?;

        let seller_fee_basis_points = collection.seller_fee_basis_points;

        let metadata_json = metadata_jsons::Entity::find_by_id(collection.id)
            .one(conn)
            .await?
            .ok_or(BackgroundTaskError::RecordNotFound)?;
        let creators = collection_creators::Entity::find()
            .filter(collection_creators::Column::CollectionId.eq(collection.id))
            .all(conn)
            .await?;

        let owner_address = fetch_owner(conn, collection.project_id, collection.blockchain)
            .await
            .map_err(|_| BackgroundTaskError::NoProjectWallet)?;

        let mut metadata_json_am: metadata_jsons::ActiveModel = metadata_json.clone().into();

        metadata_json_am.uri = Set(Some(upload_response.uri));
        metadata_json_am.identifier = Set(Some(upload_response.cid));

        let metadata_json = metadata_json_am.update(conn).await?;
        let metadata_uri = metadata_json
            .uri
            .ok_or(BackgroundTaskError::NoMetadataUri)?;

        let event_key = NftEventKey {
            id: collection.id.to_string(),
            user_id: self.updated_by_id.to_string(),
            project_id: collection.project_id.to_string(),
        };

        match collection.blockchain {
            BlockchainEnum::Solana => {
                context
                    .solana
                    .event()
                    .update_drop(
                        drop.drop_type,
                        event_key,
                        MetaplexMasterEditionTransaction {
                            master_edition: Some(MasterEdition {
                                owner_address,
                                metadata_uri,
                                supply: collection.supply.map(TryInto::try_into).transpose()?,
                                name: metadata_json.name,
                                symbol: metadata_json.symbol,
                                seller_fee_basis_points: seller_fee_basis_points.into(),
                                creators: creators.into_iter().map(Into::into).collect(),
                            }),
                        },
                    )
                    .await?;
            },
            BlockchainEnum::Polygon => {
                context
                    .polygon
                    .event()
                    .update_drop(drop.drop_type, event_key, UpdateEdtionTransaction {
                        edition_info: Some(EditionInfo {
                            description: metadata_json.description,
                            image_uri: metadata_json.image,
                            collection: metadata_json.name,
                            uri: metadata_uri,
                            creator: creators
                                .get(0)
                                .ok_or(BackgroundTaskError::NoCreator)?
                                .address
                                .clone(),
                        }),
                    })
                    .await?;
            },
            BlockchainEnum::Ethereum => {
                return Err(BackgroundTaskError::BlockchainNotSupported);
            },
        };

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Caller {
    CreateDrop(CreateDrop),
    PatchDrop(PatchDrop),
    MintToCollection(MintToCollection),
    CreateCollection(CreateCollection),
    PatchCollection(PatchCollection),
    QueueMintToDrop(QueueMintToDrop),
    UpdateMint(UpdateMint),
}

#[async_trait::async_trait]
impl After for Caller {
    async fn after(
        &self,
        db: Connection,
        context: Context,
        upload_response: UploadResponse,
    ) -> Result<(), BackgroundTaskError> {
        match self {
            Self::CreateDrop(inner) => inner.after(db, context, upload_response).await,
            Self::MintToCollection(inner) => inner.after(db, context, upload_response).await,
            Self::CreateCollection(inner) => inner.after(db, context, upload_response).await,
            Self::QueueMintToDrop(inner) => inner.after(db, context, upload_response).await,
            Self::UpdateMint(inner) => inner.after(db, context, upload_response).await,
            Self::PatchCollection(inner) => inner.after(db, context, upload_response).await,
            Self::PatchDrop(inner) => inner.after(db, context, upload_response).await,
        }
    }
}

#[derive(Clone, Serialize, Debug, Deserialize)]
pub struct MetadataJsonUploadTask {
    pub metadata_json: MetadataJsonInput,
    pub caller: Caller,
}

impl MetadataJsonUploadTask {
    #[must_use]
    pub fn new(metadata_json: MetadataJsonInput, caller: Caller) -> Self {
        Self {
            metadata_json,
            caller,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Context {
    hub_uploads: HubUploadClient,
    solana: Solana,
    polygon: Polygon,
}

impl Context {
    #[must_use]
    pub fn new(hub_uploads: HubUploadClient, solana: Solana, polygon: Polygon) -> Self {
        Self {
            hub_uploads,
            solana,
            polygon,
        }
    }
}

#[async_trait::async_trait]
impl BackgroundTask<Context> for MetadataJsonUploadTask {
    const QUEUE: &'static str = "job_queue";
    const NAME: &'static str = "MetadataJsonUploadTask";

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn queue(&self) -> &'static str {
        Self::QUEUE
    }

    fn payload(&self) -> Result<Value> {
        serde_json::to_value(self).map_err(Into::into)
    }

    async fn process(&self, db: Connection, context: Context) -> Result<(), BackgroundTaskError> {
        let response = context.hub_uploads.upload(&self.metadata_json).await?;

        self.caller.after(db, context.clone(), response).await?;

        Ok(())
    }
}
