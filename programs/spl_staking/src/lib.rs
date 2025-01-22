
use anchor_lang::prelude::*;
use anchor_spl::{token::{self, Mint, Token, TokenAccount, Transfer}};


declare_id!("5kKjqePmcCLnevspkqkRZjxCoprbW57GVVu1jaDa1qit");

#[program]
pub mod spl_staking {  
    use super::*;

    // Initialize the staking program state with basic parameters
    // This function sets up the initial configuration for the staking program
    pub fn initialize_state(ctx: Context<InitializeState>, start_time: i64, end_time: i64, lock_duration: i64, apy: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.owner = *ctx.accounts.owner.key;
        state.start_time = start_time;
        state.end_time = end_time;
        state.lock_duration = lock_duration;
        state.apy = apy;
        state.bump = ctx.bumps.state; 
        state.token_mint = ctx.accounts.mint.key();
        state.vault = ctx.accounts.vault.key();
        state.total_staked = 0;

        // Initial funding of the vault with 1 billion tokens
        token::transfer(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.owner_token_account.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            },
        ), 1_000_000_000)?;
        Ok(())
    }
    
    // Update the staking parameters
    // Allows the owner to modify staking conditions after initialization
    pub fn update_stake(ctx: Context<UpdateStake>, start_time: i64, end_time: i64, lock_duration: i64, apy: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.start_time = start_time;
        state.end_time = end_time;
        state.lock_duration = lock_duration;
        state.apy = apy;

        Ok(())
    }

    // Handle user staking tokens
    // Users can stake their tokens and start earning rewards
    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        let clock = Clock::get()?;
        let state = &ctx.accounts.state;
        let user = &mut ctx.accounts.user_stake_account;

        // Verify staking conditions
        require!(clock.unix_timestamp >= state.start_time, CustomError::StakingNotStarted);
        require!(clock.unix_timestamp <= state.end_time, CustomError::StakingEnded);
        require!(user.amount_staked == 0, CustomError::AlreadyStaked);

        // Transfer tokens from user to vault
        token::transfer(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ), amount)?;

        // Initialize user's staking information
        user.user = *ctx.accounts.user.key;
        user.amount_staked = amount;
        user.start_time = clock.unix_timestamp;
        user.lock_duration = state.lock_duration;
        user.reward_claimed = 0;
        user.apy = state.apy;

        Ok(())
    }

    // Allow users to claim their staking rewards
    // Calculates and transfers earned rewards to the user
    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        let clock = Clock::get()?;
        let user = &mut ctx.accounts.user_stake_account;
        let state = &ctx.accounts.state;

        // Calculate rewards based on staking duration
        let staking_duration = clock.unix_timestamp - user.start_time;
        let rewards = calculate_rewards(user.amount_staked, state.apy, staking_duration);

        require!(rewards > user.reward_claimed, CustomError::NoRewardsAvailable);

        let claimable = rewards - user.reward_claimed;
        
        // Generate PDA signer for vault authority
        let seeds = &[b"state".as_ref(), &[state.bump]];
        let signer = [&seeds[..]];
        
        // Transfer rewards from vault to user
        token::transfer(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.state.to_account_info(),
            },
            &signer
        ), claimable)?;
        
        user.reward_claimed = rewards;

        Ok(())
    }

    // Allow users to unstake their tokens
    // Returns staked tokens after lock period ends
    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        let clock = Clock::get()?;
        let user = &mut ctx.accounts.user_stake_account;
        let state = &ctx.accounts.state;

        // Verify lock period has ended
        require!(clock.unix_timestamp >= user.start_time + user.lock_duration, CustomError::LockPeriodNotOver);

        let amount = user.amount_staked;
        user.amount_staked = 0;
        user.reward_claimed = 0;

        // Generate PDA signer for vault authority
        let seeds = &[b"state".as_ref(), &[state.bump]];
        let signer = [&seeds[..]];

        // Transfer staked tokens back to user
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

// Account struct for program initialization
#[derive(Accounts)]
pub struct InitializeState<'info> {
    // Program state account - stores global configuration
    #[account(
        init,
        payer = owner,
        seeds = [b"state"],
        bump,
        space = 8 + std::mem::size_of::<State>(),
    )]
    pub state: Account<'info, State>,

    // Vault account - holds staked tokens
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
    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// Account struct for updating stake parameters
#[derive(Accounts)]
pub struct UpdateStake<'info> {
    #[account(mut, has_one = owner)]
    pub state: Account<'info, State>,
    pub owner: Signer<'info>,
}

// Account struct for staking tokens
#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub state: Account<'info, State>,
    // User's stake account - stores individual staking info
    #[account(
        init_if_needed,
        payer = user,
        seeds = [user.key.as_ref()],
        bump,
        space = 8 + std::mem::size_of::<UserStakeAccount>()
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

// Account struct for claiming rewards
#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub state: Account<'info, State>,
    #[account(
        mut,
        seeds = [user_stake_account.user.as_ref()],
        bump
    )]
    pub user_stake_account: Account<'info, UserStakeAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

// Account struct for unstaking tokens
#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub state: Account<'info, State>,
    #[account(
        mut,
        seeds = [user_stake_account.user.as_ref()],
        bump
    )]
    pub user_stake_account: Account<'info, UserStakeAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

// State account structure - stores global program configuration
#[account]
#[derive(Default)]
pub struct State {
    pub owner: Pubkey,
    pub token_mint: Pubkey,
    pub vault: Pubkey,
    pub start_time: i64,
    pub end_time: i64,
    pub lock_duration: i64,
    pub apy: u64,
    pub total_staked: u64,
    pub bump: u8,
}

// User stake account structure - stores individual staking information
#[account]
pub struct UserStakeAccount {
    pub user: Pubkey,
    pub amount_staked: u64,
    pub start_time: i64,
    pub lock_duration: i64,
    pub apy: u64,
    pub reward_claimed: u64,
}

impl UserStakeAccount {
    pub const SIZE: usize = 32 + 8 + 8 + 8 + 8 + 8;
}

// Custom error definitions for the program
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

// Helper function to calculate staking rewards
// Returns the reward amount based on stake amount, APY, and duration
fn calculate_rewards(amount: u64, apy: u64, duration: i64) -> u64 {
    let seconds_in_year = 365 * 24 * 60 * 60;
    (amount as u128 * apy as u128 * duration as u128 / seconds_in_year as u128 / 100) as u64
}