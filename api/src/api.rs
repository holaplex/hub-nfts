use hub_core::uuid::Uuid;
use poem::{error::InternalServerError, web::Data, Result};
use poem_openapi::{param::Header, payload::Json, Object, OpenApi};
use solana_program::program_pack::Pack;
use solana_sdk::signer::{keypair::Keypair, Signer};
use spl_associated_token_account::get_associated_token_address;

use crate::{
    proto::{
        self,
        drop_events::{self},
        DropEventKey,
    },
    AppState, DropEvents,
};

pub struct NftApi;

#[OpenApi]
impl NftApi {
    #[oai(path = "/create", method = "post")]
    async fn create(
        &self,
        state: Data<&AppState>,
        #[oai(name = "X-USER-ID")] user_id: Header<Uuid>,
        #[oai(name = "X-ORGANIZATION-ID")] organization: Header<Uuid>,
        input: Json<CreateEditionInput>,
    ) -> Result<Json<String>> {
        let Data(state) = state;
        let Header(organization) = organization;
        let Header(user_id) = user_id;
        let producer = state.producer.clone();
        let rpc = &*state.rpc;

        let owner = input.owner_address.parse().unwrap();

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

        let blockhash = rpc.get_latest_blockhash().unwrap();

        let message = solana_program::message::Message::new_with_blockhash(
            &[
                create_account_ins,
                initialize_mint_ins,
                ata_ins,
                min_to_ins,
                create_metadata_account_ins,
                create_master_edition_ins,
            ],
            Some(&owner),
            &blockhash,
        );

        let serialized_message = message.serialize();
        let signature = mint.try_sign_message(&message.serialize()).unwrap();
        let hashed_message = hex::encode(message.serialize());

        // payload includes serialized_message, signature, hashed_message and project_id

        let event = DropEvents {
            event: Some(drop_events::Event::MintEditionTransaction(
                proto::Transaction {
                    serialized_message,
                    signed_message_signature: signature.to_string(),
                    hashed_message: hashed_message.to_string(),
                    project_id: input.project_id.to_string(),
                    organization_id: organization.to_string(),
                    blockhash: blockhash.to_string(),
                },
            )),
        };

        let key = DropEventKey {
            id: signature.to_string(),
            user_id: user_id.to_string(),
        };

        producer
            .send(Some(&event), Some(&key))
            .await
            .map_err(InternalServerError)?;

        Ok(Json("its working".to_string()))
    }
}

#[derive(Debug, Clone, Object)]
pub struct CreateEditionInput {
    owner_address: String,
    project_id: Uuid,
}
