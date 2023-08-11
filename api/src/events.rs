use hub_core::{
    chrono::{DateTime, NaiveDateTime, Utc},
    credits::{CreditsClient, TransactionId},
    prelude::*,
    producer::Producer,
    uuid::Uuid,
};
use sea_orm::{
    sea_query::{Expr, SimpleExpr},
    ActiveModelTrait, ColumnTrait, EntityTrait, JoinType, QueryFilter, QuerySelect, RelationTrait,
    Set, TransactionTrait,
};

use crate::{
    db::Connection,
    entities::{
        collection_creators, collection_mints, collections, customer_wallets, drops,
        metadata_json_attributes, metadata_json_files, metadata_jsons, mint_creators,
        mint_histories, nft_transfers,
        prelude::{CollectionMints, Collections, Drops, MintHistory, UpdateHistories},
        project_wallets,
        sea_orm_active_enums::{Blockchain, CreationStatus},
        transfer_charges, update_histories,
    },
    proto::{
        nft_events::Event as NftEvent,
        polygon_nft_events::Event as PolygonNftEvents,
        solana_nft_events::Event as SolanaNftsEvent,
        treasury_events::{
            Blockchain as ProtoBlockchainEnum, CustomerWallet, Event as TreasuryEvent,
            PolygonTransactionResult, ProjectWallet, TransactionStatus,
        },
        Attribute, CreationStatus as NftCreationStatus, DropCreation, File, Metadata,
        MintCollectionCreation, MintCreation, MintOwnershipUpdate, MintedTokensOwnershipUpdate,
        NftEventKey, NftEvents, SolanaCollectionPayload, SolanaCompletedMintTransaction,
        SolanaCompletedTransferTransaction, SolanaMintPayload, SolanaNftEventKey,
        SolanaUpdatedMintPayload, TreasuryEventKey,
    },
    Actions, Services,
};

#[derive(Clone)]
pub struct Processor {
    pub db: Connection,
    pub credits: CreditsClient<Actions>,
    pub producer: Producer<NftEvents>,
}

#[derive(Clone)]
struct MintTransaction {
    signature: String,
    address: String,
}

#[derive(Clone)]
enum MintResult {
    Success(MintTransaction),
    Failure,
}

#[derive(Clone)]
enum TransferResult {
    Success(String),
    Failure,
}

#[derive(Clone)]
enum UpdateResult {
    Success(String),
    Failure,
}

impl Processor {
    #[must_use]
    pub fn new(
        db: Connection,
        credits: CreditsClient<Actions>,
        producer: Producer<NftEvents>,
    ) -> Self {
        Self {
            db,
            credits,
            producer,
        }
    }

