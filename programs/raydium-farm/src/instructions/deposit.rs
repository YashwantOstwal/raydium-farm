use std::ops::Index;

use crate::{states::*,error::ErrorCode};

use anchor_lang::prelude::*;
use anchor_spl::{ token_interface::{Mint,TokenAccount,TokenInterface,transfer_checked,TransferChecked},associated_token::{get_associated_token_address_with_program_id}};

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mint::token_program = staking_mint_program,
    )]
    pub staking_mint: Box<InterfaceAccount<'info,Mint>>,

    #[account(
        mut,
        seeds = [Farm::STATIC_SEED.as_bytes(),staking_mint.key().as_ref()],
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
        init_if_needed,
        payer = user,
        space = UserLedger::LEN,
        seeds = [UserLedger::STATIC_SEED.as_bytes(),farm.key().as_ref(),user.key().as_ref()],
        bump
    )]
    pub user_ledger:Box<Account<'info,UserLedger>>,

    pub system_program:Program<'info,System>,

    // REMAINING ACCOUNTS 

    // reward_mint_i: InterfaceAccount<Mint>,
    // reward_vault_i:InterfaceAccount<TokenAccount>
    // user_reward_token_i: InterfaceAccount<TokenAccount>,

    // i: 0 -> n - 1 where n = reward_streams_count
}

pub fn handle_deposit<'info>(ctx:Context<'info,Deposit<'info>>,deposit_amount:u64)-> Result<()> {

    require!(deposit_amount > 0,ErrorCode::InvalidAmount);
    let farm = &mut ctx.accounts.farm;
    farm.update_farm()?;
    
    require!(ctx.accounts.user_staking_token.amount >= deposit_amount,ErrorCode::InsufficientBalance);
    
    let user_ledger = &mut ctx.accounts.user_ledger;
    user_ledger.update_user_ledger(farm)?;


    let farm_seeds:&[&[u8]] = &[Farm::STATIC_SEED.as_bytes().as_ref(),farm.staking_mint.as_ref(),&[farm.bump]];
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

    // Deposit
    let transfer_ctx = CpiContext::new(ctx.accounts.staking_mint_program.key(), TransferChecked {
        from:ctx.accounts.user_staking_token.to_account_info(),
        to:ctx.accounts.staking_token_vault.to_account_info(),
        mint:ctx.accounts.staking_mint.to_account_info(),
        authority:ctx.accounts.user.to_account_info()
    });

    transfer_checked(transfer_ctx, deposit_amount, ctx.accounts.staking_mint.decimals)?;
    farm.staked_amount = farm.staked_amount.checked_add(deposit_amount).unwrap();
    user_ledger.staked_amount = user_ledger.staked_amount.checked_add(deposit_amount).unwrap();

    for i in 0..farm.reward_streams_count {
        user_ledger.reward_infos[i as usize].rewards_debt_x64 = user_ledger.reward_infos[i as usize].rewards_debt_x64.checked_add(farm.reward_streams[i as usize].acc_rewards_per_base_unit_x64.checked_mul(deposit_amount.into()).unwrap()).unwrap();
    }
    
    Ok(())
}