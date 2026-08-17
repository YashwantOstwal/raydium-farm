use crate::{error::ErrorCode, states::*, utils::{ceil_div_x64, div_x64, duration, to_x64}, RewardStreamArgs};

use anchor_lang::prelude::*;
use anchor_spl::{token_interface::{TokenAccount,Mint,TokenInterface,transfer_checked,TransferChecked}};

#[derive(Accounts)]
pub struct RestartRewards<'info> {
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
        mut,
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
}

pub fn handle_restart_rewards(ctx:Context<RestartRewards>,reward_stream_idx:u8,reward_stream:RewardStreamArgs)-> Result<()> {

    let farm = &mut ctx.accounts.farm;
    farm.update()?;

    require!(reward_stream_idx < farm.reward_streams_count,ErrorCode::ReferencedRewardStreamInvalid);

    require!(ctx.accounts.reward_mint.key() == farm.reward_streams[reward_stream_idx as usize].reward_mint,ErrorCode::MismatchingAccounts);
    require!(farm.reward_streams[reward_stream_idx as usize].status == RewardStreamStatus::Ended,ErrorCode::RewardStreamIsRunning);

    let block_timestamp = Clock::get()?.unix_timestamp;
    require!(reward_stream.open_time >= block_timestamp,ErrorCode::OpenTimeCannotBeInPast);

    let total_rewards_x64 = reward_stream.emission_per_second_x64.checked_mul(duration(reward_stream.end_time, reward_stream.open_time).into()).unwrap();

    let rewards_left_x64 = farm.reward_streams[reward_stream_idx as usize].rewards_left_x64;
    let new_rewards_left_x64 = if total_rewards_x64 > rewards_left_x64 {
        let transfer_amount = ceil_div_x64(total_rewards_x64.checked_sub(rewards_left_x64).unwrap());
        require!(transfer_amount <= ctx.accounts.authority_reward_token.amount,ErrorCode::InsufficientBalance);

        let transfer_ixn_ctx = CpiContext::new(ctx.accounts.reward_mint_program.key(), TransferChecked {
            from:ctx.accounts.authority_reward_token.to_account_info(),
            to:ctx.accounts.reward_vault.to_account_info(),
            mint:ctx.accounts.reward_mint.to_account_info(),
            authority:ctx.accounts.authority.to_account_info()
        });
        transfer_checked(transfer_ixn_ctx, transfer_amount, ctx.accounts.reward_mint.decimals)?;
        let transfer_amount_x64 = to_x64(transfer_amount);
        rewards_left_x64.checked_add(transfer_amount_x64).unwrap()
    } else if total_rewards_x64 < rewards_left_x64 {
        let refund_amount = div_x64(rewards_left_x64.checked_sub(total_rewards_x64).unwrap());
        if  refund_amount > 0 {
            let farm_seeds:&[&[u8]] = &[Farm::STATIC_SEED,farm.staking_mint.as_ref(),&[farm.bump]];
            let signer_seeds = [&farm_seeds[..]];

            let transfer_ixn_ctx = CpiContext::new(ctx.accounts.reward_mint_program.key(), TransferChecked {
                to:ctx.accounts.authority_reward_token.to_account_info(),
                from:ctx.accounts.reward_vault.to_account_info(),
                mint:ctx.accounts.reward_mint.to_account_info(),
                authority:farm.to_account_info()
            }).with_signer(&signer_seeds);

            transfer_checked(transfer_ixn_ctx, refund_amount, ctx.accounts.reward_mint.decimals)?;
        }
        let refund_amount_x64 = to_x64(refund_amount);
        rewards_left_x64.checked_sub(refund_amount_x64).unwrap()
    }else {
        rewards_left_x64
    };

    let farm_state = &farm.reward_streams[reward_stream_idx as usize];
    farm.reward_streams[reward_stream_idx as usize] = RewardStream {
        status: if reward_stream.open_time == block_timestamp { RewardStreamStatus::Running } else { RewardStreamStatus::Unused },
        open_time: reward_stream.open_time,
        end_time: reward_stream.end_time,
        rewards_left_x64:new_rewards_left_x64,
        emission_per_second_x64: reward_stream.emission_per_second_x64,

        reward_mint: farm_state.reward_mint,
        reward_mint_program:farm_state.reward_mint_program,
        acc_rewards_per_base_unit_x64: farm_state.acc_rewards_per_base_unit_x64,
    };
    Ok(())
}