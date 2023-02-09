use hub_core::{prelude::*, uuid::Uuid};
use poem::{web::Data, Result};
use poem_openapi::{
    param::{Header, Path},
    payload::Json,
    Object, OpenApi,
};
use sea_orm::{prelude::*, Set};
use solana_client::rpc_client::RpcClient;
use solana_program::{borsh::get_packed_len, program_pack::Pack};
use solana_sdk::{
    signer::{keypair::Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use spl_token::state::Account;

use crate::{entities::prelude::*, AppState};

pub struct NftApi;

#[OpenApi]
impl NftApi {
    #[oai(path = "/create", method = "post")]
    async fn create(
        &self,
        state: Data<&AppState>,
        #[oai(name = "X-ORGANIZATION-ID")] organization: Header<Uuid>,
        _owner_address: String,
    ) -> Result<Json<String>> {
        let Data(state) = state;
        let Header(organization) = organization;
        let conn = state.connection.get();

        let rpc = &*state.rpc;

        let owner_keypair = Keypair::new();
        rpc.request_airdrop(&owner_keypair.pubkey(), 1000000)
            .unwrap();
        let owner = owner_keypair.pubkey();

        // let owner = "5pqCSBiXmiBtjdg85A3JFeMEYUGfW19RdUqd1Y8pJsdr"
        //     .to_string()
        //     .parse()
        //     .unwrap();

        let mint = Keypair::new();

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

        let rent = rpc.get_minimum_balance_for_rent_exemption(len).unwrap();

        let create_account_ins = solana_program::system_instruction::create_account(
            &owner,
            &mint.pubkey(),
            rent,
            len.try_into().unwrap(),
            &spl_token::ID,
        );

        let initialize_mint_ins = spl_token::instruction::initialize_mint(
            &spl_token::ID,
            &mint.pubkey(),
            &owner,
            Some(&owner),
            0,
        )
        .unwrap();

        let ata_ins = spl_associated_token_account::instruction::create_associated_token_account(
            &owner,
            &owner,
            &mint.pubkey(),
            &spl_token::ID,
        );

        let min_to_ins =
            spl_token::instruction::mint_to(&spl_token::ID, &mint.pubkey(), &ata, &owner, &[], 1)
                .unwrap();

        let create_metadata_account_ins =
            mpl_token_metadata::instruction::create_metadata_accounts_v3(
                mpl_token_metadata::ID,
                token_metadata_pubkey,
                mint.pubkey(),
                owner,
                owner,
                owner,
                "test".to_string(),
                "T".to_string(),
                "http://t.com".to_string(),
                None,
                100,
                true,
                false,
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
            owner,
            Some(1),
        );

        let mut tx = Transaction::new_with_payer(
            &[
                create_account_ins,
                initialize_mint_ins,
                ata_ins,
                min_to_ins,
                create_metadata_account_ins,
                create_master_edition_ins,
            ],
            Some(&owner),
        );

        let blockhash = rpc.get_latest_blockhash().unwrap();

        tx.sign(&[&owner_keypair, &mint], blockhash);
        let tx_resp = rpc.send_and_confirm_transaction(&tx).unwrap();

        Ok(Json("its working".to_string()))
    }
}
