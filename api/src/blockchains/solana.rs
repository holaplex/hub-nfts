use hub_core::{anyhow::Result, chrono::Utc, clap, prelude::*};
use mpl_token_metadata::{
    instruction::{mint_new_edition_from_master_edition_via_token, update_metadata_accounts_v2},
    state::{Creator, DataV2, EDITION, PREFIX},
};
use sea_orm::{prelude::*, Set};
use solana_client::rpc_client::RpcClient;
use solana_program::{program_pack::Pack, pubkey::Pubkey, system_instruction::create_account};
use solana_sdk::signer::{keypair::Keypair, Signer};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::{
    instruction::{initialize_mint, mint_to},
    state,
};

use super::{Edition, TransactionResponse};
use crate::{
    db::Connection,
    entities::{collection_creators, nft_transfers, solana_collections},
};

const TOKEN_PROGRAM_PUBKEY: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

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

#[derive(Clone)]
pub struct CreateDropRequest {
    pub creators: Vec<Creator>,
    pub owner_address: String,
    pub name: String,
    pub symbol: String,
    pub seller_fee_basis_points: u16,
    pub supply: Option<u64>,
    pub metadata_json_uri: String,
    pub collection: Uuid,
}

#[derive(Clone)]
pub struct CreateEditionRequest {
    pub collection: Uuid,
    pub recipient: String,
    pub owner_address: String,
    pub edition: u64,
}

#[derive(Clone)]
pub struct UpdateEditionRequest {
    pub collection: Uuid,
    pub owner_address: String,
    pub seller_fee_basis_points: Option<u16>,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub creators: Vec<Creator>,
}

#[derive(Clone)]
pub struct TransferAssetRequest {
    pub sender: String,
    pub recipient: String,
    pub mint_address: String,
}
impl Solana {
    pub fn new(rpc_client: Arc<RpcClient>, db: Connection, payer_keypair: Vec<u8>) -> Self {
        Self {
            rpc_client,
            db,
            payer_keypair,
        }
    }

    #[must_use]
    pub fn edition(
        &self,
    ) -> impl Edition<
        CreateDropRequest,
        CreateEditionRequest,
        UpdateEditionRequest,
        TransferAssetRequest,
        Pubkey,
    > {
        self.clone()
    }
}

#[async_trait::async_trait]