    pub async fn process(&self, msg: Services) -> Result<()> {
        match msg {
            Services::Treasury(TreasuryEventKey { id, .. }, e) => match e.event {
                Some(TreasuryEvent::ProjectWalletCreated(payload)) => {
                    self.project_wallet_created(payload).await
                },
                Some(TreasuryEvent::CustomerWalletCreated(payload)) => {
                    self.customer_wallet_created(payload).await
                },
                Some(
                    TreasuryEvent::PolygonCreateDropTxnSubmitted(payload)
                    | TreasuryEvent::PolygonRetryCreateDropSubmitted(payload),
                ) => self.drop_created(id, payload.into()).await,
                Some(
                    TreasuryEvent::PolygonMintDropSubmitted(payload)
                    | TreasuryEvent::PolygonRetryMintDropSubmitted(payload),
                ) => self.drop_minted(id, payload.into()).await,
                Some(TreasuryEvent::PolygonTransferAssetSubmitted(payload)) => {
                    self.mint_transferred(id, payload.into()).await
                },
                None | Some(_) => Ok(()),
            },
            Services::Solana(
                SolanaNftEventKey {
                    id,
                    project_id,
                    user_id,
                },
                e,
            ) => match e.event {
                Some(
                    SolanaNftsEvent::CreateDropSubmitted(payload)
                    | SolanaNftsEvent::RetryCreateDropSubmitted(payload),
                ) => {
                    self.drop_created(id, MintResult::Success(payload.into()))
                        .await
                },
                Some(
                    SolanaNftsEvent::CreateCollectionSubmitted(payload)
                    | SolanaNftsEvent::RetryCreateCollectionSubmitted(payload),
                ) => {
                    self.collection_created(id, MintResult::Success(payload.into()))
                        .await
                },
                Some(
                    SolanaNftsEvent::MintDropSubmitted(payload)
                    | SolanaNftsEvent::RetryMintDropSubmitted(payload),
                ) => {
                    self.drop_minted(id, MintResult::Success(payload.into()))
                        .await
                },
                Some(
                    SolanaNftsEvent::MintToCollectionSubmitted(payload)
                    | SolanaNftsEvent::RetryMintToCollectionSubmitted(payload),
                ) => {
                    self.minted_to_collection(id, MintResult::Success(payload.into()))
                        .await
                },
                Some(
                    SolanaNftsEvent::UpdateCollectionMintSubmitted(payload)
                    | SolanaNftsEvent::RetryUpdateMintSubmitted(payload),
                ) => {
                    self.mint_updated(id, project_id, UpdateResult::Success(payload.signature))
                        .await
                },
                Some(SolanaNftsEvent::TransferAssetSubmitted(
                    SolanaCompletedTransferTransaction { signature },
                )) => {
                    self.mint_transferred(id, TransferResult::Success(signature))
                        .await
                },
                Some(
                    SolanaNftsEvent::CreateDropFailed(_)
                    | SolanaNftsEvent::RetryCreateDropFailed(_),
                ) => self.drop_created(id, MintResult::Failure).await,
                Some(
                    SolanaNftsEvent::CreateCollectionFailed(_)
                    | SolanaNftsEvent::RetryCreateCollectionFailed(_),
                ) => self.collection_created(id, MintResult::Failure).await,
                Some(SolanaNftsEvent::MintDropFailed(_)) => {
                    self.drop_minted(id, MintResult::Failure).await
                },
                Some(
                    SolanaNftsEvent::MintToCollectionFailed(_)
                    | SolanaNftsEvent::RetryMintToCollectionFailed(_),
                ) => self.minted_to_collection(id, MintResult::Failure).await,
                Some(SolanaNftsEvent::TransferAssetFailed(_)) => {
                    self.mint_transferred(id, TransferResult::Failure).await
                },
                Some(SolanaNftsEvent::RetryMintDropFailed(_)) => {
                    self.drop_minted(id, MintResult::Failure).await
                },
                Some(
                    SolanaNftsEvent::UpdateCollectionMintFailed(_)
                    | SolanaNftsEvent::RetryUpdateMintFailed(_),
                ) => {
                    self.mint_updated(id, project_id, UpdateResult::Failure)
                        .await
                },
                Some(SolanaNftsEvent::UpdateMintOwner(e)) => self.update_mint_owner(id, e).await,
                Some(SolanaNftsEvent::ImportedExternalCollection(e)) => {
                    self.index_collection(id, project_id, user_id, e).await
                },
                Some(SolanaNftsEvent::ImportedExternalMint(e)) => {
                    self.index_mint(id, user_id, e).await
                },

                None | Some(_) => Ok(()),
            },
            Services::Polygon(_, e) => match e.event {
                Some(PolygonNftEvents::UpdateMintsOwner(p)) => {
                    self.update_polygon_mints_owner(p).await
                },
                None | Some(_) => Ok(()),
            },
        }
    }

