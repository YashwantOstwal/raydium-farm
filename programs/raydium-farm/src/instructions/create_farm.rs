use std::ops::Index;

use crate::{error::ErrorCode, states::*, utils::*};

use anchor_lang::prelude::*;
use anchor_spl::{associated_token::{create, get_associated_token_address_with_program_id, AssociatedToken, Create,}, token_interface::{Mint, TokenAccount, TokenInterface,transfer_checked,TransferChecked},token::{Token},token_2022::{Token2022}};


#[derive(Accounts)]
pub struct CreateFarm<'info>{
    #[account(mut)]
    pub creator: Signer<'info>,

    pub staking_mint:InterfaceAccount<'info,Mint>,

    /// CHECK: created in the instruction to reduce redudant account from the context "staking_mint_program", seeds order referred from the official repo of AssociatedToken https://github.com/solana-program/associated-token-account/blob/main/interface/src/address.rs, 
    #[account(
        mut,
        seeds = [farm.key().as_ref(),staking_mint.to_account_info().owner.key().as_ref(),staking_mint.key().as_ref()],
        bump,
        seeds::program = associated_token_program
        
        // associated_token::mint = staking_mint,
        // associated_token::authority = farm,
        // associated_token::token_program = staking_mint.to_account_info().owner
    )]
    pub staking_token_vault:UncheckedAccount<'info>,
    
    #[account(
        init,
        payer = creator,
        space = Farm::LEN,
        seeds = [Farm::STATIC_SEED,staking_mint.key().as_ref()],
        bump,
    )]
    pub farm:Account<'info,Farm>,

    pub system_program:Program<'info,System>,

    // Right token program is used by the appropriate CPI calls.
    pub token_program:Program<'info,Token>,
    pub token_2022_program:Program<'info,Token2022>,

    pub associated_token_program:Program<'info,AssociatedToken>,

    // REMAINING ACCOUNTS
    
    // pub reward_mint_i: InterfaceAccount<Mint>,
    // pub reward_vault_i: SystemAccount (~ account does not exist yet),
    // pub creator_reward_token_i: InterfaceAccount<TokenAccount>,

    // i: 0 -> n - 1 where n = no. of reward streams (upto 5 allowed)
}

#[derive(AnchorSerialize,AnchorDeserialize,Copy,Clone)]
pub struct RewardStreamArgs {
    pub open_time:i64,
    pub end_time:i64,
    pub emission_per_second_x64:u128,
}
pub fn handle_create_farm<'info>(ctx: Context<'info, CreateFarm<'info>>,reward_streams:[Option<RewardStreamArgs>;5])-> Result<()> {
    
    let mut reward_streams_count:u8 = 0;
    let block_timestamp = Clock::get()?.unix_timestamp;

    let create_staking_vault_ctx = CpiContext::new(ctx.accounts.associated_token_program.key(),Create {
        payer:ctx.accounts.creator.to_account_info(),
        associated_token:ctx.accounts.staking_token_vault.to_account_info(),
        authority:ctx.accounts.farm.to_account_info(),
        mint:ctx.accounts.staking_mint.to_account_info(),
        token_program:if ctx.accounts.staking_mint.to_account_info().owner.key() == ctx.accounts.token_program.key() {
            ctx.accounts.token_program.to_account_info()
        }else {
            ctx.accounts.token_2022_program.to_account_info()
        },
        system_program:ctx.accounts.system_program.to_account_info(),
    });

    create(create_staking_vault_ctx)?;
    for i in 0..5 {
        if let Some(RewardStreamArgs {open_time,end_time,emission_per_second_x64}) = reward_streams[i] {

            let reward_mint:&InterfaceAccount<Mint> = &InterfaceAccount::try_from(ctx.remaining_accounts.index(i*3)).unwrap();
            let reward_vault:&SystemAccount = &SystemAccount::try_from(ctx.remaining_accounts.index(i*3 + 1)).unwrap();
            let creator_reward_token:&InterfaceAccount<TokenAccount> = &InterfaceAccount::try_from(ctx.remaining_accounts.index(i*3 + 2)).unwrap();

            // validate reward vault is an ATA of reward mint owned by farm.
            let reward_vault_address = get_associated_token_address_with_program_id(&ctx.accounts.farm.key(), &reward_mint.key(), &reward_mint.to_account_info().owner.key());
            require_keys_eq!(reward_vault.key(),reward_vault_address);

            // validate creator's reward token is of mint reward mint, and owned by the creator.
            require_keys_eq!(creator_reward_token.owner,ctx.accounts.creator.key(),ErrorCode::MismatchingAccounts); 

            require!(open_time >= block_timestamp,ErrorCode::OpenTimeCannotBeInPast);
            require!(end_time > open_time,ErrorCode::OpenTimeCannotBeInPast);

            let create_reward_vault_ctx = CpiContext::new(ctx.accounts.associated_token_program.key(),Create {
                payer:ctx.accounts.creator.to_account_info(),
                associated_token:reward_vault.to_account_info(),
                authority:ctx.accounts.farm.to_account_info(),
                mint:reward_mint.to_account_info(),
                token_program:if reward_mint.to_account_info().owner.key() == ctx.accounts.token_program.key() {
                    ctx.accounts.token_program.to_account_info()
                }else {
                    ctx.accounts.token_2022_program.to_account_info()
                },
                system_program:ctx.accounts.system_program.to_account_info(),
            });
            
            create(create_reward_vault_ctx)?;
            
            let total_rewards_x64 = emission_per_second_x64
                .checked_mul(duration(end_time,open_time).into()).unwrap();

            let required_vault_balance = ceil_div_x64(total_rewards_x64);

            require!(required_vault_balance <= creator_reward_token.amount,ErrorCode::InsufficientBalance);
            let fund_vault_ctx = CpiContext::new(reward_mint.to_account_info().owner.key(),TransferChecked {
                from:creator_reward_token.to_account_info(),
                to:reward_vault.to_account_info(),
                mint:reward_mint.to_account_info(),
                authority:ctx.accounts.creator.to_account_info()
            });
            transfer_checked(fund_vault_ctx,required_vault_balance,reward_mint.decimals)?;


            let farm = &mut ctx.accounts.farm;
            farm.reward_streams[reward_streams_count as usize] = RewardStream {
                reward_mint:reward_mint.key(),
                reward_mint_program:reward_mint.to_account_info().owner.key(),
                status: if open_time == block_timestamp { RewardStreamStatus::Running } else { RewardStreamStatus::Unused },
                open_time,
                end_time,
                emission_per_second_x64,
                rewards_left_x64:to_x64(required_vault_balance),
                acc_rewards_per_base_unit_x64:0,

            };
            reward_streams_count += 1;
        }
    }

    let farm = &mut ctx.accounts.farm;
    farm.set_inner(Farm {
        authority: ctx.accounts.creator.key(),
        staking_mint: ctx.accounts.staking_mint.key(),
        staking_mint_program: ctx.accounts.staking_mint.to_account_info().owner.key(),
        last_updated_time: block_timestamp,
        reward_streams_count,
        reward_streams: farm.reward_streams,
        staked_amount:0,
        bump: ctx.bumps.farm,
    });
    Ok(())
}
