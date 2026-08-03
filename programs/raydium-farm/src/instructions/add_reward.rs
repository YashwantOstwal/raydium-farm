use crate::{states::*,RewardStreamArgs,error::ErrorCode};

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenInterface, TokenAccount, TransferChecked};


#[derive(Accounts)]
pub struct AddReward<'info> {
    pub authority: Signer<'info>,

    pub staking_mint: InterfaceAccount<'info,Mint>,

    #[account(
        mut,
        has_one = authority,
        has_one = staking_mint,
        seeds = [Farm::STATIC_SEED.as_ref(), staking_mint.key().as_ref()],
        bump = farm.bump
    )]
    pub farm: Account<'info,Farm>,

    #[account(
        mint::token_program = reward_mint_program,
    )]
    pub reward_mint: InterfaceAccount<'info,Mint>,
    
    
    #[account(
        mut,
        associated_token::authority = farm,
        associated_token::mint = reward_mint,
        associated_token::token_program = reward_mint_program
    )]
    pub reward_vault: InterfaceAccount<'info,TokenAccount>,

    #[account(
        mut,
        token::authority = authority,
        token::mint = reward_mint,
        token::token_program = reward_mint_program
    )]
    pub authority_reward_token: InterfaceAccount<'info,TokenAccount>,

    pub reward_mint_program:Interface<'info,TokenInterface>,
}

pub fn handle_add_reward(ctx:Context<AddReward>,new_reward_stream:RewardStreamArgs) -> Result<()> {

    let farm = &mut ctx.accounts.farm;
    farm.update()?;
    require!(farm.reward_streams_count < 5, ErrorCode::RewardStreamsLimitExceeded);

    let block_timestamp = Clock::get()?.unix_timestamp;
    require!(new_reward_stream.open_time > block_timestamp,ErrorCode::OpenTimeHasToBeInFuture);

    let total_reward_amount = (new_reward_stream.end_time.checked_sub(new_reward_stream.open_time).unwrap() as u128).checked_mul(new_reward_stream.emission_per_second_x64).unwrap().checked_shr(64).unwrap() as u64;

    let transfer_amount = total_reward_amount.checked_sub(ctx.accounts.reward_vault.amount).unwrap();
    if transfer_amount > 0 {
        require!(transfer_amount <= ctx.accounts.authority_reward_token.amount,ErrorCode::InsufficientBalance);

        let fund_vault_ctx = CpiContext::new(ctx.accounts.reward_mint_program.key(), TransferChecked {
            from:ctx.accounts.authority_reward_token.to_account_info(),
            to:ctx.accounts.reward_vault.to_account_info(),
            mint:ctx.accounts.reward_mint.to_account_info(),
            authority:ctx.accounts.authority.to_account_info()
        });
        transfer_checked(fund_vault_ctx, transfer_amount, ctx.accounts.reward_mint.decimals)?;
    }
    let new_farm_idx = farm.reward_streams_count as usize;
    farm.reward_streams[new_farm_idx] = RewardStream {
        reward_mint: ctx.accounts.reward_mint.key(),
        status: RewardStreamStatus::Unused,
        open_time: new_reward_stream.open_time,
        end_time: new_reward_stream.end_time,
        acc_rewards_per_base_unit_x64: 0,
        emission_per_second_x64: new_reward_stream.emission_per_second_x64,
    };
    farm.reward_streams_count += 1;
    Ok(())
}