// lib.rs

use anchor_lang::prelude::*;

use anchor_spl::{
    associated_token::AssociatedToken,
    solana_program::{
        system_instruction, program::invoke, program_error::ProgramError, pubkey::Pubkey,
    },
    spl_token::{self},
};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_pack::Pack,
    sysvar::{rent::Rent, Sysvar},
};
use std::convert::TryInto;

declare_id!("5qhgwxdX4NvWzm1F4ujnarQPVZ1uM9EH8jY9ypDmhMH5");

#[program]
pub mod nft_lock_and_swap_program {
    use super::*;

    // Import nft_lock_program and nft_swap_program modules
    use crate::nft_lock_program::*;
    use crate::nft_swap_program::*;

    #[state]
    pub struct NFTCollection {
        pub metadata: String,
        pub image: Vec<u8>,
        pub owner: Pubkey,
        pub locked: bool,
        pub lock_fee_account: Pubkey,
        pub lock_fee_amount: u64,
        pub protocol_fee_account: Pubkey,
        pub protocol_fee_amount: u64,
        pub nft_mint: Pubkey,
        pub sol_amount: u64,
    }

    #[derive(Accounts)]
    pub struct Initialize<'info> {
        #[account(init, payer = user, space = 8 + 1920)]
        pub nft_collection: Account<'info, NFTCollection>,
        #[account(mut)]
        pub user: Signer<'info>,
        pub rent: Sysvar<'info, Rent>,
        pub system_program: Program<'info, System>,
        pub rent_to: AccountInfo<'info>,
        #[account(
            mut,
            associated_token::mint = sol_mint,
            associated_token::authority = user,
        )]
        pub sol_account: AccountInfo<'info>,
        pub associated_token_program: Program<'info, AssociatedToken>,
        pub token_program: Program<'info, Token>,
    }

    #[derive(Accounts)]
    pub struct LockNFT<'info> {
        #[account(mut)]
        pub nft_collection: Account<'info, NFTCollection>,
        #[account(mut)]
        pub user: Signer<'info>,
        pub rent: Sysvar<'info, Rent>,
        pub system_program: Program<'info, System>,
        pub rent_to: AccountInfo<'info>,
        #[account(
            mut,
            associated_token::mint = sol_mint,
            associated_token::authority = user,
        )]
        pub sol_account: AccountInfo<'info>,
        pub associated_token_program: Program<'info, AssociatedToken>,
        pub token_program: Program<'info, Token>,
    }

    #[derive(Accounts)]
    pub struct UnlockNFT<'info> {
        #[account(mut)]
        pub nft_collection: Account<'info, NFTCollection>,
        #[account(mut)]
        pub user: Signer<'info>,
        pub rent: Sysvar<'info, Rent>,
        pub system_program: Program<'info, System>,
        pub rent_to: AccountInfo<'info>,
        #[account(
            mut,
            associated_token::mint = sol_mint,
            associated_token::authority = user,
        )]
        pub sol_account: AccountInfo<'info>,
        pub associated_token_program: Program<'info, AssociatedToken>,
        pub token_program: Program<'info, Token>,
    }

    #[derive(Accounts)]
    pub struct ExecuteSwap<'info> {
        #[account(mut)]
        pub swap: Account<'info, Swap>,
        #[account(mut, associated_token::mint = nft_mint, associated_token::authority = user)]
        pub nft_account: AccountInfo<'info>,
        #[account(mut, associated_token::mint = sol_mint, associated_token::authority = user)]
        pub sol_account: AccountInfo<'info>,
        #[account(mut, associated_token::mint = sol_mint, associated_token::authority = authority)]
        pub sol_receiver_account: AccountInfo<'info>,
        #[account(mut)]
        pub user: Signer<'info>,
        pub associated_token_program: Program<'info, AssociatedToken>,
        pub token_program: Program<'info, Token>,
    }

    impl<'info> nft_lock_and_swap_program::Initialize<'info> {
        pub fn initialize(
            &mut self,
            metadata: String,
            image: Vec<u8>,
            lock_fee_amount: u64,
            protocol_fee_account: Pubkey,
            protocol_fee_amount: u64,
            nft_mint: Pubkey,
            sol_amount: u64,
        ) -> ProgramResult {
            let metadata_len = metadata.len() as u64;
            let image_len = image.len() as u64;
            assert!(metadata_len <= 1920, "Metadata 1920 baytı aşamaz.");
            assert!(image_len <= 8 * 1024, "Resim 8KB'ı aşamaz.");

            self.nft_collection.metadata = metadata.clone();
            self.nft_collection.image = image.clone();
            self.nft_collection.owner = *self.user.key;
            self.nft_collection.locked = false;
            self.nft_collection.lock_fee_account = Pubkey::default();
            self.nft_collection.lock_fee_amount = lock_fee_amount;
            self.nft_collection.protocol_fee_account = protocol_fee_account;
            self.nft_collection.protocol_fee_amount = protocol_fee_amount;
            self.nft_collection.nft_mint = nft_mint;
            self.nft_collection.sol_amount = sol_amount;

            Ok(())
        }
    }

    impl<'info> nft_lock_and_swap_program::LockNFT<'info> {
        pub fn lock_nft(
            &mut self,
            lock_fee_receiver: AccountInfo<'info>,
        ) -> ProgramResult {
            // Kilit ücretini alıcıya gönder
            let cpi_accounts = AssociatedToken::Transfer {
                from: self.sol_account.clone(),
                to: lock_fee_receiver.clone(),
                authority: self.user.clone(),
            };
            let cpi_program = self.associated_token_program.clone();
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
            cpi_ctx.transfer(self.nft_collection.lock_fee_amount)?;

            // Protokol kirasını protokol hesabına gönder
            let cpi_accounts_protocol = AssociatedToken::Transfer {
                from: self.sol_account.clone(),
                to: self.nft_collection.protocol_fee_account.clone(),
                authority: self.user.clone(),
            };
            let cpi_ctx_protocol = CpiContext::new(self.associated_token_program.clone(), cpi_accounts_protocol);
            cpi_ctx_protocol.transfer(self.nft_collection.protocol_fee_amount)?;

            // NFT'yi kilitle
            self.nft_collection.locked = true;
            self.nft_collection.lock_fee_account = lock_fee_receiver.key().clone();

            Ok(())
        }
    }

    impl<'info> nft_lock_and_swap_program::UnlockNFT<'info> {
        pub fn unlock_nft(
            &mut self,
            unlock_fee_receiver: AccountInfo<'info>,
        ) -> ProgramResult {
            // Kilidi açma ücretini alıcıya gönder
            let cpi_accounts = AssociatedToken::Transfer {
                from: self.sol_account.clone(),
                to: unlock_fee_receiver.clone(),
                authority: self.user.clone(),
            };
            let cpi_program = self.associated_token_program.clone();
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
            cpi_ctx.transfer(self.nft_collection.lock_fee_amount)?;

            // Protokol kirasını protokol hesabına geri gönder
            let cpi_accounts_protocol = AssociatedToken::Transfer {
                from: self.sol_account.clone(),
                to: self.nft_collection.protocol_fee_account.clone(),
                authority: self.user.clone(),
            };
            let cpi_ctx_protocol = CpiContext::new(self.associated_token_program.clone(), cpi_accounts_protocol);
            cpi_ctx_protocol.transfer(self.nft_collection.protocol_fee_amount)?;

            // NFT'nin kilidini kaldır
            self.nft_collection.locked = false;
            self.nft_collection.lock_fee_account = Pubkey::default();

            Ok(())
        }
    }

    impl<'info> nft_lock_and_swap_program::ExecuteSwap<'info> {
        pub fn execute(
            &mut self,
            nft_mint: Pubkey,
            sol_amount: u64,
        ) -> ProgramResult {
            // Swap'ın NFT mint'inin beklenen mint ile eşleşip eşleşmediğini kontrol edin
            if *self.swap.nft_account.key != nft_mint {
                return Err(ProgramError::InvalidArgument);
            }

            // Kullanıcının yeterli SOL bakiyesine sahip olup olmadığını kontrol edin
            if **self.sol_account.lamports.borrow() < sol_amount {
                return Err(ProgramError::InsufficientFunds);
            }

            // Kullanıcının SOL bakiyesini azaltın
            **self.sol_account.lamports.borrow_mut() -= sol_amount;

            // SOL'u alıcısına transfer edin
            **self.sol_receiver_account.lamports.borrow_mut() += sol_amount;

            // NFT'yi kullanıcıya transfer edin
            let cpi_accounts = spl_token::instruction::transfer(
                self.token_program.clone(),
                self.nft_account.clone(),
                self.sol_account.clone(),
                self.sol_receiver_account.clone(),
                &[],
                sol_amount.try_into().unwrap(),
            )?;
            let cpi_ctx = CpiContext::new(self.token_program.clone(), cpi_accounts);
            cpi_ctx.invoke()?;

            Ok(())
        }
    }
}
