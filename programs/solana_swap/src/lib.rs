use anchor_lang::prelude::*;
use anchor_lang::solana_program::{self, system_instruction};
use anchor_spl::token::{self, Token, TokenAccount};
use raydium_amm_v2::state::{Pool, PoolState, TickArray};
use raydium_amm_v2::instruction::swap;

declare_id!("8s6z79N61q9q8WoMnMzZcHzSvoX5njMrHV8U3iaojsUB");

#[program]
pub mod sol_token_manager {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, admin: Pubkey) -> Result<()> {
        let (program_vault, program_vault_bump) = Pubkey::find_program_address(
            &[b"program_vault"],
            ctx.program_id,
        );
    
        ctx.accounts.program_state.admin = admin;
        ctx.accounts.program_state.program_vault = program_vault;
        ctx.accounts.program_state.program_vault_bump = program_vault_bump;
    
        msg!("Program initialized. Admin: {}, Vault: {}, Bump: {}", 
            admin, program_vault, program_vault_bump);
            
        Ok(())
    }

    pub fn deposit_sol(ctx: Context<DepositSol>, amount: u64) -> Result<()> {
        let ix = system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.program_vault.key(),
            amount,
        );
        solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.user.to_account_info(),
                ctx.accounts.program_vault.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        msg!("Deposited {} SOL to program vault", amount);
        Ok(())
    }

    pub fn withdraw_sol(ctx: Context<WithdrawSol>, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.user.key() == ctx.accounts.program_state.admin,
            SolTokenManagerError::Unauthorized
        );

        let seeds = &[
            b"program_vault".as_ref(),
            &[ctx.accounts.program_state.program_vault_bump],
        ];
        let signer = &[&seeds[..]];

        solana_program::program::invoke_signed(
            &system_instruction::transfer(
                &ctx.accounts.program_vault.key(),
                &ctx.accounts.user.key(),
                amount,
            ),
            &[
                ctx.accounts.program_vault.to_account_info(),
                ctx.accounts.user.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            signer,
        )?;

        msg!("Withdrawn {} SOL from program vault", amount);
        Ok(())
    }

    pub fn buy_tokens(
        ctx: Context<BuyTokens>,
        amount_in: u64,
        minimum_amount_out: u64,
    ) -> Result<()> {
        let seeds = &[
            b"program_vault".as_ref(),
            &[ctx.accounts.program_state.program_vault_bump],
        ];
        let signer = &[&seeds[..]];

        solana_program::program::invoke_signed(
            &system_instruction::transfer(
                &ctx.accounts.program_vault.key(),
                &ctx.accounts.wsol_vault.key(),
                amount_in,
            ),
            &[
                ctx.accounts.program_vault.to_account_info(),
                ctx.accounts.wsol_vault.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            signer,
        )?;

        let cpi_accounts = raydium_amm_v3::cpi::accounts::Swap {
            pool: ctx.accounts.pool.to_account_info(),
            pool_state: ctx.accounts.pool_state.to_account_info(),
            input_token_account: ctx.accounts.wsol_vault.to_account_info(),
            output_token_account: ctx.accounts.usdc_vault.to_account_info(),
            input_vault: ctx.accounts.wsol_vault.to_account_info(),
            output_vault: ctx.accounts.usdc_vault.to_account_info(),
            tick_array_0: ctx.accounts.tick_array_0.to_account_info(),
            tick_array_1: ctx.accounts.tick_array_1.to_account_info(),
            tick_array_2: ctx.accounts.tick_array_2.to_account_info(),
            oracle: ctx.accounts.oracle.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(
            ctx.accounts.raydium_program.to_account_info(),
            cpi_accounts,
        );

        raydium_amm_v2::cpi::swap(
            cpi_ctx,
            amount_in,
            minimum_amount_out,
            0,
            true,
            true,
        )?;

        let transfer_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.usdc_vault.to_account_info(),
                to: ctx.accounts.user_usdc.to_account_info(),
                authority: ctx.accounts.program_vault.to_account_info(),
            },
        );

        token::transfer(
            transfer_ctx.with_signer(signer),
            ctx.accounts.usdc_vault.amount,
        )?;

        msg!("Bought USDC tokens for {} SOL", amount_in);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user, space = 8 + 32 + 32 + 1)]
    pub program_state: Account<'info, ProgramState>, 
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BuyTokens<'info> {
    #[account(mut)]
    pub program_state: Account<'info, ProgramState>,
    /// CHECK: This account is the program's vault and should be managed by the program.
    #[account(
        mut,
        seeds = [b"program_vault"],
        bump = program_state.program_vault_bump,
    )]
    pub program_vault: AccountInfo<'info>,
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub wsol_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub usdc_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub tick_array_0: Account<'info, TickArray>,
    #[account(mut)]
    pub tick_array_1: Account<'info, TickArray>,
    #[account(mut)]
    pub tick_array_2: Account<'info, TickArray>,
    /// CHECK: This is the oracle account
    #[account(mut)]
    pub oracle: AccountInfo<'info>,
    #[account(mut)]
    pub user_usdc: Account<'info, TokenAccount>,
    pub raydium_program: Program<'info, raydium_amm_v3::program::RaydiumAmmV3>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct ProgramState {
    pub admin: Pubkey,
    pub program_vault: Pubkey,
    pub program_vault_bump: u8,
}

#[derive(Accounts)]
pub struct DepositSol<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: This account is the program's vault and should be managed by the program.
    #[account(mut)]
    pub program_vault: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawSol<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub program_state: Account<'info, ProgramState>,
    /// CHECK: This account is the program's vault and should be managed by the program.
    #[account(
        mut,
        seeds = [b"program_vault"],
        bump = program_state.program_vault_bump,
    )]
    pub program_vault: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum SolTokenManagerError {
    #[msg("You are not authorized to perform this action")]
    Unauthorized,
}