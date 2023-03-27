//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use async_graphql::Enum;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Enum, Copy, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "blockchain")]
pub enum Blockchain {
    #[sea_orm(string_value = "ethereum")]
    Ethereum,
    #[sea_orm(string_value = "polygon")]
    Polygon,
    #[sea_orm(string_value = "solana")]
    Solana,
}
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Enum, Copy)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "creation_status")]
pub enum CreationStatus {
    #[sea_orm(string_value = "created")]
    Created,
    #[sea_orm(string_value = "paused")]
    Paused,
    #[sea_orm(string_value = "pending")]
    Pending,
}
