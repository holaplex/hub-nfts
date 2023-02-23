use std::sync::Arc;

use async_graphql::{self, Context, Error, InputObject, Object, Result};
use chrono::{DateTime, Local, Utc};
use mpl_token_metadata::state::Creator;
use sea_orm::{prelude::*, Set};
use solana_client::rpc_client::RpcClient;
use solana_program::program_pack::Pack;
use solana_sdk::{
    signer::{keypair::Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;

use crate::{
    entities::{
        collections, drops,
        sea_orm_active_enums::{Blockchain, CreationStatus},
        solana_collections,
    },
    AppContext, UserID,
};

#[derive(Default)]
pub struct Mutation;

#[Object(name = "DropMutation")]
impl Mutation {
    /// Res
    ///
    /// # Errors
    /// This function fails if ...
    pub async fn create_drop(
        &self,
        ctx: &Context<'_>,
        input: CreateDropInput,
    ) -> Result<drops::Model> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let UserID(id) = user_id;

        let user_id = id.ok_or_else(|| Error::new("X-USER-ID header not found"))?;

        let keypair_bytes = ctx.data::<Vec<u8>>()?;

        let rpc = &**ctx.data::<Arc<RpcClient>>()?;

        let owner = Keypair::from_bytes(keypair_bytes)?;

        let mint = Keypair::new();

        let update_authority = input.update_authority_address.parse()?;

        let ata = get_associated_token_address(&owner.pubkey(), &mint.pubkey());

        let (token_metadata_pubkey, _) = solana_program::pubkey::Pubkey::find_program_address(
            &[
                b"metadata",
                mpl_token_metadata::ID.as_ref(),
                mint.pubkey().as_ref(),
            ],
            &mpl_token_metadata::ID,
        );

        let (master_edition_pubkey, _) = solana_program::pubkey::Pubkey::find_program_address(
            &[
                b"metadata",
                mpl_token_metadata::ID.as_ref(),
                mint.pubkey().as_ref(),
                b"edition",
            ],
            &mpl_token_metadata::ID,
        );

        let len = spl_token::state::Mint::LEN;

        let rent = rpc.get_minimum_balance_for_rent_exemption(len)?;

        let create_account_ins = solana_program::system_instruction::create_account(
            &owner.pubkey(),
            &mint.pubkey(),
            rent,
            len.try_into()?,
            &spl_token::ID,
        );

        let initialize_mint_ins = spl_token::instruction::initialize_mint(
            &spl_token::ID,
            &mint.pubkey(),
            &owner.pubkey(),
            Some(&owner.pubkey()),
            0,
        )?;

        let ata_ins = spl_associated_token_account::instruction::create_associated_token_account(
            &owner.pubkey(),
            &owner.pubkey(),
            &mint.pubkey(),
            &spl_token::ID,
        );

        let min_to_ins = spl_token::instruction::mint_to(
            &spl_token::ID,
            &mint.pubkey(),
            &ata,
            &owner.pubkey(),
            &[],
            1,
        )?;

        let creators = input.creators.as_ref().map(|creators| {
            creators
                .iter()
                .map(|creator| creator.clone().try_into().unwrap())
                .collect()
        });
        let create_metadata_account_ins =
            mpl_token_metadata::instruction::create_metadata_accounts_v3(
                mpl_token_metadata::ID,
                token_metadata_pubkey,
                mint.pubkey(),
                owner.pubkey(),
                owner.pubkey(),
                update_authority,
                input.name.clone(),
                input.symbol.clone(),
                input.uri.clone(),
                creators,
                input.seller_fee_basis_points,
                input.update_authority_is_signer,
                input.is_mutable,
                None,
                None,
                None,
            );

        let create_master_edition_ins = mpl_token_metadata::instruction::create_master_edition_v3(
            mpl_token_metadata::ID,
            master_edition_pubkey,
            mint.pubkey(),
            update_authority,
            owner.pubkey(),
            token_metadata_pubkey,
            owner.pubkey(),
            input.supply,
        );

        let blockhash = rpc.get_latest_blockhash()?;

        let tx = Transaction::new_signed_with_payer(
            &[
                create_account_ins,
                initialize_mint_ins,
                ata_ins,
                min_to_ins,
                create_metadata_account_ins,
                create_master_edition_ins,
            ],
            Some(&owner.pubkey()),
            &[&owner, &mint],
            blockhash,
        );

        rpc.send_and_confirm_transaction(&tx)?;

        let solana_collections_active_model = solana_collections::ActiveModel {
            project_id: Set(input.project_id),
            address: Set(master_edition_pubkey.to_string()),
            seller_fee_basis_points: Set(input.seller_fee_basis_points.try_into()?),
            created_by: Set(user_id),
            created_at: Set(Local::now().naive_utc()),
            ata_pubkey: Set(ata.to_string()),
            owner_pubkey: Set(owner.pubkey().to_string()),
            update_authority: Set(input.update_authority_address),
            mint_pubkey: Set(mint.pubkey().to_string()),
            metadata_pubkey: Set(token_metadata_pubkey.to_string()),
            ..Default::default()
        };

        let solana_collection = solana_collections_active_model.insert(db.get()).await?;

        let collection_active_model = collections::ActiveModel {
            collection: Set(solana_collection.id),
            blockchain: Set(input.blockchain),
            name: Set(input.name),
            description: Set(input.description),
            metadata_uri: Set(input.uri),
            royalty_wallet: Set(input.royalty_address.to_string()),
            supply: Set(input.supply.map(|s| s.try_into().unwrap_or_default())),
            creation_status: Set(CreationStatus::Created),
            ..Default::default()
        };

        let collection = collection_active_model.insert(db.get()).await?;

        let drop = drops::ActiveModel {
            project_id: Set(input.project_id),
            organization_id: Set(input.organization_id),
            collection_id: Set(collection.id),
            creation_status: Set(CreationStatus::Pending),
            start_time: Set(input.start_time.naive_utc()),
            end_time: Set(input.end_time.naive_utc()),
            price: Set(input.price.try_into()?),
            created_by: Set(user_id),
            created_at: Set(Local::now().naive_utc()),
            ..Default::default()
        };

        let drop_model = drop.insert(db.get()).await?;

        Ok(drop_model)
    }
}

#[derive(Debug, Clone, InputObject)]
pub struct CreateDropInput {
    royalty_address: String,
    update_authority_address: String,
    project_id: Uuid,
    organization_id: Uuid,
    price: u64,
    name: String,
    description: String,
    symbol: String,
    uri: String,
    creators: Option<Vec<MetadataCreator>>,
    seller_fee_basis_points: u16,
    update_authority_is_signer: bool,
    is_mutable: bool,
    supply: Option<u64>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    blockchain: Blockchain,
}

#[derive(Debug, Clone, InputObject)]
pub struct MetadataCreator {
    pub address: String,
    pub verified: bool,
    pub share: u8,
}

impl TryFrom<MetadataCreator> for Creator {
    type Error = Error;

    fn try_from(
        MetadataCreator {
            address,
            verified,
            share,
        }: MetadataCreator,
    ) -> Result<Self> {
        Ok(Self {
            address: address.parse()?,
            verified,
            share,
        })
    }
}
