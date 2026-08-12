
use anchor_lang::prelude::*;

use crate::{Deposit, Farm, Withdraw};

#[account]
#[derive(InitSpace,Debug)]
pub struct UserLedger {
    pub user: Pubkey,
    pub staked_amount: u64,
    pub reward_infos: [RewardInfo;5],
    pub bump: u8
}

#[derive(AnchorSerialize,AnchorDeserialize,Clone,Copy,InitSpace,Debug)]
pub struct RewardInfo {
    pub rewards_debt_x64: u128,
    pub pending_rewards_x64: u128,
}

impl UserLedger {
    pub const LEN:usize = 8 + UserLedger::INIT_SPACE;
    pub const STATIC_SEED:&[u8] = b"user_ledger";

    pub fn update(&mut self, updated_farm:&Account<Farm>) -> Result<()>{
    for i in 0..updated_farm.reward_streams_count {
        let new_rewards = updated_farm.reward_streams[i as usize].acc_rewards_per_base_unit_x64.checked_mul(self.staked_amount.into()).unwrap().checked_sub(self.reward_infos[i as usize].rewards_debt_x64).unwrap();

        self.reward_infos[i as usize].pending_rewards_x64 = self.reward_infos[i as usize].pending_rewards_x64.checked_add(new_rewards).unwrap();
        self.reward_infos[i as usize].rewards_debt_x64 = self.reward_infos[i as usize].rewards_debt_x64.checked_add(new_rewards).unwrap();
        
    }
        Ok(())
    }

}
