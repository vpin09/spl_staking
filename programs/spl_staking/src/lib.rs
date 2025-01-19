use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("5xegvvEypwkoYsvcRN3TcMyVg66ENihYMZu19obgc6t1");

#[program]
pub mod spl_staking {  
    use super::*;

    pub fn initialize_state(ctx: Context<InitializeState>, start_time: i64, end_time: i64, lock_duration: i64, apy: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.owner = *ctx.accounts.owner.key;
        state.start_time = start_time;
        state.end_time = end_time;
        state.lock_duration = lock_duration;
        state.apy = apy;
        state.bump = ctx.bumps.state; 
        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        let clock = Clock::get()?;
        let state = &ctx.accounts.state;
        let user = &mut ctx.accounts.user_stake_account;

        require!(clock.unix_timestamp >= state.start_time, CustomError::StakingNotStarted);
        require!(clock.unix_timestamp <= state.end_time, CustomError::StakingEnded);
        require!(user.amount_staked == 0, CustomError::AlreadyStaked);

        user.owner = *ctx.accounts.user.key;
        user.amount_staked = amount;
        user.start_time = clock.unix_timestamp;
        user.lock_duration = state.lock_duration;
        user.reward_claimed = 0;

        token::transfer(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ), amount)?;

        Ok(())
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        let clock = Clock::get()?;
        let user = &mut ctx.accounts.user_stake_account;
        let state = &ctx.accounts.state;

        let staking_duration = clock.unix_timestamp - user.start_time;
        let rewards = calculate_rewards(user.amount_staked, state.apy, staking_duration);

        require!(rewards > user.reward_claimed, CustomError::NoRewardsAvailable);

        let claimable = rewards - user.reward_claimed;
        user.reward_claimed = rewards;

        let seeds = &[b"state".as_ref(), &[state.bump]];
        let signer = [&seeds[..]];

        token::transfer(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.state.to_account_info(),
            },
            &signer
        ), claimable)?;

        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        let clock = Clock::get()?;
        let user = &mut ctx.accounts.user_stake_account;
        let state = &ctx.accounts.state;

        require!(clock.unix_timestamp >= user.start_time + user.lock_duration, CustomError::LockPeriodNotOver);

        let amount = user.amount_staked;
        user.amount_staked = 0;
        user.reward_claimed = 0;

        let seeds = &[b"state".as_ref(), &[state.bump]];
        let signer = [&seeds[..]];

        token::transfer(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.state.to_account_info(),
            },
            &signer
        ), amount)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeState<'info> {
    #[account(
        init,
        payer = owner,
        seeds = [b"state"],
        bump,
        space = 8 + State::SIZE
    )]
    pub state: Account<'info, State>,

    #[account(
        init,
        payer = owner,
        token::mint = mint,
        token::authority = state,
        seeds = [b"vault"],
        bump
    )]
    pub vault: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub state: Account<'info, State>,
    #[account(
        init_if_needed,
        payer = user,
        seeds = [user.key.as_ref()],
        bump,
        space = 8 + UserStakeAccount::SIZE
    )]
    pub user_stake_account: Account<'info, UserStakeAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub state: Account<'info, State>,
    #[account(
        mut,
        seeds = [user_stake_account.owner.as_ref()],
        bump
    )]
    pub user_stake_account: Account<'info, UserStakeAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut
    )]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub state: Account<'info, State>,
    #[account(
        mut,
        seeds = [user_stake_account.owner.as_ref()],
        bump
    )]
    pub user_stake_account: Account<'info, UserStakeAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct State {
    pub owner: Pubkey,
    pub start_time: i64,
    pub end_time: i64,
    pub lock_duration: i64,
    pub apy: u64,
    pub bump: u8,
}

impl State {
    pub const SIZE: usize = 32 + 8 + 8 + 8 + 8 + 1;
}

#[account]
pub struct UserStakeAccount {
    pub owner: Pubkey,
    pub amount_staked: u64,
    pub start_time: i64,
    pub lock_duration: i64,
    pub reward_claimed: u64,
}

impl UserStakeAccount {
    pub const SIZE: usize = 32 + 8 + 8 + 8 + 8;
}

#[error_code]
pub enum CustomError {
    #[msg("The staking period has not started yet.")]
    StakingNotStarted,
    #[msg("The staking period has ended.")]
    StakingEnded,
    #[msg("The user already has an active stake.")]
    AlreadyStaked,
    #[msg("No rewards are available to claim.")]
    NoRewardsAvailable,
    #[msg("The lock period has not yet ended.")]
    LockPeriodNotOver,
}

fn calculate_rewards(amount: u64, apy: u64, duration: i64) -> u64 {
    let seconds_in_year = 365 * 24 * 60 * 60;
    (amount as u128 * apy as u128 * duration as u128 / seconds_in_year as u128 / 100) as u64
}