    async fn index_collection(
        &self,
        id: String,
        project_id: String,
        created_by: String,
        payload: SolanaCollectionPayload,
    ) -> Result<()> {
        let SolanaCollectionPayload {
            supply,
            mint_address,
            seller_fee_basis_points,
            creators,
            metadata,
            files,
            ..
        } = payload;

        let metadata = metadata.context("no collection metadata found")?;

        let Metadata {
            name,
            description,
            symbol,
            attributes,
            uri,
            image,
        } = metadata;

        let collection_am = collections::ActiveModel {
            id: Set(id.parse()?),
            blockchain: Set(Blockchain::Solana),
            supply: Set(supply.map(Into::into)),
            project_id: Set(project_id.parse()?),
            credits_deduction_id: Set(None),
            creation_status: Set(CreationStatus::Created),
            total_mints: Set(0),
            address: Set(Some(mint_address)),
            signature: Set(None),
            seller_fee_basis_points: Set(seller_fee_basis_points.try_into()?),
            created_by: Set(created_by.parse()?),
            created_at: Set(Utc::now().into()),
        };

        collection_am.insert(self.db.get()).await?;

        let metadata_json = metadata_jsons::ActiveModel {
            id: Set(id.parse()?),
            name: Set(name),
            uri: Set(uri),
            symbol: Set(symbol),
            description: Set(description.unwrap_or_default()),
            image: Set(image),
            animation_url: Set(None),
            external_url: Set(None),
            identifier: Set(String::new()),
        };

        let json_model = metadata_json.insert(self.db.get()).await?;
        for creator in creators {
            let collection_creator = collection_creators::ActiveModel {
                collection_id: Set(id.parse()?),
                address: Set(creator.address),
                verified: Set(creator.verified),
                share: Set(creator.share.try_into()?),
            };
            collection_creator.insert(self.db.get()).await?;
        }
        index_attributes(&self.db, json_model.id, attributes).await?;
        index_files(&self.db, json_model.id, files).await?;

        Ok(())
    }

    async fn index_mint(
        &self,
        id: String,
        created_by: String,
        payload: SolanaMintPayload,
    ) -> Result<()> {
        let SolanaMintPayload {
            collection_id,
            mint_address,
            owner,
            seller_fee_basis_points,
            compressed,
            creators,
            files,
            metadata,
            ..
        } = payload;

        let metadata = metadata.context("no collection metadata found")?;

        let Metadata {
            name,
            description,
            symbol,
            attributes,
            uri,
            image,
        } = metadata;

        let mint_am = collection_mints::ActiveModel {
            id: Set(id.parse()?),
            collection_id: Set(collection_id.parse()?),
            address: Set(Some(mint_address)),
            owner: Set(owner),
            creation_status: Set(CreationStatus::Created),
            created_by: Set(created_by.parse()?),
            created_at: Set(Utc::now().into()),
            signature: Set(None),
            edition: Set(-1),
            seller_fee_basis_points: Set(seller_fee_basis_points.try_into()?),
            credits_deduction_id: Set(None),
            compressed: Set(compressed),
        };

        let mint_model = mint_am.insert(self.db.get()).await?;

        let metadata_json = metadata_jsons::ActiveModel {
            id: Set(id.parse()?),
            name: Set(name),
            uri: Set(uri),
            symbol: Set(symbol),
            description: Set(description.unwrap_or_default()),
            image: Set(image),
            animation_url: Set(None),
            external_url: Set(None),
            identifier: Set(String::new()),
        };

        let json_model = metadata_json.insert(self.db.get()).await?;

        for creator in creators {
            let mint_creator_am = mint_creators::ActiveModel {
                collection_mint_id: Set(mint_model.id),
                address: Set(creator.address),
                verified: Set(creator.verified),
                share: Set(creator.share.try_into()?),
            };
            mint_creator_am.insert(self.db.get()).await?;
        }
        index_attributes(&self.db, json_model.id, attributes).await?;
        index_files(&self.db, json_model.id, files).await?;

        let collection_id = Uuid::from_str(&collection_id)?;

        collections::Entity::update_many()
            .col_expr(
                collections::Column::TotalMints,
                <Expr as Into<SimpleExpr>>::into(Expr::col(collections::Column::TotalMints))
                    .add(SimpleExpr::Value(1.into())),
            )
            .filter(collections::Column::Id.eq(collection_id))
            .exec(self.db.get())
            .await?;

        Ok(())
    }

