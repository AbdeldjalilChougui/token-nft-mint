//! Mint a fungible SPL token with Metaplex metadata, and mint 1-of-1 NFTs
//! (metadata + master edition), all through CPIs from one Anchor program.
//!
//! * `create_token`: new mint (keypair signer) + metadata account.
//! * `mint_tokens`: mint authority mints to a recipient's ATA (created if needed).
//! * `create_nft`: new 0-decimal mint, mint exactly 1 to the creator, create
//!   metadata and a master edition (max_supply = 0). Creating the master
//!   edition moves mint + freeze authority to the edition PDA, so no more
//!   tokens can ever be minted.
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    metadata::{
        create_master_edition_v3, create_metadata_accounts_v3, mpl_token_metadata::types::DataV2,
        CreateMasterEditionV3, CreateMetadataAccountsV3, Metadata,
    },
    token::{mint_to, Mint, MintTo, Token, TokenAccount},
};

declare_id!("D6x2WuFRf5DYwwo6L74h7cLWx9p3Tss6LaDeuoj6HHTT");

/// Metaplex limits (bytes).
pub const MAX_NAME_LEN: usize = 32;
pub const MAX_SYMBOL_LEN: usize = 10;
pub const MAX_URI_LEN: usize = 200;

#[error_code]
pub enum MintError {
    #[msg("Name longer than 32 bytes")]
    NameTooLong,
    #[msg("Symbol longer than 10 bytes")]
    SymbolTooLong,
    #[msg("URI longer than 200 bytes")]
    UriTooLong,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
}

fn check_meta(name: &str, symbol: &str, uri: &str) -> Result<()> {
    require!(name.len() <= MAX_NAME_LEN, MintError::NameTooLong);
    require!(symbol.len() <= MAX_SYMBOL_LEN, MintError::SymbolTooLong);
    require!(uri.len() <= MAX_URI_LEN, MintError::UriTooLong);
    Ok(())
}

fn data_v2(name: String, symbol: String, uri: String) -> DataV2 {
    DataV2 {
        name,
        symbol,
        uri,
        seller_fee_basis_points: 0,
        creators: None,
        collection: None,
        uses: None,
    }
}

#[program]
pub mod token_nft_mint {
    use super::*;

    pub fn create_token(
        ctx: Context<CreateToken>,
        _decimals: u8,
        name: String,
        symbol: String,
        uri: String,
    ) -> Result<()> {
        check_meta(&name, &symbol, &uri)?;
        let a = &ctx.accounts;
        create_metadata_accounts_v3(
            CpiContext::new(
                a.token_metadata_program.key(),
                CreateMetadataAccountsV3 {
                    metadata: a.metadata.to_account_info(),
                    mint: a.mint.to_account_info(),
                    mint_authority: a.payer.to_account_info(),
                    payer: a.payer.to_account_info(),
                    update_authority: a.payer.to_account_info(),
                    system_program: a.system_program.to_account_info(),
                    rent: a.rent.to_account_info(),
                },
            ),
            data_v2(name, symbol, uri),
            true,
            true,
            None,
        )
    }

    pub fn mint_tokens(ctx: Context<MintTokens>, amount: u64) -> Result<()> {
        require!(amount > 0, MintError::ZeroAmount);
        let a = &ctx.accounts;
        mint_to(
            CpiContext::new(
                a.token_program.key(),
                MintTo {
                    mint: a.mint.to_account_info(),
                    to: a.recipient_ata.to_account_info(),
                    authority: a.authority.to_account_info(),
                },
            ),
            amount,
        )
    }

    pub fn create_nft(ctx: Context<CreateNft>, name: String, symbol: String, uri: String) -> Result<()> {
        check_meta(&name, &symbol, &uri)?;
        let a = &ctx.accounts;
        mint_to(
            CpiContext::new(
                a.token_program.key(),
                MintTo {
                    mint: a.mint.to_account_info(),
                    to: a.payer_ata.to_account_info(),
                    authority: a.payer.to_account_info(),
                },
            ),
            1,
        )?;
        create_metadata_accounts_v3(
            CpiContext::new(
                a.token_metadata_program.key(),
                CreateMetadataAccountsV3 {
                    metadata: a.metadata.to_account_info(),
                    mint: a.mint.to_account_info(),
                    mint_authority: a.payer.to_account_info(),
                    payer: a.payer.to_account_info(),
                    update_authority: a.payer.to_account_info(),
                    system_program: a.system_program.to_account_info(),
                    rent: a.rent.to_account_info(),
                },
            ),
            data_v2(name, symbol, uri),
            true,
            true,
            None,
        )?;
        create_master_edition_v3(
            CpiContext::new(
                a.token_metadata_program.key(),
                CreateMasterEditionV3 {
                    edition: a.master_edition.to_account_info(),
                    mint: a.mint.to_account_info(),
                    update_authority: a.payer.to_account_info(),
                    mint_authority: a.payer.to_account_info(),
                    payer: a.payer.to_account_info(),
                    metadata: a.metadata.to_account_info(),
                    token_program: a.token_program.to_account_info(),
                    system_program: a.system_program.to_account_info(),
                    rent: a.rent.to_account_info(),
                },
            ),
            Some(0),
        )
    }
}

#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct CreateToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        payer = payer,
        mint::decimals = decimals,
        mint::authority = payer,
        mint::freeze_authority = payer
    )]
    pub mint: Account<'info, Mint>,
    /// CHECK: created by the Token Metadata program; address checked by seeds.
    #[account(
        mut,
        seeds = [b"metadata", token_metadata_program.key().as_ref(), mint.key().as_ref()],
        bump,
        seeds::program = token_metadata_program.key()
    )]
    pub metadata: UncheckedAccount<'info>,
    pub token_metadata_program: Program<'info, Metadata>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct MintTokens<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut, mint::authority = authority)]
    pub mint: Account<'info, Mint>,
    /// CHECK: any wallet can receive; only used as the ATA owner.
    pub recipient: UncheckedAccount<'info>,
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = mint,
        associated_token::authority = recipient
    )]
    pub recipient_ata: Account<'info, TokenAccount>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateNft<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        payer = payer,
        mint::decimals = 0,
        mint::authority = payer,
        mint::freeze_authority = payer
    )]
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = payer
    )]
    pub payer_ata: Account<'info, TokenAccount>,
    /// CHECK: created by the Token Metadata program; address checked by seeds.
    #[account(
        mut,
        seeds = [b"metadata", token_metadata_program.key().as_ref(), mint.key().as_ref()],
        bump,
        seeds::program = token_metadata_program.key()
    )]
    pub metadata: UncheckedAccount<'info>,
    /// CHECK: created by the Token Metadata program; address checked by seeds.
    #[account(
        mut,
        seeds = [b"metadata", token_metadata_program.key().as_ref(), mint.key().as_ref(), b"edition"],
        bump,
        seeds::program = token_metadata_program.key()
    )]
    pub master_edition: UncheckedAccount<'info>,
    pub token_metadata_program: Program<'info, Metadata>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}
