use std::ops::Index;

use crate::{states::*,error::ErrorCode,utils::*};

use anchor_lang::prelude::*;
use anchor_spl::{ token_interface::{Mint,TokenAccount,Token2022,TokenInterface,transfer_checked,TransferChecked,},token::{Token},associated_token::{get_associated_token_address_with_program_id}};

#[derive(Accounts)]
pub struct Harvest<'info> {

    /// CHECK: Owner of the below UserLedger account and Reward mint token accounts, harvest is permissionless ixn. Could be invoked by the farm authority to harvest on behalf of the stakers before withdrawing the reward funds after that particular reward stream has ended.
    pub user: UncheckedAccount<'info>,

    pub staking_mint: InterfaceAccount<'info,Mint>,

    #[account(
        mut,
        has_one = staking_mint,
        seeds = [Farm::STATIC_SEED,staking_mint.key().as_ref()],
        bump = farm.bump
    )]
    pub farm: Account<'info,Farm>,

    #[account(
        mut,
        has_one = user,
        seeds = [UserLedger::STATIC_SEED,farm.key().as_ref(),user.key().as_ref()],
        bump = user_ledger.bump
    )]
    pub user_ledger:Account<'info,UserLedger>,

    pub token_program:Program<'info,Token>,
    pub token_2022_program:Program<'info,Token2022>

    // REMAINING ACCOUNTS 

    // reward_mint_i: InterfaceAccount<Mint>,
    // reward_vault_i:InterfaceAccount<TokenAccount>
    // user_reward_token_i: InterfaceAccount<TokenAccount>,

    // i: 0 -> n - 1 where n = farm.reward_streams_count
}

pub fn handle_harvest<'info>(ctx:Context<'info,Harvest<'info>>)-> Result<()> {

    let farm = &mut ctx.accounts.farm;
    farm.update()?;
    
    let user_ledger = &mut ctx.accounts.user_ledger;
    user_ledger.update(farm)?;

    let farm_seeds:&[&[u8]] = &[Farm::STATIC_SEED.as_ref(),farm.staking_mint.as_ref(),&[farm.bump]];
    let signer_seeds = [&farm_seeds[..]];
    // Reward stakers.
      for i in  0..farm.reward_streams_count {
        let reward_mint:InterfaceAccount<Mint> = InterfaceAccount::try_from(ctx.remaining_accounts.index((i*3) as usize))?;
        let reward_vault:InterfaceAccount<TokenAccount> = InterfaceAccount::try_from(ctx.remaining_accounts.index((i*3 + 1) as usize))?;
        let user_reward_token:InterfaceAccount<TokenAccount> = InterfaceAccount::try_from(ctx.remaining_accounts.index((i*3 + 2) as usize))?;

        // validate the user reward token.
        require_keys_eq!(user_reward_token.mint,farm.reward_streams[i as usize].reward_mint,ErrorCode::MismatchingAccounts);
        require_keys_eq!(user_reward_token.owner,ctx.accounts.user.key(),ErrorCode::MismatchingAccounts);

        // validate reward vault is an ATA of reward mint owned by farm.
        let reward_vault_address = get_associated_token_address_with_program_id(&farm.key(), &reward_mint.key(), &reward_mint.to_account_info().owner.key());
        require_keys_eq!(reward_vault.key(),reward_vault_address);

        // validate the reward mint.
        require_keys_eq!(reward_mint.key(),farm.reward_streams[i as usize].reward_mint,ErrorCode::MismatchingAccounts);

        let transfer_amount = from_x64(user_ledger.reward_infos[i as usize].pending_rewards_x64);
        if transfer_amount > 0 {
            let transfer_ctx = CpiContext::new(reward_mint.to_account_info().owner.key(), TransferChecked {
                from:reward_vault.to_account_info(),
                to:user_reward_token.to_account_info(),
                mint: reward_mint.to_account_info(),
                authority:farm.to_account_info()
            }).with_signer(&signer_seeds);

            transfer_checked(transfer_ctx,transfer_amount,reward_mint.decimals)?;
        }
        user_ledger.reward_infos[i as usize].pending_rewards_x64 = user_ledger.reward_infos[i as usize].pending_rewards_x64.checked_sub((u128::from(transfer_amount)).checked_shl(64).unwrap()).unwrap();
    }

    Ok(())
}