    async fn update_mint_owner(&self, id: String, payload: MintOwnershipUpdate) -> Result<()> {
        let id = Uuid::from_str(&id)?;
        let db = self.db.get();

        let mint = CollectionMints::find_by_id(id)
            .one(db)
            .await
            .context("failed to load mint from db")?
            .context("mint not found in db")?;

        let mut mint_am: collection_mints::ActiveModel = mint.into();
        mint_am.owner = Set(payload.recipient.clone());

        mint_am.update(self.db.get()).await?;

        let nft_transfer = nft_transfers::ActiveModel {
            tx_signature: Set(Some(payload.tx_signature)),
            collection_mint_id: Set(id),
            sender: Set(payload.sender),
            recipient: Set(payload.recipient),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        };

        nft_transfer.insert(db).await?;
        Ok(())
    }
    async fn update_polygon_mints_owner(&self, payload: MintedTokensOwnershipUpdate) -> Result<()> {
        let MintedTokensOwnershipUpdate {
            mint_ids,
            new_owner,
            timestamp,
            transaction_hash,
        } = payload;

        let ts = timestamp.context("No timestamp found")?;
        let created_at = DateTime::<Utc>::from_utc(
            NaiveDateTime::from_timestamp_opt(ts.seconds, ts.nanos.try_into()?)
                .context("failed to parse to NaiveDateTime")?,
            Utc,
        )
        .into();

        let db = self.db.get();
        let txn = db.begin().await?;

        let mint_ids = mint_ids
            .into_iter()
            .map(|s| Uuid::from_str(&s))
            .collect::<Result<Vec<Uuid>, _>>()?;

        let mints = CollectionMints::find()
            .filter(collection_mints::Column::Id.is_in(mint_ids))
            .all(db)
            .await?;

        for mint in mints {
            let mut mint_am: collection_mints::ActiveModel = mint.clone().into();
            mint_am.owner = Set(new_owner.clone());
            mint_am.update(&txn).await?;

            let nft_transfers = nft_transfers::ActiveModel {
                tx_signature: Set(Some(transaction_hash.clone())),
                collection_mint_id: Set(mint.id),
                sender: Set(mint.owner),
                recipient: Set(new_owner.clone()),
                created_at: Set(created_at),
                ..Default::default()
            };

            nft_transfers.insert(&txn).await?;
        }

        txn.commit().await?;

        Ok(())
    }

    async fn project_wallet_created(&self, payload: ProjectWallet) -> Result<()> {
        let conn = self.db.get();
        let project_id = Uuid::from_str(&payload.project_id)?;

        let blockchain = ProtoBlockchainEnum::from_i32(payload.blockchain)
            .context("failed to get blockchain enum variant")?;

        let active_model = project_wallets::ActiveModel {
            project_id: Set(project_id),
            wallet_address: Set(payload.wallet_address),
            blockchain: Set(blockchain.try_into()?),
            ..Default::default()
        };

        active_model
            .insert(conn)
            .await
            .context("failed to insert project wallet")?;

        Ok(())
    }

    async fn customer_wallet_created(&self, payload: CustomerWallet) -> Result<()> {
        let conn = self.db.get();

        let blockchain = ProtoBlockchainEnum::from_i32(payload.blockchain)
            .context("failed to get blockchain enum variant")?;

        let active_model = customer_wallets::ActiveModel {
            customer_id: Set(payload.customer_id.parse()?),
            address: Set(payload.wallet_address),
            blockchain: Set(blockchain.try_into()?),
            ..Default::default()
        };

        active_model
            .insert(conn)
            .await
            .context("failed to insert customer wallet")?;

        Ok(())
    }

