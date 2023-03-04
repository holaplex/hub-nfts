use chrono::Local;
use hub_core::{anyhow::Result, clap, prelude::*};
use mpl_token_metadata::state::Creator;
use sea_orm::{prelude::*, Set};
use solana_client::rpc_client::RpcClient;
use solana_program::program_pack::Pack;
use solana_sdk::signer::{keypair::Keypair, Signer};
use spl_associated_token_account::get_associated_token_address;

use super::blockchain::{Blockchain, Transaction};
use crate::{db::Connection, entities::solana_collections};

#[derive(Debug, clap::Args, Clone)]
pub struct SolanaArgs {
    #[arg(long, env)]
    pub solana_endpoint: String,

    #[arg(long, env)]
    pub solana_keypair_path: String,
}

#[derive(Clone)]
pub struct Solana {
    rpc_client: Arc<RpcClient>,
    db: Connection,
    payer_keypair: Vec<u8>,
}

pub struct CreateDrop {
    pub creators: Vec<Creator>,
    pub owner_address: String,
    pub name: String,
    pub symbol: String,
    pub seller_fee_basis_points: u16,
    pub supply: Option<u64>,
    pub metadata_json_uri: String,
}

pub struct MintDrop {}

impl Solana {
    pub fn new(rpc_client: Arc<RpcClient>, db: Connection, payer_keypair: Vec<u8>) -> Self {
        Self {
            rpc_client,
            db,
            payer_keypair,
        }
    }
}

impl Blockchain<CreateDrop, MintDrop> for Solana {
    async fn drop(&self, input: CreateDrop, collection_id: Uuid) -> Result<Transaction> {
        let CreateDrop {
            creators,
            owner_address,
            name,
            symbol,
            seller_fee_basis_points,
            supply,
            metadata_json_uri,
        } = input;
        let rpc = &self.rpc_client;
        let conn = self.db.get();

        let payer = Keypair::from_bytes(&self.payer_keypair)?;
        let mint = Keypair::new();

        let owner = owner_address.parse()?;
        let ata = get_associated_token_address(&owner, &mint.pubkey());

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
            &payer.pubkey(),
            &mint.pubkey(),
            rent,
            len.try_into()?,
            &spl_token::ID,
        );

        let initialize_mint_ins = spl_token::instruction::initialize_mint(
            &spl_token::ID,
            &mint.pubkey(),
            &owner,
            Some(&owner),
            0,
        )?;

        let ata_ins = spl_associated_token_account::instruction::create_associated_token_account(
            &payer.pubkey(),
            &owner,
            &mint.pubkey(),
            &spl_token::ID,
        );

        let min_to_ins =
            spl_token::instruction::mint_to(&spl_token::ID, &mint.pubkey(), &ata, &owner, &[], 1)?;

        let create_metadata_account_ins =
            mpl_token_metadata::instruction::create_metadata_accounts_v3(
                mpl_token_metadata::ID,
                token_metadata_pubkey,
                mint.pubkey(),
                owner,
                payer.pubkey(),
                owner,
                name.clone(),
                symbol.clone(),
                metadata_json_uri.clone(),
                Some(creators),
                seller_fee_basis_points,
                true,
                true,
                None,
                None,
                None,
            );

        let create_master_edition_ins = mpl_token_metadata::instruction::create_master_edition_v3(
            mpl_token_metadata::ID,
            master_edition_pubkey,
            mint.pubkey(),
            owner,
            owner,
            token_metadata_pubkey,
            payer.pubkey(),
            supply,
        );

        let blockhash = rpc.get_latest_blockhash()?;

        let instructions = &[
            create_account_ins,
            initialize_mint_ins,
            ata_ins,
            min_to_ins,
            create_metadata_account_ins,
            create_master_edition_ins,
        ];

        let message = solana_program::message::Message::new_with_blockhash(
            instructions,
            Some(&payer.pubkey()),
            &blockhash,
        );

        let serialized_message = message.serialize();
        let mint_signature = mint.try_sign_message(&message.serialize())?;
        let payer_signature = payer.try_sign_message(&message.serialize())?;

        // TODO: drop created_by on solana_collection
        let solana_collections_active_model = solana_collections::ActiveModel {
            collection_id: Set(collection_id),
            master_edition_address: Set(master_edition_pubkey.to_string()),
            seller_fee_basis_points: Set(seller_fee_basis_points.try_into()?),
            created_at: Set(Local::now().naive_utc()),
            ata_pubkey: Set(ata.to_string()),
            owner_pubkey: Set(owner.to_string()),
            update_authority: Set(owner.to_string()),
            mint_pubkey: Set(mint.pubkey().to_string()),
            metadata_pubkey: Set(token_metadata_pubkey.to_string()),
            ..Default::default()
        };

        solana_collections_active_model.insert(conn).await?;

        Ok(Transaction {
            serialized_message,
            signed_message_signatures: vec![
                payer_signature.to_string(),
                mint_signature.to_string(),
            ],
        })
    }

    async fn edition(&self, _input: MintDrop) -> Result<Transaction> {
        todo!()
    }
}
