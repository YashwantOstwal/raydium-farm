use std::ops::Index;

use crate::{states::*,error::ErrorCode};

use anchor_lang::prelude::*;
use anchor_spl::{ token_interface::{Mint,TokenAccount,TokenInterface,transfer_checked,TransferChecked},associated_token::{get_associated_token_address_with_program_id}};

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mint::token_program = staking_mint_program,
    )]
    pub staking_mint: Box<InterfaceAccount<'info,Mint>>,

    #[account(
        mut,
        seeds = [Farm::STATIC_SEED,staking_mint.key().as_ref()],
        bump = farm.bump
    )]
    pub farm: Box<Account<'info,Farm>>,

    pub staking_mint_program:Interface<'info,TokenInterface>,

    #[account(
        mut,
        token::mint = staking_mint,
        token::authority = user,
        token::token_program = staking_mint_program,
    )]
    pub user_staking_token: Box<InterfaceAccount<'info,TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = staking_mint,
        associated_token::authority = farm,
        associated_token::token_program = staking_mint_program,
    )]
    pub staking_token_vault: Box<InterfaceAccount<'info,TokenAccount>>,

    #[account(
        mut,
        seeds = [UserLedger::STATIC_SEED,farm.key().as_ref(),user.key().as_ref()],
        bump = user_ledger.bump
    )]
    pub user_ledger:Box<Account<'info,UserLedger>>,

    // REMAINING ACCOUNTS 

    // reward_mint_i: InterfaceAccount<Mint>,
    // reward_vault_i:InterfaceAccount<TokenAccount>
    // user_reward_token_i: InterfaceAccount<TokenAccount>,

    // i: 0 -> n - 1 where n = reward_streams_count
}

pub fn handle_withdraw<'info>(ctx:Context<'info,Withdraw<'info>>,withdraw_amount:u64)-> Result<()> {

    let user_ledger = &mut ctx.accounts.user_ledger;
    require!(user_ledger.staked_amount >= withdraw_amount,ErrorCode::InsufficientBalance);
    
    let farm = &mut ctx.accounts.farm;
    farm.update()?;
    
    user_ledger.update(farm)?;

    let farm_seeds:&[&[u8]] = &[Farm::STATIC_SEED,farm.staking_mint.as_ref(),&[farm.bump]];
    let signer_seeds = [&farm_seeds[..]];
    // Reward stakers.
    for i in  0..farm.reward_streams_count {
        let reward_mint:InterfaceAccount<Mint> = InterfaceAccount::try_from(ctx.remaining_accounts.index((i*3) as usize)).unwrap();
        let reward_vault:InterfaceAccount<TokenAccount> = InterfaceAccount::try_from(ctx.remaining_accounts.index((i*3 + 1) as usize)).unwrap();
        let user_reward_token:InterfaceAccount<TokenAccount> = InterfaceAccount::try_from(ctx.remaining_accounts.index((i*3 + 2) as usize)).unwrap();

        // validate the user reward token.
        require_keys_eq!(user_reward_token.mint,farm.reward_streams[i as usize].reward_mint,ErrorCode::MismatchingAccounts);
        require_keys_eq!(user_reward_token.owner,ctx.accounts.user.key(),ErrorCode::MismatchingAccounts);

        // validate reward vault is an ATA of reward mint owned by farm.
        let reward_vault_address = get_associated_token_address_with_program_id(&farm.key(), &reward_mint.key(), &reward_mint.to_account_info().owner.key());
        require_keys_eq!(reward_vault.key(),reward_vault_address);

        // validate the reward mint.
        require_keys_eq!(reward_mint.key(),farm.reward_streams[i as usize].reward_mint,ErrorCode::MismatchingAccounts);

        let transfer_amount = user_ledger.reward_infos[i as usize].pending_rewards_x64.checked_shr(64).unwrap() as u64;
        if transfer_amount > 0 {
            let transfer_ctx = CpiContext::new(reward_mint.to_account_info().owner.key(), TransferChecked {
                from:reward_vault.to_account_info(),
                to:user_reward_token.to_account_info(),
                mint: reward_mint.to_account_info(),
                authority:farm.to_account_info()
            }).with_signer(&signer_seeds);

            transfer_checked(transfer_ctx,transfer_amount,reward_mint.decimals)?;
        }

        user_ledger.reward_infos[i as usize].pending_rewards_x64 = user_ledger.reward_infos[i as usize].pending_rewards_x64.checked_sub(transfer_amount.checked_shl(64).unwrap() as u128).unwrap();
    }

    // Withdraw
    let transfer_ctx = CpiContext::new(ctx.accounts.staking_mint_program.key(), TransferChecked {
        from:ctx.accounts.staking_token_vault.to_account_info(),
        to:ctx.accounts.user_staking_token.to_account_info(),
        mint:ctx.accounts.staking_mint.to_account_info(),
        authority:farm.to_account_info()
    }).with_signer(&signer_seeds);

    transfer_checked(transfer_ctx, withdraw_amount, ctx.accounts.staking_mint.decimals)?;
    farm.staked_amount = farm.staked_amount.checked_sub(withdraw_amount).unwrap();
    user_ledger.staked_amount = user_ledger.staked_amount.checked_sub(withdraw_amount).unwrap();

    for i in 0..farm.reward_streams_count {
        user_ledger.reward_infos[i as usize].rewards_debt_x64 = user_ledger.reward_infos[i as usize].rewards_debt_x64.checked_sub(farm.reward_streams[i as usize].acc_rewards_per_base_unit_x64.checked_mul(withdraw_amount.into()).unwrap()).unwrap();
    }
    Ok(())
}