    async fn drop_created(&self, id: String, payload: MintResult) -> Result<()> {
        let conn = self.db.get();
        let collection_id = Uuid::from_str(&id)?;

        let (collection_model, drop) = Collections::find_by_id(collection_id)
            .join(JoinType::InnerJoin, collections::Relation::Drop.def())
            .select_also(drops::Entity)
            .one(conn)
            .await
            .context("failed to load collection from db")?
            .context("collection not found in db")?;
        let drop_model = drop.context("failed to get drop from db")?;

        let mut drops_active_model: drops::ActiveModel = drop_model.clone().into();
        let mut collection_active_model: collections::ActiveModel = collection_model.into();
        let mut creation_status = NftCreationStatus::Completed;

        if let MintResult::Success(MintTransaction { signature, address }) = payload {
            collection_active_model.signature = Set(Some(signature));
            collection_active_model.address = Set(Some(address));
            collection_active_model.creation_status = Set(CreationStatus::Created);
            drops_active_model.creation_status = Set(CreationStatus::Created);

            let deduction_id = drop_model
                .credits_deduction_id
                .context("drop has no deduction id")?;
            self.credits
                .confirm_deduction(TransactionId(deduction_id))
                .await?;
        } else {
            collection_active_model.creation_status = Set(CreationStatus::Failed);
            drops_active_model.creation_status = Set(CreationStatus::Failed);
            creation_status = NftCreationStatus::Failed;
        }

        self.producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::DropCreated(DropCreation {
                        status: creation_status as i32,
                    })),
                }),
                Some(&NftEventKey {
                    id: drop_model.id.to_string(),
                    project_id: drop_model.project_id.to_string(),
                    user_id: drop_model.created_by.to_string(),
                }),
            )
            .await?;

        collection_active_model.update(conn).await?;
        drops_active_model.update(conn).await?;

        Ok(())
    }

    async fn collection_created(&self, id: String, payload: MintResult) -> Result<()> {
        let conn = self.db.get();
        let collection_id = Uuid::from_str(&id)?;

        let collection_model = collections::Entity::find_by_id(collection_id)
            .one(conn)
            .await
            .context("failed to load collection from db")?
            .context("collection not found in db")?;

        let mut collection_active_model: collections::ActiveModel = collection_model.clone().into();
        let mut creation_status = NftCreationStatus::Completed;

        if let MintResult::Success(MintTransaction { signature, address }) = payload {
            collection_active_model.signature = Set(Some(signature));
            collection_active_model.address = Set(Some(address));
            collection_active_model.creation_status = Set(CreationStatus::Created);

            let deduction_id = collection_model
                .credits_deduction_id
                .context("drop has no deduction id")?;
            self.credits
                .confirm_deduction(TransactionId(deduction_id))
                .await?;
        } else {
            collection_active_model.creation_status = Set(CreationStatus::Failed);
            creation_status = NftCreationStatus::Failed;
        }

        self.producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::DropCreated(DropCreation {
                        status: creation_status as i32,
                    })),
                }),
                Some(&NftEventKey {
                    id: collection_model.id.to_string(),
                    project_id: collection_model.project_id.to_string(),
                    user_id: collection_model.created_by.to_string(),
                }),
            )
            .await?;

        collection_active_model.update(conn).await?;

        Ok(())
    }

    async fn drop_minted(&self, id: String, payload: MintResult) -> Result<()> {
        let conn = self.db.get();
        let collection_mint_id = Uuid::from_str(&id)?;

        let (collection_mint, collection) =
            collection_mints::Entity::find_by_id(collection_mint_id)
                .find_also_related(collections::Entity)
                .one(conn)
                .await
                .context("failed to load collection mint from db")?
                .context("collection mint not found in db")?;

        let mint_history = MintHistory::find()
            .filter(mint_histories::Column::MintId.eq(collection_mint_id))
            .one(conn)
            .await
            .context("failed to load mint_history from db")?
            .context("mint_history not found in db")?;

        let collection = collection.context("collection not found")?;

        let drop = Drops::find()
            .filter(drops::Column::CollectionId.eq(collection.id))
            .one(conn)
            .await
            .context("failed to load drop from db")?
            .context("drop not found in db")?;

        let mut collection_mint_active_model: collection_mints::ActiveModel =
            collection_mint.clone().into();
        let mut mint_history_am: mint_histories::ActiveModel = mint_history.into();
        let mut creation_status = NftCreationStatus::Completed;

        if let MintResult::Success(MintTransaction { signature, address }) = payload {
            mint_history_am.status = Set(CreationStatus::Created);
            mint_history_am.tx_signature = Set(Some(signature.clone()));
            collection_mint_active_model.creation_status = Set(CreationStatus::Created);
            collection_mint_active_model.signature = Set(Some(signature));
            collection_mint_active_model.address = Set(Some(address));

            let deduction_id = collection_mint
                .credits_deduction_id
                .context("deduction id not found")?;

            self.credits
                .confirm_deduction(TransactionId(deduction_id))
                .await?;
        } else {
            mint_history_am.status = Set(CreationStatus::Failed);
            collection_mint_active_model.creation_status = Set(CreationStatus::Failed);
            creation_status = NftCreationStatus::Failed;
        }

        self.producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::DropMinted(MintCreation {
                        drop_id: drop.id.to_string(),
                        status: creation_status as i32,
                    })),
                }),
                Some(&NftEventKey {
                    id: collection_mint.id.to_string(),
                    project_id: drop.project_id.to_string(),
                    user_id: collection_mint.created_by.to_string(),
                }),
            )
            .await?;

        collection_mint_active_model.update(conn).await?;
        mint_history_am.update(conn).await?;

        Ok(())
    }

    async fn minted_to_collection(&self, id: String, payload: MintResult) -> Result<()> {
        let conn = self.db.get();
        let collection_mint_id = Uuid::from_str(&id)?;

        let (collection_mint, collection) =
            collection_mints::Entity::find_by_id_with_collection(collection_mint_id)
                .one(conn)
                .await
                .context("failed to load collection mint from db")?
                .context("collection mint not found in db")?;

        let mint_history = MintHistory::find()
            .filter(mint_histories::Column::MintId.eq(collection_mint_id))
            .one(conn)
            .await
            .context("failed to load mint_history from db")?
            .context("mint_history not found in db")?;

        let collection = collection.context("collection not found")?;

        let mut collection_mint_active_model: collection_mints::ActiveModel =
            collection_mint.clone().into();
        let mut mint_history_am: mint_histories::ActiveModel = mint_history.into();
        let mut creation_status = NftCreationStatus::Completed;

        if let MintResult::Success(MintTransaction { signature, address }) = payload {
            mint_history_am.status = Set(CreationStatus::Created);
            mint_history_am.tx_signature = Set(Some(signature.clone()));
            collection_mint_active_model.creation_status = Set(CreationStatus::Created);
            collection_mint_active_model.signature = Set(Some(signature));
            collection_mint_active_model.address = Set(Some(address));

            let deduction_id = collection_mint
                .credits_deduction_id
                .context("deduction id not found")?;

            self.credits
                .confirm_deduction(TransactionId(deduction_id))
                .await?;
        } else {
            mint_history_am.status = Set(CreationStatus::Failed);
            collection_mint_active_model.creation_status = Set(CreationStatus::Failed);
            creation_status = NftCreationStatus::Failed;
        }

        self.producer
            .send(
                Some(&NftEvents {
                    event: Some(NftEvent::MintedToCollection(MintCollectionCreation {
                        collection_id: collection.id.to_string(),
                        status: creation_status as i32,
                    })),
                }),
                Some(&NftEventKey {
                    id: collection_mint.id.to_string(),
                    project_id: collection.project_id.to_string(),
                    user_id: collection_mint.created_by.to_string(),
                }),
            )
            .await?;

        collection_mint_active_model.update(conn).await?;
        mint_history_am.update(conn).await?;

        Ok(())
    }

    async fn mint_transferred(&self, id: String, payload: TransferResult) -> Result<()> {
        let conn = self.db.get();
        let transfer_id = Uuid::from_str(&id)?;

        let transfer_charge = transfer_charges::Entity::find()
            .filter(transfer_charges::Column::Id.eq(transfer_id))
            .one(conn)
            .await?
            .context("failed to load transfer charge from db")?;

        if let TransferResult::Success(_) = payload {
            let deduction_id = transfer_charge
                .credits_deduction_id
                .context("deduction id not found")?;

            self.credits
                .confirm_deduction(TransactionId(deduction_id))
                .await?;
        }

        Ok(())
    }

    async fn mint_updated(
        &self,
        id: String,
        project_id: String,
        payload: UpdateResult,
    ) -> Result<()> {
        let update_history = UpdateHistories::find_by_id(id.parse()?)
            .one(self.db.get())
            .await?
            .context("Update history record not found")?;
        let mut update_history_am: update_histories::ActiveModel = update_history.clone().into();

        if let UpdateResult::Success(signature) = payload {
            update_history_am.txn_signature = Set(Some(signature));
            update_history_am.status = Set(CreationStatus::Created);

            self.credits
                .confirm_deduction(TransactionId(update_history.credit_deduction_id))
                .await?;

            self.producer
                .send(
                    Some(&NftEvents {
                        event: Some(NftEvent::SolanaMintUpdated(SolanaUpdatedMintPayload {
                            mint_id: update_history.mint_id.to_string(),
                        })),
                    }),
                    Some(&NftEventKey {
                        id: update_history.id.to_string(),
                        project_id,
                        user_id: update_history.created_by.to_string(),
                    }),
                )
                .await?;
        } else {
            update_history_am.status = Set(CreationStatus::Failed);
        }

        update_history_am.update(self.db.get()).await?;

        Ok(())
    }
}

