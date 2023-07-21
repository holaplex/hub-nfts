use hub_core::{
    chrono::{DateTime, NaiveDateTime, Utc},
    credits::{CreditsClient, TransactionId},
    prelude::*,
    producer::Producer,
    uuid::Uuid,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, JoinType, QueryFilter, QuerySelect, RelationTrait,
    Set, TransactionTrait,
};

use crate::{
    db::Connection,
    entities::{
        collection_mints, collections, customer_wallets, drops, nft_transfers,
        prelude::{CollectionMints, Purchases},
        project_wallets, purchases,
        sea_orm_active_enums::{Blockchain, CreationStatus},
        transfer_charges,
    },
    proto::{
        nft_events::Event as NftEvent,
        polygon_nft_events::Event as PolygonNftEvents,
        solana_nft_events::Event as SolanaNftsEvent,
        treasury_events::{
            Blockchain as ProtoBlockchainEnum, CustomerWallet, Event as TreasuryEvent,
            PolygonTransactionResult, ProjectWallet, TransactionStatus,
        },
        CreationStatus as NftCreationStatus, DropCreation, MintCreation, MintOwnershipUpdate,
        MintedTokensOwnershipUpdate, NftEventKey, NftEvents, SolanaCompletedMintTransaction,
        SolanaCompletedTransferTransaction, SolanaNftEventKey, TreasuryEventKey,
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
            Services::Solana(SolanaNftEventKey { id, .. }, e) => match e.event {
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
                Some(SolanaNftsEvent::TransferAssetFailed(_)) => {
                    self.mint_transferred(id, TransferResult::Failure).await
                },
                Some(SolanaNftsEvent::RetryMintDropFailed(_)) => {
                    self.drop_minted(id, MintResult::Failure).await
                },
                Some(SolanaNftsEvent::UpdateMintOwner(e)) => self.update_mint_owner(id, e).await,
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

        let (collection_model, drop) = collections::Entity::find_by_id(collection_id)
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

        // TODO: add unique event for collection created
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

        let collection_mint = collection_mints::Entity::find_by_id(collection_mint_id)
            .one(conn)
            .await
            .context("failed to load collection mint from db")?
            .context("collection mint not found in db")?;

        let (purchase, drop) = Purchases::find()
            .find_also_related(drops::Entity)
            .filter(purchases::Column::MintId.eq(collection_mint_id))
            .one(conn)
            .await
            .context("failed to load purchase from db")?
            .context("purchase not found in db")?;

        let drop = drop.context("drop not found")?;

        let mut collection_mint_active_model: collection_mints::ActiveModel =
            collection_mint.clone().into();
        let mut purchase_am: purchases::ActiveModel = purchase.into();
        let mut creation_status = NftCreationStatus::Completed;

        if let MintResult::Success(MintTransaction { signature, address }) = payload {
            purchase_am.status = Set(CreationStatus::Created);
            purchase_am.tx_signature = Set(Some(signature.clone()));
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
            purchase_am.status = Set(CreationStatus::Failed);
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
        purchase_am.update(conn).await?;

        Ok(())
    }

    async fn mint_transferred(&self, id: String, payload: TransferResult) -> Result<()> {
        let conn = self.db.get();
        let transfer_id = Uuid::from_str(&id)?;

        let transfer_charge = transfer_charges::Entity::find()
            .filter(transfer_charges::Column::CreditsDeductionId.eq(transfer_id))
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
