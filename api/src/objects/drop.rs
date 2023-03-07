#![allow(clippy::unused_async)]

use async_graphql::{Enum, FieldError, Object, Result};
use chrono::Utc;
use sea_orm::entity::prelude::*;

use super::Collection;
use crate::entities::{collections, drops, sea_orm_active_enums::CreationStatus};

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
    async fn id(&self) -> Uuid {
        self.drop.id
    }

    async fn project_id(&self) -> Uuid {
        self.drop.project_id
    }

    async fn creation_status(&self) -> CreationStatus {
        self.drop.creation_status
    }

    async fn start_time(&self) -> Option<DateTime> {
        self.drop.start_time
    }

    async fn end_time(&self) -> Option<DateTime> {
        self.drop.end_time
    }

    async fn price(&self) -> i64 {
        self.drop.price
    }

    async fn created_by_id(&self) -> Uuid {
        self.drop.created_by
    }

    async fn created_at(&self) -> DateTime {
        self.drop.created_at
    }

    async fn collection(&self) -> Collection {
        self.collection.clone().into()
    }

    async fn status(&self) -> Result<DropStatus> {
        let now = Utc::now().naive_utc();
        let scheduled = self.drop.start_time.map(|start_time| now < start_time);
        let expired = self.drop.end_time.map(|end_time| now > end_time);
        let minted = self
            .collection
            .supply
            .map(|supply| supply == self.collection.total_mints);

        match (scheduled, expired, minted, self.drop.creation_status) {
            (_, _, _, CreationStatus::Pending) => Ok(DropStatus::Creating),
            (Some(true), ..) => Ok(DropStatus::Scheduled),
            (_, Some(true), ..) => Ok(DropStatus::Expired),
            (_, _, Some(true), _) => Ok(DropStatus::Minted),
            (_, _, Some(false), _) => Ok(DropStatus::Minting),
            _ => Err(FieldError::new("unsupported drop status")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Enum)]
enum DropStatus {
    Minting,
    Minted,
    Scheduled,
    Expired,
    Creating,
}