impl TryFrom<ProtoBlockchainEnum> for Blockchain {
    type Error = Error;

    fn try_from(v: ProtoBlockchainEnum) -> Result<Self> {
        match v {
            ProtoBlockchainEnum::Unspecified => Err(anyhow!("Invalid enum variant")),
            ProtoBlockchainEnum::Solana => Ok(Self::Solana),
            ProtoBlockchainEnum::Polygon => Ok(Self::Polygon),
            ProtoBlockchainEnum::Ethereum => Ok(Self::Ethereum),
        }
    }
}

impl TryFrom<TransactionStatus> for CreationStatus {
    type Error = Error;

    fn try_from(i: TransactionStatus) -> Result<Self> {
        match i {
            TransactionStatus::Unspecified => Err(anyhow!("Invalid enum variant")),
            TransactionStatus::Blocked => Ok(Self::Blocked),
            TransactionStatus::Failed => Ok(Self::Failed),
            TransactionStatus::Completed => Ok(Self::Created),
            TransactionStatus::Cancelled => Ok(Self::Canceled),
            TransactionStatus::Rejected => Ok(Self::Rejected),
            _ => Ok(Self::Pending),
        }
    }
}

impl From<SolanaCompletedMintTransaction> for MintTransaction {
    fn from(i: SolanaCompletedMintTransaction) -> Self {
        Self {
            signature: i.signature,
            address: i.address,
        }
    }
}

