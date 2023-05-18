use async_graphql::{Context, Enum, Object, Result};
use hub_core::chrono::Utc;
use sea_orm::entity::prelude::*;

use super::Collection;
use crate::{
    entities::{collections, drops, purchases, sea_orm_active_enums::CreationStatus},
    AppContext,
};

/// An NFT campaign that controls the minting rules for a collection, such as its start date and end date.
#[derive(Clone, Debug)]
pub struct Drop {
    pub drop: drops::Model,
    pub collection: collections::Model,
}

impl Drop {
    #[must_use]
    pub fn new(drop: drops::Model, collection: collections::Model) -> Self {
        Self { drop, collection }
    }
}

#[Object]
impl Drop {
    /// The unique identifier for the drop.
    async fn id(&self) -> Uuid {
        self.drop.id
    }

    /// The identifier of the project to which the drop is associated.
    async fn project_id(&self) -> Uuid {
        self.drop.project_id
    }

    /// The creation status of the drop.
    async fn creation_status(&self) -> CreationStatus {
        self.drop.creation_status
    }

    /// The date and time in UTC when the drop is eligible for minting. A value of `null` means the drop can be minted immediately.
    async fn start_time(&self) -> Option<DateTimeWithTimeZone> {
        self.drop.start_time
    }

    /// The end date and time in UTC for the drop. A value of `null` means the drop does not end until it is fully minted.
    async fn end_time(&self) -> Option<DateTimeWithTimeZone> {
        self.drop.end_time
    }

    /// The cost to mint the drop in US dollars. When purchasing with crypto the user will be charged at the current conversion rate for the blockchain's native coin at the time of minting.
    async fn price(&self) -> i64 {
        self.drop.price
    }

    /// The user id of the person who created the drop.
    async fn created_by_id(&self) -> Uuid {
        self.drop.created_by
    }

    /// The date and time in UTC when the drop was created.
    async fn created_at(&self) -> DateTimeWithTimeZone {
        self.drop.created_at
    }

    // The paused_at field represents the date and time in UTC when the drop was paused.
    // If it is null, the drop is currently not paused.
    async fn paused_at(&self) -> Option<DateTimeWithTimeZone> {
        self.drop.paused_at
    }

    /// The shutdown_at field represents the date and time in UTC when the drop was shutdown
    /// If it is null, the drop is currently not shutdown
    async fn shutdown_at(&self) -> Option<DateTimeWithTimeZone> {
        self.drop.shutdown_at
    }

    /// The collection for which the drop is managing mints.
    async fn collection(&self) -> Result<Collection> {
        Ok(self.collection.clone().into())
    }

    /// The current status of the drop.
    async fn status(&self) -> Result<DropStatus> {
        let now = Utc::now();
        let scheduled = self.drop.start_time.map(|start_time| now < start_time);
        let expired = self.drop.end_time.map(|end_time| now > end_time);
        let paused_at = self.drop.paused_at;
        let shutdown_at = self.drop.shutdown_at;

        let minted = self
            .collection
            .supply
            .map(|supply| supply == self.collection.total_mints);

        match (
            scheduled,
            expired,
            minted,
            paused_at,
            shutdown_at,
            self.drop.creation_status,
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
        }
    }

    /// A list of all NFT purchases from this drop.
    async fn purchases(&self, ctx: &Context<'_>) -> Result<Option<Vec<purchases::Model>>> {
        let AppContext {
            drop_purchases_loader,
            ..
        } = ctx.data::<AppContext>()?;

        drop_purchases_loader.load_one(self.drop.id).await
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
