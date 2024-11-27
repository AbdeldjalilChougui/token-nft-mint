//! LiteSVM tests. The Metaplex Token Metadata program is loaded from
//! `tests/fixtures/mpl_token_metadata.so` (run `scripts/fetch-programs.sh`
//! once; it is dumped from devnet and not committed).
use anchor_lang::{
    prelude::Pubkey,
    solana_program::{instruction::Instruction, system_program},
    InstructionData, ToAccountMetas,
};
use anchor_spl::metadata::mpl_token_metadata::{
    accounts::{MasterEdition, Metadata},
    ID as MPL_ID,
};
use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use litesvm_token::{
    get_spl_account,
    spl_token::state::{Account as TokenAccount, Mint},
};
use solana_instruction::error::InstructionError;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use solana_transaction_error::TransactionError;
use token_nft_mint::MintError;

const TOKEN: Pubkey = anchor_spl::token::ID;
const RENT_SYSVAR: Pubkey = anchor_lang::prelude::pubkey!("SysvarRent111111111111111111111111111111111");
const ATA_PROGRAM: Pubkey = anchor_spl::associated_token::ID;

fn svm() -> LiteSVM {
    let mut svm = LiteSVM::new();
    let so = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deploy/token_nft_mint.so"));
    svm.add_program(token_nft_mint::id(), so).unwrap();
    let mpl = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/mpl_token_metadata.so");
    let bytes = std::fs::read(mpl)
        .expect("missing tests/fixtures/mpl_token_metadata.so: run scripts/fetch-programs.sh");
    svm.add_program(MPL_ID, &bytes).unwrap();
    svm
}

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) -> Result<(), FailedTransactionMetadata> {
    svm.expire_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &svm.latest_blockhash());
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx).map(|_| ())
}
fn code(e: &FailedTransactionMetadata) -> Option<u32> {
    match e.err {
        TransactionError::InstructionError(_, InstructionError::Custom(c)) => Some(c),
        _ => None,
    }
}
fn ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    anchor_spl::associated_token::get_associated_token_address(owner, mint)
}
fn metadata_pda(mint: &Pubkey) -> Pubkey {
    Metadata::find_pda(mint).0
}
fn edition_pda(mint: &Pubkey) -> Pubkey {
    MasterEdition::find_pda(mint).0
}
fn strip(s: &str) -> &str {
    s.trim_end_matches('\0')
}

fn create_token_ix(payer: &Pubkey, mint: &Pubkey, name: &str, symbol: &str, uri: &str) -> Instruction {
    Instruction::new_with_bytes(
        token_nft_mint::id(),
        &token_nft_mint::instruction::CreateToken {
            _decimals: 6,
            name: name.into(),
            symbol: symbol.into(),
            uri: uri.into(),
        }
        .data(),
        token_nft_mint::accounts::CreateToken {
            payer: *payer,
            mint: *mint,
            metadata: metadata_pda(mint),
            token_metadata_program: MPL_ID,
            token_program: TOKEN,
            system_program: system_program::ID,
            rent: RENT_SYSVAR,
        }
        .to_account_metas(None),
    )
}

