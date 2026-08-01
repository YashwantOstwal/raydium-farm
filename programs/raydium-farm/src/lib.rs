pub mod constants;
pub mod error;
pub mod instructions;
pub mod states;

use anchor_lang::prelude::*;

pub use instructions::*;
pub use states::*;

declare_id!("5SZrAvKrkuAdg1WNVtE3bpNuPaKRUZXfogXBotWFMjMu");

#[program]
pub mod raydium_farm {
    use super::*;

    pub fn create_farm<'info>(ctx:Context<'info,CreateFarm<'info>>,reward_streams:[Option<RewardStreamArgs>;5])->Result<()> {
        instructions::create_farm::handle_create_farm(ctx,reward_streams)
    }
    pub fn deposit<'info>(ctx:Context<'info,Deposit<'info>>,deposit_amount:u64)->Result<()> {
        instructions::deposit::handle_deposit(ctx,deposit_amount)
    }

    pub fn withdraw<'info>(ctx:Context<'info,Withdraw<'info>>,withdraw_amount:u64)->Result<()> {
        instructions::withdraw::handle_withdraw(ctx,withdraw_amount)
    }
}
