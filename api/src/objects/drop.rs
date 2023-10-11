use async_graphql::{Context, Enum, Error, Object, Result};
use hub_core::chrono::Utc;
use sea_orm::entity::prelude::*;

use super::{Collection, CollectionMint};
use crate::{
    entities::{
        drops, mint_histories,
        sea_orm_active_enums::{CreationStatus, DropType},
    },
    AppContext,
};

/// An NFT campaign that controls the minting rules for a collection, such as its start date and end date.
#[derive(Clone, Debug)]
pub struct Drop {
    pub id: Uuid,
    pub drop_type: DropType,
    pub project_id: Uuid,
    pub collection_id: Uuid,
    pub creation_status: CreationStatus,
    pub start_time: Option<DateTimeWithTimeZone>,
    pub end_time: Option<DateTimeWithTimeZone>,
    pub price: i64,
    pub created_by: Uuid,
    pub created_at: DateTimeWithTimeZone,
    pub paused_at: Option<DateTimeWithTimeZone>,
    pub shutdown_at: Option<DateTimeWithTimeZone>,
}

#[Object]
impl Drop {
    /// The unique identifier for the drop.
    async fn id(&self) -> Uuid {
        self.id
    }

    // The type of the drop.
    async fn drop_type(&self) -> DropType {
        self.drop_type
    }

    /// The identifier of the project to which the drop is associated.
    async fn project_id(&self) -> Uuid {
        self.project_id
    }

    /// The creation status of the drop.
    async fn creation_status(&self) -> CreationStatus {
        self.creation_status
    }

    /// The date and time in UTC when the drop is eligible for minting. A value of `null` means the drop can be minted immediately.
    async fn start_time(&self) -> Option<DateTimeWithTimeZone> {
        self.start_time
    }

    /// The end date and time in UTC for the drop. A value of `null` means the drop does not end until it is fully minted.
    async fn end_time(&self) -> Option<DateTimeWithTimeZone> {
        self.end_time
    }

    /// The cost to mint the drop in US dollars. When purchasing with crypto the user will be charged at the current conversion rate for the blockchain's native coin at the time of minting.
    async fn price(&self) -> i64 {
        self.price
    }

    /// The user id of the person who created the drop.
    async fn created_by_id(&self) -> Uuid {
        self.created_by
    }

    /// The date and time in UTC when the drop was created.
    async fn created_at(&self) -> DateTimeWithTimeZone {
        self.created_at
    }

    // The paused_at field represents the date and time in UTC when the drop was paused.
    // If it is null, the drop is currently not paused.
    async fn paused_at(&self) -> Option<DateTimeWithTimeZone> {
        self.paused_at
    }

    /// The shutdown_at field represents the date and time in UTC when the drop was shutdown
    /// If it is null, the drop is currently not shutdown
    async fn shutdown_at(&self) -> Option<DateTimeWithTimeZone> {
        self.shutdown_at
    }

    /// The collection for which the drop is managing mints.
    async fn collection(&self, ctx: &Context<'_>) -> Result<Option<Collection>> {
        let AppContext {
            collection_loader, ..
        } = ctx.data::<AppContext>()?;

        collection_loader.load_one(self.collection_id).await
    }

    /// The current status of the drop.
    async fn status(&self, ctx: &Context<'_>) -> Result<DropStatus> {
        let AppContext {
            collection_total_mints_loader,
            collection_supply_loader,
            ..
        } = ctx.data::<AppContext>()?;

        let now = Utc::now();
        let scheduled = self.start_time.map(|start_time| now < start_time);
        let expired = self.end_time.map(|end_time| now > end_time);
        let paused_at = self.paused_at;
        let shutdown_at = self.shutdown_at;

        let total_mints = collection_total_mints_loader
            .load_one(self.collection_id)
            .await?
            .ok_or(Error::new("Unable to find collection total mints"))?;
        let supply = collection_supply_loader
            .load_one(self.collection_id)
            .await?
            .ok_or(Error::new("Unable to find collection supply"))?;

        let minted = supply.map(|supply| supply == total_mints && total_mints > 0);

        match (
            scheduled,
            expired,
            minted,
            paused_at,
            shutdown_at,
            self.creation_status,
        ) {
            (_, _, _, Some(_), ..) => Ok(DropStatus::Paused),
            (_, _, _, _, Some(_), _) => Ok(DropStatus::Shutdown),
            (_, _, _, _, _, CreationStatus::Pending) => Ok(DropStatus::Creating),
            (
                _,
                _,
                _,
                _,
                _,
                CreationStatus::Blocked
                | CreationStatus::Canceled
                | CreationStatus::Failed
                | CreationStatus::Rejected,
            ) => Ok(DropStatus::Failed),
            (Some(true), ..) => Ok(DropStatus::Scheduled),
            (_, Some(true), ..) => Ok(DropStatus::Expired),
            (_, _, Some(true), ..) => Ok(DropStatus::Minted),
            (_, _, Some(false), ..) | (_, _, None, _, _, CreationStatus::Created) => {
                Ok(DropStatus::Minting)
            },
            (_, _, _, _, _, CreationStatus::Queued) => {
                Err(Error::new("Unable to calculate drop status"))
            },
        }
    }

    async fn queued_mints(&self, ctx: &Context<'_>) -> Result<Option<Vec<CollectionMint>>> {
        let AppContext {
            queued_mints_loader,
            ..
        } = ctx.data::<AppContext>()?;

        queued_mints_loader.load_one(self.id).await
    }

    #[graphql(deprecation = "Use `mint_histories` under `Collection` Object instead.")]
    /// A list of all NFT purchases from this drop.
    async fn purchases(&self, ctx: &Context<'_>) -> Result<Option<Vec<mint_histories::Model>>> {
        let AppContext {
            drop_mint_history_loader,
            ..
        } = ctx.data::<AppContext>()?;

        drop_mint_history_loader.load_one(self.id).await
    }
}

impl From<drops::Model> for Drop {
    fn from(
        drops::Model {
            id,
            drop_type,
            project_id,
            collection_id,
            creation_status,
            start_time,
            end_time,
            price,
            created_by,
            created_at,
            paused_at,
            shutdown_at,
            ..
        }: drops::Model,
    ) -> Self {
        Self {
            id,
            drop_type,
            project_id,
            collection_id,
            creation_status,
            start_time,
            end_time,
            price,
            created_by,
            created_at,
            paused_at,
            shutdown_at,
        }
    }
}

/// The different phases of a drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Enum)]
enum DropStatus {
    /// Actively minting.
    Minting,
    /// The minting process for the collection is complete.
    Minted,
    /// The drop is scheduled for minting.
    Scheduled,
    /// The drop has expired and its end time has passed.
    Expired,
    /// The drop is still being created and is not ready to mint.
    Creating,
    /// The drop is temporarily paused and cannot be minted at the moment.
    Paused,
    ///  The drop is permanently shut down and can no longer be minted.
    Shutdown,
    /// The creation process for the drop has failed
    Failed,
}
