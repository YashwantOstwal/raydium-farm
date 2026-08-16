use crate::{error::ErrorCode, states::*, utils::{ceil_div_x64, duration}, RewardStreamArgs};

use anchor_lang::prelude::*;
use anchor_spl::{token_interface::{transfer_checked, Mint, TokenInterface, TokenAccount, TransferChecked},associated_token::{AssociatedToken}};


#[derive(Accounts)]
pub struct AddReward<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    pub staking_mint: Box<InterfaceAccount<'info,Mint>>,

    #[account(
        mut,
        has_one = authority,
        has_one = staking_mint,
        seeds = [Farm::STATIC_SEED.as_ref(), staking_mint.key().as_ref()],
        bump = farm.bump
    )]
    pub farm: Box<Account<'info,Farm>>,

    #[account(
        mint::token_program = reward_mint_program,
    )]
    pub reward_mint: Box<InterfaceAccount<'info,Mint>>,
    
    
    #[account(
        init,
        payer = authority,
        associated_token::authority = farm,
        associated_token::mint = reward_mint,
        associated_token::token_program = reward_mint_program
    )]
    pub reward_vault: Box<InterfaceAccount<'info,TokenAccount>>,

    #[account(
        mut,
        token::authority = authority,
        token::mint = reward_mint,
        token::token_program = reward_mint_program
    )]
    pub authority_reward_token: Box<InterfaceAccount<'info,TokenAccount>>,

    pub reward_mint_program:Interface<'info,TokenInterface>,
    pub system_program:Program<'info,System>,
    pub associated_token_program:Program<'info,AssociatedToken>,
}

pub fn handle_add_reward(ctx:Context<AddReward>,new_reward_stream:RewardStreamArgs) -> Result<()> {

    let farm = &mut ctx.accounts.farm;
    farm.update()?;
    require!(farm.reward_streams_count < 5, ErrorCode::RewardStreamsLimitExceeded);

    let block_timestamp = Clock::get()?.unix_timestamp;
    require!(new_reward_stream.open_time >= block_timestamp,ErrorCode::OpenTimeCannotBeInPast);

    let reward_mint = &ctx.accounts.reward_mint;
    let reward_vault = &ctx.accounts.reward_vault;
    for i in 0..farm.reward_streams_count {
        require_keys_neq!(farm.reward_streams[i as usize].reward_mint,reward_mint.key(),ErrorCode::RewardStreamWithRewardMintAlreadyExist)
    }

    let total_rewards_x64 = new_reward_stream.emission_per_second_x64
        .checked_mul(duration(new_reward_stream.end_time, new_reward_stream.open_time).into())
        .unwrap();

    let required_vault_balance = ceil_div_x64(total_rewards_x64);
    require!(required_vault_balance <= ctx.accounts.authority_reward_token.amount,ErrorCode::InsufficientBalance);
    let transfer_ixn_ctx = CpiContext::new(ctx.accounts.reward_mint_program.key(),TransferChecked {
        from: ctx.accounts.authority_reward_token.to_account_info(),
        to:reward_vault.to_account_info(),
        mint:reward_mint.to_account_info(),
        authority:ctx.accounts.authority.to_account_info()
    });
    transfer_checked(transfer_ixn_ctx,required_vault_balance,reward_mint.decimals)?;

    let new_farm_idx = farm.reward_streams_count as usize;
    farm.reward_streams[new_farm_idx] = RewardStream {
        reward_mint: reward_mint.key(),
        reward_mint_program:reward_mint.to_account_info().owner.key(),
        status: if new_reward_stream.open_time == block_timestamp { RewardStreamStatus::Running } else { RewardStreamStatus::Unused },
        open_time: new_reward_stream.open_time,
        end_time: new_reward_stream.end_time,
        acc_rewards_per_base_unit_x64: 0,
        rewards_left_x64: (required_vault_balance as u128).checked_shl(64).unwrap(),
        emission_per_second_x64: new_reward_stream.emission_per_second_x64,
    };
    farm.reward_streams_count += 1;
    Ok(())
}