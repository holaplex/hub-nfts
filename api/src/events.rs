use hub_core::{
    credits::{CreditsClient, TransactionId},
    prelude::*,
    producer::Producer,
    uuid::Uuid,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, JoinType, QueryFilter, QuerySelect, RelationTrait,
    Set,
};

use crate::{
    db::Connection,
    entities::{
        collection_mints, collections, drops, nft_transfers,
        prelude::Purchases,
        project_wallets, purchases,
        sea_orm_active_enums::{Blockchain, CreationStatus},
    },
    proto::{
        nft_events::Event as NftEvent,
        solana_nft_events::Event as SolanaNftsEvent,
        treasury_events::{
            Blockchain as ProtoBlockchainEnum, Event as TreasuryEvent, PolygonTransactionResult,
            ProjectWallet, TransactionStatus,
        },
        CreationStatus as NftCreationStatus, NftEventKey, NftEvents,
        SolanaCompletedMintTransaction, SolanaCompletedTransferTransaction, SolanaNftEventKey,
        TreasuryEventKey,
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
                Some(SolanaNftsEvent::CreateDropFailed(_)) => {
                    self.drop_created(id, MintResult::Failure).await
                },
                Some(SolanaNftsEvent::MintDropFailed(_)) => {
                    self.drop_minted(id, MintResult::Failure).await
                },
                Some(SolanaNftsEvent::TransferAssetFailed(_)) => {
                    self.mint_transferred(id, TransferResult::Failure).await
                },
                Some(SolanaNftsEvent::RetryMintDropFailed(_)) => {
                    self.drop_minted(id, MintResult::Failure).await
                },
                Some(SolanaNftsEvent::RetryCreateDropFailed(_)) => {
                    self.drop_created(id, MintResult::Failure).await
                },
                None | Some(_) => Ok(()),
            },
        }
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
                    event: Some(NftEvent::DropCreated(creation_status as i32)),
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

    async fn drop_minted(&self, id: String, payload: MintResult) -> Result<()> {
        let conn = self.db.get();
        let collection_mint_id = Uuid::from_str(&id)?;

        let collection_mint = collection_mints::Entity::find_by_id(collection_mint_id)
            .one(conn)
            .await
            .context("failed to load collection mint from db")?
            .context("collection mint not found in db")?;

        let (purchase, drop) = Purchases::find()
            .join(JoinType::InnerJoin, purchases::Relation::Drop.def())
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
                    event: Some(NftEvent::DropMinted(creation_status as i32)),
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

        let (nft_transfer, collection_mint) = nft_transfers::Entity::find_by_id(transfer_id)
            .find_also_related(collection_mints::Entity)
            .one(conn)
            .await?
            .context("failed to load nft transfer from db")?;

        let collection_mint = collection_mint.context("collection mint not found")?;

        let mut collection_mint_am: collection_mints::ActiveModel = collection_mint.into();
        let mut nft_transfer_am: nft_transfers::ActiveModel = nft_transfer.clone().into();

        if let TransferResult::Success(signature) = payload {
            collection_mint_am.owner = Set(nft_transfer.recipient.clone());
            nft_transfer_am.tx_signature = Set(Some(signature));

            let deduction_id = nft_transfer
                .credits_deduction_id
                .context("deduction id not found")?;

            self.credits
                .confirm_deduction(TransactionId(deduction_id))
                .await?;
        }

        collection_mint_am.update(conn).await?;
        nft_transfer_am.insert(conn).await?;

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
