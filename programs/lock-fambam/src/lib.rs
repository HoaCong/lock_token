use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("HQZKovpR4kRKQkpgoxaEh3nKsRk4SBVb1JKcfSyXijQv");

#[program]
pub mod token_lock {
    use super::*;

    // Initialize the token lock program with the admin
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let admin_settings = &mut ctx.accounts.admin_settings;
        admin_settings.admin = ctx.accounts.admin.key();
        admin_settings.default_lock_duration = 86400; // Default to 1 day (in seconds)
        admin_settings.bump = ctx.bumps.admin_settings;

        msg!("Admin settings initialized");
        Ok(())
    }

    // Add a new token to the supported tokens list
    pub fn add_supported_token(ctx: Context<AddSupportedToken>) -> Result<()> {
        let supported_token = &mut ctx.accounts.supported_token;
        supported_token.mint = ctx.accounts.mint.key();
        supported_token.bump = ctx.bumps.supported_token;

        msg!("Token added to supported tokens: {}", supported_token.mint);
        Ok(())
    }

    // Set lock duration (in seconds)
    pub fn set_lock_duration(ctx: Context<SetLockDuration>, duration: u64) -> Result<()> {
        require!(duration > 0, TokenLockError::InvalidDuration);

        let admin_settings = &mut ctx.accounts.admin_settings;
        admin_settings.default_lock_duration = duration;

        msg!("Lock duration set to {} seconds", duration);
        Ok(())
    }

    // Lock tokens
    pub fn lock_tokens(ctx: Context<LockTokens>, amount: u64) -> Result<()> {
        require!(amount > 0, TokenLockError::InvalidAmount);

        let user_lock = &mut ctx.accounts.user_lock;
        let admin_settings = &ctx.accounts.admin_settings;
        let clock = Clock::get()?;

        // Transfer tokens from user account to lock account
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.lock_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };

        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        // Update user lock data
        if user_lock.lock_amount == 0 {
            // First time locking
            user_lock.user = ctx.accounts.user.key();
            user_lock.mint = ctx.accounts.mint.key();
            user_lock.lock_start_time = clock.unix_timestamp as u64;
            user_lock.lock_end_time =
                clock.unix_timestamp as u64 + admin_settings.default_lock_duration;
            user_lock.lock_amount = amount;
            user_lock.bump = ctx.bumps.user_lock;
            user_lock.lock_token_account = ctx.accounts.lock_token_account.key();
        } else {
            // Adding to existing lock, extend lock time from now
            user_lock.lock_amount = user_lock
                .lock_amount
                .checked_add(amount)
                .ok_or(TokenLockError::Overflow)?;
            user_lock.lock_start_time = clock.unix_timestamp as u64;
            user_lock.lock_end_time =
                clock.unix_timestamp as u64 + admin_settings.default_lock_duration;
        }

        msg!(
            "Locked {} tokens until timestamp {}",
            amount,
            user_lock.lock_end_time
        );
        Ok(())
    }

    // Unlock tokens when lock period is over
    pub fn unlock_tokens(ctx: Context<UnlockTokens>) -> Result<()> {
        let clock = Clock::get()?;
        let user_lock_info = ctx.accounts.user_lock.to_account_info();

        // Check conditions before modifying state
        require!(
            ctx.accounts.user_lock.lock_amount > 0,
            TokenLockError::NoLockedTokens
        );
        require!(
            clock.unix_timestamp as u64 >= ctx.accounts.user_lock.lock_end_time,
            TokenLockError::LockPeriodNotOver
        );

        let amount = ctx.accounts.user_lock.lock_amount;

        // PDA seeds for the authority of the lock token account
        let seeds = &[
            b"user_lock",
            ctx.accounts.user_lock.user.as_ref(),
            ctx.accounts.user_lock.mint.as_ref(),
            &[ctx.accounts.user_lock.bump],
        ];
        let signer = &[&seeds[..]];

        // Transfer tokens from lock account back to user
        let cpi_accounts = Transfer {
            from: ctx.accounts.lock_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: user_lock_info,
        };

        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, amount)?;

        // Reset lock data after the transfer
        let user_lock = &mut ctx.accounts.user_lock;
        user_lock.lock_amount = 0;
        user_lock.lock_end_time = 0;

        msg!("Unlocked {} tokens", amount);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 8 + 1,
        seeds = [b"admin_settings"],
        bump
    )]
    pub admin_settings: Account<'info, AdminSettings>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddSupportedToken<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 1,
        seeds = [b"supported_token", mint.key().as_ref()],
        bump
    )]
    pub supported_token: Account<'info, SupportedToken>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        has_one = admin
    )]
    pub admin_settings: Account<'info, AdminSettings>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetLockDuration<'info> {
    #[account(
        mut,
        has_one = admin
    )]
    pub admin_settings: Account<'info, AdminSettings>,

    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct LockTokens<'info> {
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 32 + 32 + 8 + 8 + 8 + 1 + 32,
        seeds = [b"user_lock", user.key().as_ref(), mint.key().as_ref()],
        bump
    )]
    pub user_lock: Account<'info, UserLock>,

    #[account(
        seeds = [b"supported_token", mint.key().as_ref()],
        bump = supported_token.bump
    )]
    pub supported_token: Account<'info, SupportedToken>,

    pub admin_settings: Account<'info, AdminSettings>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = user_token_account.mint == mint.key(),
        constraint = user_token_account.owner == user.key()
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = user,
        token::mint = mint,
        token::authority = user_lock,
    )]
    pub lock_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UnlockTokens<'info> {
    #[account(
        mut,
        seeds = [b"user_lock", user.key().as_ref(), mint.key().as_ref()],
        bump = user_lock.bump,
    )]
    pub user_lock: Account<'info, UserLock>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = lock_token_account.mint == mint.key(),
        constraint = lock_token_account.owner == user_lock.key(),
        constraint = lock_token_account.key() == user_lock.lock_token_account
    )]
    pub lock_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_token_account.mint == mint.key(),
        constraint = user_token_account.owner == user.key()
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct AdminSettings {
    pub admin: Pubkey,              // Admin who can add tokens and set duration
    pub default_lock_duration: u64, // Default lock duration in seconds
    pub bump: u8,                   // PDA bump
}

#[account]
pub struct SupportedToken {
    pub mint: Pubkey, // Token mint address
    pub bump: u8,     // PDA bump
}

#[account]
pub struct UserLock {
    pub user: Pubkey,               // User address
    pub mint: Pubkey,               // Token mint address
    pub lock_start_time: u64,       // When the tokens were locked (timestamp)
    pub lock_end_time: u64,         // When tokens can be unlocked (timestamp)
    pub lock_amount: u64,           // Amount of tokens locked
    pub bump: u8,                   // PDA bump
    pub lock_token_account: Pubkey, // Token account holding the locked tokens
}

#[error_code]
pub enum TokenLockError {
    #[msg("Invalid lock duration")]
    InvalidDuration,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("No tokens locked")]
    NoLockedTokens,
    #[msg("Lock period not over yet")]
    LockPeriodNotOver,
    #[msg("Arithmetic overflow")]
    Overflow,
}