impl
    Edition<
        CreateDropRequest,
        CreateEditionRequest,
        UpdateEditionRequest,
        TransferAssetRequest,
        Pubkey,
    > for Solana
{
    /// Res
    ///
    /// # Errors
    /// This function fails if unable to assemble or save solana drop
    #[allow(clippy::too_many_lines)]
    async fn create(&self, request: CreateDropRequest) -> Result<(Pubkey, TransactionResponse)> {
        let CreateDropRequest {
            creators,
            owner_address,
            name,
            symbol,
            seller_fee_basis_points,
            supply,
            metadata_json_uri,
            collection,
        } = request;
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

        let solana_collections_active_model = solana_collections::ActiveModel {
            collection_id: Set(collection),
            master_edition_address: Set(master_edition_pubkey.to_string()),
            created_at: Set(Utc::now().naive_utc()),
            ata_pubkey: Set(ata.to_string()),
            owner_pubkey: Set(owner.to_string()),
            update_authority: Set(owner.to_string()),
            mint_pubkey: Set(mint.pubkey().to_string()),
            metadata_pubkey: Set(token_metadata_pubkey.to_string()),
            ..Default::default()
        };

        solana_collections_active_model.insert(conn).await?;

        Ok((mint.pubkey(), TransactionResponse {
            serialized_message,
            signed_message_signatures: vec![
                payer_signature.to_string(),
                mint_signature.to_string(),
            ],
        }))
    }

    /// Res
    ///
    /// # Errors
    /// This function fails if unable to assemble solana mint transaction
    async fn mint(&self, request: CreateEditionRequest) -> Result<(Pubkey, TransactionResponse)> {
        let conn = self.db.get();
        let rpc = &self.rpc_client;
        let CreateEditionRequest {
            collection,
            recipient,
            owner_address,
            edition,
        } = request;

        let payer = Keypair::from_bytes(&self.payer_keypair)?;
        let owner = owner_address.parse()?;

        let solana_collection_model = solana_collections::Entity::find()
            .filter(solana_collections::Column::CollectionId.eq(collection))
            .one(conn)
            .await?;

        let sc = solana_collection_model.ok_or_else(|| anyhow!("solana collection not found"))?;

        let program_pubkey = mpl_token_metadata::id();
        let master_edition_pubkey: Pubkey = sc.master_edition_address.parse()?;
        let master_edition_mint: Pubkey = sc.mint_pubkey.parse()?;
        let existing_token_account: Pubkey = sc.ata_pubkey.parse()?;
        let recipient: Pubkey = recipient.parse()?;

        let token_key = Pubkey::from_str(TOKEN_PROGRAM_PUBKEY)?;

        let new_mint_key = Keypair::new();
        let added_token_account = get_associated_token_address(&recipient, &new_mint_key.pubkey());
        let new_mint_pub = new_mint_key.pubkey();
        let edition_seeds = &[
            PREFIX.as_bytes(),
            program_pubkey.as_ref(),
            new_mint_pub.as_ref(),
            EDITION.as_bytes(),
        ];
        let (edition_key, _) = Pubkey::find_program_address(edition_seeds, &program_pubkey);

        let metadata_seeds = &[
            PREFIX.as_bytes(),
            program_pubkey.as_ref(),
            new_mint_pub.as_ref(),
        ];
        let (metadata_key, _) = Pubkey::find_program_address(metadata_seeds, &program_pubkey);

        let mut instructions = vec![
            create_account(
                &payer.pubkey(),
                &new_mint_key.pubkey(),
                rpc.get_minimum_balance_for_rent_exemption(state::Mint::LEN)?,
                state::Mint::LEN as u64,
                &token_key,
            ),
            initialize_mint(&token_key, &new_mint_key.pubkey(), &owner, Some(&owner), 0)?,
            create_associated_token_account(
                &payer.pubkey(),
                &recipient,
                &new_mint_key.pubkey(),
                &spl_token::ID,
            ),
            mint_to(
                &token_key,
                &new_mint_key.pubkey(),
                &added_token_account,
                &owner,
                &[&owner],
                1,
            )?,
        ];

        instructions.push(mint_new_edition_from_master_edition_via_token(
            program_pubkey,
            metadata_key,
            edition_key,
            master_edition_pubkey,
            new_mint_key.pubkey(),
            owner,
            payer.pubkey(),
            owner,
            existing_token_account,
            owner,
            sc.metadata_pubkey.parse()?,
            master_edition_mint,
            edition,
        ));

        let blockhash = rpc.get_latest_blockhash()?;

        let message = solana_program::message::Message::new_with_blockhash(
            &instructions,
            Some(&payer.pubkey()),
            &blockhash,
        );

        let serialized_message = message.serialize();
        let mint_signature = new_mint_key.try_sign_message(&message.serialize())?;
        let payer_signature = payer.try_sign_message(&message.serialize())?;

        Ok((new_mint_key.pubkey(), TransactionResponse {
            serialized_message,
            signed_message_signatures: vec![
                payer_signature.to_string(),
                mint_signature.to_string(),
            ],
        }))
    }

    async fn update(&self, request: UpdateEditionRequest) -> Result<(Pubkey, TransactionResponse)> {
        let conn = self.db.get();
        let rpc = &self.rpc_client;
        let UpdateEditionRequest {
            collection,
            owner_address,
            seller_fee_basis_points,
            name,
            symbol,
            uri,
            creators,
        } = request.clone();

        let payer = Keypair::from_bytes(&self.payer_keypair)?;
        let solana_collection_model = solana_collections::Entity::find()
            .filter(solana_collections::Column::CollectionId.eq(collection))
            .one(conn)
            .await?;
        let sc = solana_collection_model.ok_or_else(|| anyhow!("solana collection not found"))?;

        let program_pubkey = mpl_token_metadata::id();

        let ins = update_metadata_accounts_v2(
            program_pubkey,
            sc.metadata_pubkey.parse()?,
            owner_address.parse()?,
            None,
            Some(DataV2 {
                name,
                symbol,
                uri,
                seller_fee_basis_points: seller_fee_basis_points.unwrap_or_default(),
                creators: Some(creators),
                collection: None,
                uses: None,
            }),
            None,
            None,
        );

        let blockhash = rpc.get_latest_blockhash()?;

        let message = solana_program::message::Message::new_with_blockhash(
            &[ins],
            Some(&payer.pubkey()),
            &blockhash,
        );

        let serialized_message = message.serialize();
        let payer_signature = payer.try_sign_message(&message.serialize())?;

        Ok((sc.mint_pubkey.parse()?, TransactionResponse {
            serialized_message,
            signed_message_signatures: vec![payer_signature.to_string()],
        }))
    }

    async fn transfer(
        &self,
        request: TransferAssetRequest,
    ) -> Result<(Pubkey, TransactionResponse)> {
        let rpc = &self.rpc_client;
        let db = self.db.get();
        let TransferAssetRequest {
            sender,
            recipient,
            mint_address,
        } = request;

        let sender: Pubkey = sender.parse()?;
        let recipient: Pubkey = recipient.parse()?;
        let mint_address: Pubkey = mint_address.parse()?;
        let payer = Keypair::from_bytes(&self.payer_keypair)?;
        let blockhash = rpc.get_latest_blockhash()?;
        let source_ata = get_associated_token_address(&sender, &mint_address);
        let destination_ata = get_associated_token_address(&recipient, &mint_address);

        let create_ata_token_account = create_associated_token_account(
            &payer.pubkey(),
            &recipient,
            &mint_address,
            &spl_token::ID,
        );

        let transfer_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            &source_ata,
            &destination_ata,
            &sender,
            &[&sender],
            1,
        )
        .context("Failed to create transfer instruction")?;

        let close_ata = spl_token::instruction::close_account(
            &spl_token::ID,
            &source_ata,
            &payer.pubkey(),
            &sender,
            &[&sender],
        )?;

        let message = solana_program::message::Message::new_with_blockhash(
            &[create_ata_token_account, transfer_instruction, close_ata],
            Some(&payer.pubkey()),
            &blockhash,
        );

        let serialized_message = message.serialize();
        let payer_signature = payer.try_sign_message(&message.serialize())?;

        let nft_transfer_am = nft_transfers::ActiveModel {
            tx_signature: Set(None),
            mint_address: Set(mint_address.to_string()),
            sender: Set(sender.to_string()),
            recipient: Set(recipient.to_string()),
            ..Default::default()
        };

        nft_transfer_am.insert(db).await?;

        Ok((mint_address, TransactionResponse {
            serialized_message,
            signed_message_signatures: vec![payer_signature.to_string()],
        }))
    }
}

impl TryFrom<collection_creators::Model> for Creator {
    type Error = Error;

    fn try_from(
        collection_creators::Model {
            address,
            verified,
            share,
            ..
        }: collection_creators::Model,
    ) -> Result<Self> {
        Ok(Self {
            address: address.parse()?,
            verified,
            share: share.try_into()?,
        })
    }
}