fn mint_tokens_ix(authority: &Pubkey, mint: &Pubkey, recipient: &Pubkey, amount: u64) -> Instruction {
    Instruction::new_with_bytes(
        token_nft_mint::id(),
        &token_nft_mint::instruction::MintTokens { amount }.data(),
        token_nft_mint::accounts::MintTokens {
            authority: *authority,
            mint: *mint,
            recipient: *recipient,
            recipient_ata: ata(recipient, mint),
            associated_token_program: ATA_PROGRAM,
            token_program: TOKEN,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn create_nft_ix(payer: &Pubkey, mint: &Pubkey, name: &str) -> Instruction {
    Instruction::new_with_bytes(
        token_nft_mint::id(),
        &token_nft_mint::instruction::CreateNft {
            name: name.into(),
            symbol: "ART".into(),
            uri: "https://example.org/nft/1.json".into(),
        }
        .data(),
        token_nft_mint::accounts::CreateNft {
            payer: *payer,
            mint: *mint,
            payer_ata: ata(payer, mint),
            metadata: metadata_pda(mint),
            master_edition: edition_pda(mint),
            token_metadata_program: MPL_ID,
            associated_token_program: ATA_PROGRAM,
            token_program: TOKEN,
            system_program: system_program::ID,
            rent: RENT_SYSVAR,
        }
        .to_account_metas(None),
    )
}

fn payer(svm: &mut LiteSVM) -> Keypair {
    let k = Keypair::new();
    svm.airdrop(&k.pubkey(), 10_000_000_000).unwrap();
    k
}

#[test]
fn fungible_token_with_metadata() {
    let mut svm = svm();
    let alice = payer(&mut svm);
    let bob = Keypair::new();
    let mint = Keypair::new();
    let ix = create_token_ix(&alice.pubkey(), &mint.pubkey(), "Dinar Test", "DZDT", "https://example.org/dzdt.json");
    send(&mut svm, ix, &[&alice, &mint]).unwrap();

    let md = Metadata::from_bytes(&svm.get_account(&metadata_pda(&mint.pubkey())).unwrap().data).unwrap();
    assert_eq!(strip(&md.name), "Dinar Test");
    assert_eq!(strip(&md.symbol), "DZDT");
    assert_eq!(md.update_authority, alice.pubkey());
    assert_eq!(md.mint, mint.pubkey());

    send(&mut svm, mint_tokens_ix(&alice.pubkey(), &mint.pubkey(), &bob.pubkey(), 2_500_000), &[&alice]).unwrap();
    send(&mut svm, mint_tokens_ix(&alice.pubkey(), &mint.pubkey(), &bob.pubkey(), 500_000), &[&alice]).unwrap();
    let acc = get_spl_account::<TokenAccount>(&svm, &ata(&bob.pubkey(), &mint.pubkey())).unwrap();
    assert_eq!(acc.amount, 3_000_000);
    let m = get_spl_account::<Mint>(&svm, &mint.pubkey()).unwrap();
    assert_eq!((m.decimals, m.supply), (6, 3_000_000));
}

#[test]
fn only_mint_authority_can_mint() {
    let mut svm = svm();
    let alice = payer(&mut svm);
    let mallory = payer(&mut svm);
    let mint = Keypair::new();
    send(&mut svm, create_token_ix(&alice.pubkey(), &mint.pubkey(), "T", "T", ""), &[&alice, &mint]).unwrap();
    let err = send(
        &mut svm,
        mint_tokens_ix(&mallory.pubkey(), &mint.pubkey(), &mallory.pubkey(), 1),
        &[&mallory],
    )
    .unwrap_err();
    // mint::authority = authority -> ConstraintMintMintAuthority (2016)
    assert_eq!(code(&err), Some(2016));
}

#[test]
fn metadata_length_limits() {
    let mut svm = svm();
    let alice = payer(&mut svm);
    let mint = Keypair::new();
    let long = "x".repeat(33);
    let err = send(&mut svm, create_token_ix(&alice.pubkey(), &mint.pubkey(), &long, "T", ""), &[&alice, &mint])
        .unwrap_err();
    assert_eq!(code(&err), Some(6000 + MintError::NameTooLong as u32));
    let err = send(
        &mut svm,
        create_token_ix(&alice.pubkey(), &mint.pubkey(), "ok", "TOOLONGSYMB", ""),
        &[&alice, &mint],
    )
    .unwrap_err();
    assert_eq!(code(&err), Some(6000 + MintError::SymbolTooLong as u32));
}

#[test]
fn nft_is_one_of_one_with_master_edition() {
    let mut svm = svm();
    let alice = payer(&mut svm);
    let mint = Keypair::new();
    send(&mut svm, create_nft_ix(&alice.pubkey(), &mint.pubkey(), "Sahara #1"), &[&alice, &mint]).unwrap();

    let acc = get_spl_account::<TokenAccount>(&svm, &ata(&alice.pubkey(), &mint.pubkey())).unwrap();
    assert_eq!(acc.amount, 1);
    let m = get_spl_account::<Mint>(&svm, &mint.pubkey()).unwrap();
    assert_eq!((m.decimals, m.supply), (0, 1));
    // The master edition took over the mint authority: supply is locked.
    assert_eq!(m.mint_authority, Some(edition_pda(&mint.pubkey())).into());

    let ed = MasterEdition::from_bytes(&svm.get_account(&edition_pda(&mint.pubkey())).unwrap().data).unwrap();
    assert_eq!((ed.supply, ed.max_supply), (0, Some(0)));
    let md = Metadata::from_bytes(&svm.get_account(&metadata_pda(&mint.pubkey())).unwrap().data).unwrap();
    assert_eq!(strip(&md.name), "Sahara #1");

    // Minting more fails: alice is no longer the mint authority.
    let err = send(&mut svm, mint_tokens_ix(&alice.pubkey(), &mint.pubkey(), &alice.pubkey(), 1), &[&alice])
        .unwrap_err();
    assert_eq!(code(&err), Some(2016));
}

#[test]
fn nft_requires_mint_keypair_signature() {
    let mut svm = svm();
    let alice = payer(&mut svm);
    let mint = Keypair::new();
    let mut ix = create_nft_ix(&alice.pubkey(), &mint.pubkey(), "x");
    ix.accounts[1].is_signer = false;
    // `init` on a non-PDA account needs the new account's signature, so the
    // system program's create_account fails.
    assert!(send(&mut svm, ix, &[&alice]).is_err());
}
