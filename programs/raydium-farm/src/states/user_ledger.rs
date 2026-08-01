use std::ops::Index;

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount};

use crate::Farm;

#[account]
#[derive(InitSpace)]
pub struct UserLedger {
    pub user: Pubkey,
    pub staked_amount: u64,
    pub reward_infos: [RewardInfo;5],
    pub bump: u8
}

#[derive(AnchorSerialize,AnchorDeserialize,Clone,Copy,InitSpace)]
pub struct RewardInfo {
    pub rewards_debt_x64: u128,
    pub pending_rewards_x64: u128,
}

impl UserLedger {
    pub const LEN:usize = 8 + UserLedger::INIT_SPACE;
    pub const STATIC_SEED:&str = "user_ledger";

    pub fn update_user_ledger(&mut self, farm:&Account<Farm>) -> Result<()>{
    for i in 0..farm.reward_streams_count {
        let new_rewards = farm.reward_streams[i as usize].acc_rewards_per_base_unit_x64.checked_mul(self.staked_amount.into()).unwrap().checked_sub(self.reward_infos[i as usize].rewards_debt_x64).unwrap();

        self.reward_infos[i as usize].pending_rewards_x64 = self.reward_infos[i as usize].pending_rewards_x64.checked_add(new_rewards).unwrap();
        self.reward_infos[i as usize].rewards_debt_x64 = self.reward_infos[i as usize].rewards_debt_x64.checked_add(new_rewards).unwrap();
    }
        Ok(())
    }
}