impl From<PolygonTransactionResult> for MintResult {
    fn from(i: PolygonTransactionResult) -> Self {
        match i.hash {
            None => Self::Failure,
            Some(signature) => Self::Success(MintTransaction {
                signature,
                address: format!("{}:{}", i.contract_address, i.edition_id),
            }),
        }
    }
}

impl From<PolygonTransactionResult> for TransferResult {
    fn from(i: PolygonTransactionResult) -> Self {
        match i.hash {
            None => Self::Failure,
            Some(signature) => Self::Success(signature),
        }
    }
}

async fn index_attributes(
    db: &Connection,
    json_id: Uuid,
    attributes: Vec<Attribute>,
) -> Result<()> {
    for attr in attributes {
        let attribute = metadata_json_attributes::ActiveModel {
            metadata_json_id: Set(json_id),
            trait_type: Set(attr.trait_type),
            value: Set(attr.value),
            ..Default::default()
        };

        attribute.insert(db.get()).await?;
    }

    Ok(())
}

async fn index_files(db: &Connection, json_id: Uuid, files: Vec<File>) -> Result<()> {
    for file in files {
        let file_am = metadata_json_files::ActiveModel {
            metadata_json_id: Set(json_id),
            uri: Set(Some(file.uri)),
            file_type: Set(file.mime),
            ..Default::default()
        };

        file_am.insert(db.get()).await?;
    }

    Ok(())
}
