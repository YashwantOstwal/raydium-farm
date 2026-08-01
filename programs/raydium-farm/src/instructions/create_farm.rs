use std::ops::Index;

use crate::{states::*,error::ErrorCode};

use anchor_lang::prelude::*;
use anchor_spl::{associated_token::{create, get_associated_token_address_with_program_id, AssociatedToken, Create}, token_interface::{Mint, TokenAccount, TokenInterface,transfer_checked,TransferChecked},token::{Token},token_2022::{Token2022}};


#[derive(Accounts)]
pub struct CreateFarm<'info>{
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        mint::token_program = staking_mint_program
    )]
    pub staking_mint:InterfaceAccount<'info,Mint>,

    pub staking_mint_program:Interface<'info,TokenInterface>,

    #[account(
        mut,
        associated_token::mint = staking_mint,
        associated_token::authority = farm,
        associated_token::token_program = staking_mint_program
    )]
    pub staking_token_vault:InterfaceAccount<'info,TokenAccount>,
    
    #[account(
        init,
        payer = creator,
        space = Farm::LEN,
        seeds = [Farm::STATIC_SEED.as_bytes(),staking_mint.key().as_ref()],
        bump,
    )]
    pub farm:Account<'info,Farm>,

    pub system_program:Program<'info,System>,
    pub associated_token_program:Program<'info,AssociatedToken>,

    // REMAINING ACCOUNTS

    // pub reward_mint_i: InterfaceAccount<Mint>,
    // pub reward_vault_i: InterfaceAccount<TokenAccount>
    // pub creator_reward_token_i: InterfaceAccount<TokenAccount>,

    // i: 0 -> no. of reward streams (upto 5 allowed) - 1
}

#[derive(AnchorSerialize,AnchorDeserialize,Copy,Clone)]
pub struct RewardStreamArgs {
    pub open_time:i64,
    pub end_time:i64,
    pub emission_per_second_x64:u128,
}
pub fn handle_create_farm<'info>(ctx: Context<'info, CreateFarm<'info>>,reward_streams:[Option<RewardStreamArgs>;5])-> Result<()> {
    
    let farm = &mut ctx.accounts.farm;

    let mut reward_streams_count:u8 = 0;
    let clock = Clock::get()?;
    for i in 0..5 {
        if let Some(RewardStreamArgs {open_time,end_time,emission_per_second_x64}) = reward_streams[i] {

            let reward_mint:&mut InterfaceAccount<Mint> = &mut InterfaceAccount::try_from(ctx.remaining_accounts.index(i*3))?;
            let reward_vault:&mut InterfaceAccount<TokenAccount> = &mut InterfaceAccount::try_from(ctx.remaining_accounts.index(i*3 + 1))?;
            let creator_reward_token:InterfaceAccount<TokenAccount> = InterfaceAccount::try_from(ctx.remaining_accounts.index(i*3 + 2))?;

            // validate reward vault is an ATA of reward mint owned by farm.
            let reward_vault_address = get_associated_token_address_with_program_id(&farm.key(), &reward_mint.key(), &reward_mint.to_account_info().owner.key());
            require_keys_eq!(reward_vault.key(),reward_vault_address);

            // validate creator's reward token is of mint reward mint, and owned by the creator.
            require_keys_eq!(creator_reward_token.owner,ctx.accounts.creator.key(),ErrorCode::MismatchingAccounts); 

            require!(open_time > clock.unix_timestamp,ErrorCode::OpenTimeHasToBeInFuture);
            require!(end_time > open_time,ErrorCode::OpenTimeHasToBeInFuture);


            // let create_reward_vault_ctx = CpiContext::new(ctx.accounts.associated_token_program.key(),Create {
            //     payer:ctx.accounts.creator.to_account_info(),
            //     associated_token:reward_vault.to_account_info(),
            //     authority:ctx.accounts.farm.to_account_info(),
            //     mint:reward_mint.to_account_info(),
            //     token_program:reward_mint_program.to_account_info(),
            //     system_program:ctx.accounts.system_program.to_account_info(),
            // });
            
            // create(create_reward_vault_ctx)?;
            let transfer_amount = (end_time.checked_sub(open_time).unwrap() as u128).checked_mul(emission_per_second_x64).unwrap().checked_shr(64).unwrap() as u64;
            
            require!(transfer_amount >= creator_reward_token.amount,ErrorCode::InsufficientBalance);

            let fund_vault_ctx = CpiContext::new(ctx.accounts.staking_mint_program.key(),TransferChecked {
                from:creator_reward_token.to_account_info(),
                to:reward_vault.to_account_info(),
                mint:reward_mint.to_account_info(),
                authority:ctx.accounts.creator.to_account_info()
            });

            transfer_checked(fund_vault_ctx,transfer_amount,reward_mint.decimals)?;

            farm.reward_streams[reward_streams_count as usize] = RewardStream {
                reward_mint:reward_mint.key(),
                status:RewardStreamStatus::Unused,
                open_time,
                end_time,
                emission_per_second_x64,
                // total_rewards_emitted_x64:0,
                acc_rewards_per_base_unit_x64:0,

            };
            reward_streams_count += 1;
        }
    }
    farm.set_inner(Farm {
        creator: ctx.accounts.creator.key(),
        staking_mint: ctx.accounts.staking_mint.key(),
        staking_mint_program:ctx.accounts.staking_mint_program.key(), // used to auto-resolve / validate the staking vault over other ixns.
        last_updated_time: clock.unix_timestamp,
        reward_streams_count,
        reward_streams: farm.reward_streams,
        staked_amount:0,
        bump: ctx.bumps.farm,
    });
    Ok(())
}
