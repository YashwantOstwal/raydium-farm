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
    pub fn harvest<'info>(ctx:Context<'info,Harvest<'info>>)->Result<()> {
        instructions::harvest::handle_harvest(ctx)
    }
    pub fn add_reward(ctx:Context<AddReward>,new_reward_stream:RewardStreamArgs)->Result<()> {
        instructions::add_reward::handle_add_reward(ctx,new_reward_stream)
    }
    pub fn set_rewards(ctx:Context<SetRewards>,reward_stream_idx:u8,updated_reward_stream:RewardStreamArgs)->Result<()> {
        instructions::set_rewards::handle_set_rewards(ctx,reward_stream_idx,updated_reward_stream)
    }
    pub fn restart_rewards(ctx:Context<RestartRewards>,reward_stream_idx:u8,reward_stream:RewardStreamArgs)->Result<()> {
        instructions::restart_rewards::handle_restart_rewards(ctx,reward_stream_idx,reward_stream)
    }

    // Invoke only after the reward stream is ended and every staker have had a chance to harvest at the end time and after.
    // Do not call it early. or else harvest on behalf of all the stakers and then withdraw the remaining.
    pub fn withdraw_reward(ctx:Context<WithdrawReward>,reward_stream_idx:u8)->Result<()> {
        instructions::withdraw_reward::handle_withdraw_reward(ctx,reward_stream_idx)
    }
}
