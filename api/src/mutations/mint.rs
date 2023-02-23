use std::{str::FromStr, sync::Arc};

use async_graphql::{self, Context, Error, InputObject, Object, Result};
use mpl_token_metadata::{
    instruction::mint_new_edition_from_master_edition_via_token,
    state::{EDITION, PREFIX},
};
use sea_orm::{prelude::*, JoinType, QuerySelect, Set};
use solana_client::rpc_client::RpcClient;
use solana_program::{program_pack::Pack, pubkey::Pubkey, system_instruction::create_account};
use solana_sdk::{
    signer::{keypair::Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::{
    instruction::{initialize_mint, mint_to},
    state::Mint,
};

use crate::{
    entities::{
        collection_mints, drops,
        prelude::{Collections, Drops, SolanaCollections},
        sea_orm_active_enums::CreationStatus,
        solana_collections,
    },
    AppContext, UserID,
};

const TOKEN_PROGRAM_PUBKEY: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

#[derive(Default)]
pub struct Mutation;

#[Object(name = "MintMutation")]
impl Mutation {
    /// Res
    ///
    /// # Errors
    /// This function fails if ...
    pub async fn mint_edition(&self, ctx: &Context<'_>, input: MintDropInput) -> Result<String> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let rpc = &**ctx.data::<Arc<RpcClient>>()?;
        let UserID(id) = user_id;
        let user_id = id.ok_or_else(|| Error::new("X-USER-ID header not found"))?;

        let keypair_bytes = ctx.data::<Vec<u8>>()?;

        let wallet = Keypair::from_bytes(keypair_bytes)?;

        let drop_model = Drops::find()
            .select_also(Collections)
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .filter(drops::Column::Id.eq(input.drop))
            .one(db.get())
            .await?;

        let (drop, collection_model) =
            drop_model.ok_or_else(|| Error::new("Drop not found in db"))?;
        let collection =
            collection_model.ok_or_else(|| Error::new("Collection not found in db"))?;

        let solana_collection_model = SolanaCollections::find()
            .filter(solana_collections::Column::Id.eq(collection.collection))
            .one(db.get())
            .await?;

        let sc =
            solana_collection_model.ok_or_else(|| Error::new("Solana Collection not found"))?;

        let program_pubkey = mpl_token_metadata::id();
        let master_edition_pubkey: Pubkey = sc.address.parse()?;
        let master_edition_mint: Pubkey = sc.mint_pubkey.parse()?;
        let existing_token_account: Pubkey = sc.ata_pubkey.parse()?;

        let token_key = Pubkey::from_str(TOKEN_PROGRAM_PUBKEY).unwrap();

        let new_mint_key = Keypair::new();
        let added_token_account =
            get_associated_token_address(&wallet.pubkey(), &new_mint_key.pubkey());
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
                &wallet.pubkey(),
                &new_mint_key.pubkey(),
                rpc.get_minimum_balance_for_rent_exemption(Mint::LEN)
                    .unwrap(),
                Mint::LEN as u64,
                &token_key,
            ),
            initialize_mint(
                &token_key,
                &new_mint_key.pubkey(),
                &wallet.pubkey(),
                Some(&wallet.pubkey()),
                0,
            )
            .unwrap(),
            create_associated_token_account(
                &wallet.pubkey(),
                &wallet.pubkey(),
                &new_mint_key.pubkey(),
                &spl_token::ID,
            ),
            mint_to(
                &token_key,
                &new_mint_key.pubkey(),
                &added_token_account,
                &wallet.pubkey(),
                &[&wallet.pubkey()],
                1,
            )
            .unwrap(),
        ];

        instructions.push(mint_new_edition_from_master_edition_via_token(
            program_pubkey,
            metadata_key,
            edition_key,
            master_edition_pubkey,
            new_mint_key.pubkey(),
            wallet.pubkey(),
            wallet.pubkey(),
            wallet.pubkey(),
            existing_token_account,
            sc.update_authority.parse()?,
            sc.metadata_pubkey.parse()?,
            master_edition_mint,
            input.edition,
        ));

        let destination_token_account =
            get_associated_token_address(&input.destination.parse()?, &new_mint_key.pubkey());

        instructions.push(create_associated_token_account(
            &wallet.pubkey(),
            &input.destination.parse()?,
            &new_mint_key.pubkey(),
            &spl_token::ID,
        ));

        // instructions.push(create_account(
        //     &input.destination.parse()?,
        //     &new_mint_key.pubkey(),
        //     rpc.get_minimum_balance_for_rent_exemption(Mint::LEN)
        //         .unwrap(),
        //     Mint::LEN as u64,
        //     &token_key,
        // ));

        instructions.push(spl_token::instruction::transfer(
            &spl_token::id(),
            &added_token_account,
            &destination_token_account,
            &wallet.pubkey(),
            &[&wallet.pubkey()],
            1,
        )?);

        let recent_blockhash = rpc.get_latest_blockhash().unwrap();

        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&wallet.pubkey()),
            &[&new_mint_key, &wallet],
            recent_blockhash,
        );

        let signature = rpc.send_and_confirm_transaction(&tx)?;

        let collection_mint_active_model = collection_mints::ActiveModel {
            drop_id: Set(drop.id),
            address: Set(edition_key.to_string()),
            owner: Set(wallet.pubkey().to_string()),
            creation_status: Set(CreationStatus::Created),
            created_by: Set(user_id),
            ..Default::default()
        };

        collection_mint_active_model.insert(db.get()).await?;

        Ok(format!(
            "Minted edition signautre {:?}",
            signature.to_string()
        ))
    }
}
#[derive(Debug, Clone, InputObject)]
pub struct MintDropInput {
    drop: Uuid,
    owner_address: String,
    destination: String,
    edition: u64,
}
