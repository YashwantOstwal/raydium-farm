
use anchor_lang::{prelude::system_program, AccountDeserialize};
use anchor_spl::{associated_token::*,token};
use litesvm::*;
use litesvm_token::{
    get_spl_account, spl_token::{native_mint::DECIMALS, state::{Account as TokenAccount,Mint}}, CreateAccount, CreateAssociatedTokenAccount, CreateMint, MintTo, Transfer,TOKEN_ID
};
use raydium_farm::{utils::*, RewardStreamArgs};
use sha2::{Sha256, Digest};
use solana_sdk::{
    account::Account, clock::{self, Clock}, message::{AccountMeta, Instruction}, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::{read_keypair_file, Keypair, Signer}, transaction::Transaction
};

pub mod client;
pub use client::*;



#[test]
pub fn test_raydium_farm() {
    let mut svm =   LiteSVM::new();

    let yash = Keypair::new(); // it's me.
    svm.airdrop(&yash.pubkey(), LAMPORTS_PER_SOL).unwrap();

    let raydium_farm_keypair = read_keypair_file("/home/yashwant/Desktop/web3/raydium-farm/target/deploy/raydium_farm-keypair.json").unwrap();
    let raydium_farm_id = raydium_farm_keypair.pubkey();

    let raydium_farm_bytes = include_bytes!("/home/yashwant/Desktop/web3/raydium-farm/target/deploy/raydium_farm.so");

    svm.add_program(raydium_farm_id, raydium_farm_bytes).unwrap();

    assert!(svm.get_account(&raydium_farm_id).is_some());
    assert!(svm.get_account(&raydium_farm_id).unwrap().executable);

    // Setup
    // Staking mint for the farm we are about to create.
    let staking_mint_authority = Keypair::new();
    let staking_mint = CreateMint::new(&mut svm, &yash).authority(&staking_mint_authority.pubkey()).decimals(0).token_program_id(&token::ID).send().unwrap();
    let staking_mint_program = token::ID;

    // Alice 
    let alice = Keypair::new();
    svm.airdrop(&alice.pubkey(), LAMPORTS_PER_SOL).unwrap();
    let alice_staking_ata = CreateAssociatedTokenAccount::new(&mut svm,&alice,&staking_mint).owner(&alice.pubkey()).send().unwrap();
    MintTo::new(&mut svm,&yash,&staking_mint,&alice_staking_ata,100).owner(&staking_mint_authority).send().unwrap();
    let alice_staking_token: TokenAccount = get_spl_account(&svm, &alice_staking_ata).unwrap();
    assert_eq!(alice_staking_token.amount,100);
    
    // Bob
    let bob = Keypair::new();
    svm.airdrop(&bob.pubkey(), LAMPORTS_PER_SOL).unwrap();
    let bob_staking_ata = CreateAssociatedTokenAccount::new(&mut svm,&bob,&staking_mint).owner(&bob.pubkey()).send().unwrap();
    MintTo::new(&mut svm,&yash,&staking_mint,&bob_staking_ata,100).owner(&staking_mint_authority).send().unwrap();
    let bob_staking_token: TokenAccount = get_spl_account(&svm, &bob_staking_ata).unwrap();
    assert_eq!(bob_staking_token.amount,100);


    let farm_seeds: &[&[u8]] = &[raydium_farm::Farm::STATIC_SEED,&staking_mint.to_bytes()];
    let (farm_pda,farm_bump) = Pubkey::find_program_address(farm_seeds, &raydium_farm_id);

    let reward_0_mint = CreateMint::new(&mut svm, &yash).authority(&yash.pubkey()).decimals(0).token_program_id(&token::ID).send().unwrap();

    let yash_reward_0_token = CreateAssociatedTokenAccount::new(&mut svm,&yash,&reward_0_mint).owner(&yash.pubkey()).send().unwrap();
    MintTo::new(&mut svm,&yash,&reward_0_mint,&yash_reward_0_token, 100000).owner(&yash).send().unwrap();
    let yash_reward_0_token_account_before:TokenAccount = get_spl_account(&svm, &yash_reward_0_token).unwrap();


    //  Creating a farm with "staking_mint" with one reward stream of "reward_mint" (2 decimals),
    //  emission per second = 201.11 tokens per 2 seconds = 100.555 tokens per second. Following the Q64.64 fixed point precision notation, the last 64 bits stores the fractional part of the emission rate
    //  open time = 0 (now),
    //  close time = 3.
    let emission_per_second_x64 = 20111u128.checked_shl(64).unwrap().checked_div(2).unwrap(); 

    let clock:Clock = svm.get_sysvar();
    let open_time = clock.unix_timestamp;
    let mut end_time = clock.unix_timestamp + 3; // This reward stream duration is 3 seconds.

    create_farm(&mut svm,CreateFarmIxn {
        creator:&yash,
        staking_mint:&staking_mint,
        reward_streams:[Some(CreateFarmRewardStreams {
            reward_mint:&reward_0_mint,
            open_time,
            end_time,
            emission_per_second_x64,
        }),None,None,None,None]
    }).unwrap();

    // Verifying the farm state.
    let farm_data = get_farm(&svm, &farm_pda);

    assert_eq!(farm_data.authority,yash.pubkey());
    assert_eq!(farm_data.staking_mint,staking_mint);

    let staking_mint_program = svm.get_account(&staking_mint).unwrap().owner;
    assert_eq!(farm_data.staking_mint_program,staking_mint_program);
    assert_eq!(farm_data.staked_amount,0);
    assert_eq!(farm_data.last_updated_time,clock.unix_timestamp);
    assert_eq!(farm_data.reward_streams_count,1);
    assert_eq!(farm_data.bump,farm_bump);

    let farm_reward_stream_0 = &farm_data.reward_streams[0 as usize];
    assert_eq!(farm_reward_stream_0.reward_mint, reward_0_mint);
    assert_eq!(farm_reward_stream_0.open_time, open_time);
    assert_eq!(farm_reward_stream_0.end_time, end_time);
    assert_eq!(farm_reward_stream_0.emission_per_second_x64, emission_per_second_x64);
    verify_updated_farm_status(&svm, &farm_data, 0);
    assert_eq!(farm_reward_stream_0.acc_rewards_per_base_unit_x64,0);
    
    let total_rewards_x64 = emission_per_second_x64.checked_mul(end_time.checked_sub(open_time).unwrap() as u128).unwrap(); // 100.555 * 2^ 64 * 3
    let expected_vault_balance = ceil_div_x64(total_rewards_x64); // ceil((100.555 * 2^ 64 * 3) / 2^64) = 301.67 (301.665).
    assert_eq!(expected_vault_balance, 30167u64);
    assert_eq!(farm_reward_stream_0.rewards_left_x64.checked_shr(64).unwrap() as u64,expected_vault_balance);

    let reward_vault_0 = get_associated_token_address_with_program_id(&farm_pda, &reward_0_mint,&token::ID);
    let reward_vault_0_token: TokenAccount = get_spl_account(&svm, &reward_vault_0).unwrap();
    assert_eq!(reward_vault_0_token.amount, expected_vault_balance); // += expected_vault_balance

    let yash_reward_0_token_account: TokenAccount = get_spl_account(&svm, &yash_reward_0_token).unwrap();
    assert_eq!(yash_reward_0_token_account.amount, yash_reward_0_token_account_before.amount - expected_vault_balance); // -= expected_vault_balance

    // Creating Alice reward tokens
    let mut alice_reward_tokens:Vec<Pubkey> = vec![];
    for i in 0..farm_data.reward_streams_count {
        let alice_reward_token = CreateAssociatedTokenAccount::new(&mut svm,&alice,&farm_data.reward_streams[i as usize].reward_mint).owner(&alice.pubkey()).token_program_id(&farm_data.reward_streams[i as usize].reward_mint_program).send().unwrap();
        alice_reward_tokens.push(alice_reward_token);
    }

    let alice_staking_token_before = alice_staking_token;


    // Alice staking 100 tokens.
    stake(&mut svm,StakeIxn {
        staker:&alice,staking_mint:&staking_mint,staking_mint_program:&staking_mint_program,staker_staking_token:&alice_staking_ata,reward_tokens:&alice_reward_tokens,deposit_amount:100
    }).unwrap();
    
    let alice_staking_token_after: TokenAccount = get_spl_account(&svm, &alice_staking_ata).unwrap();
    assert_eq!(alice_staking_token_after.amount,alice_staking_token_before.amount - 100);

    let farm_data_after = get_farm(&svm,&farm_pda);
    assert_eq!(farm_data_after.staked_amount,100); 

    let (alice_ledger_pda,alice_ledger_bump) = derive_user_ledger_pda(&farm_pda,&alice.pubkey());
    let alice_ledger_after = get_user_ledger(&svm,&alice_ledger_pda);

    // Verifying the ledger state.
    assert_eq!(alice_ledger_after.user,alice.pubkey());
    assert_eq!(alice_ledger_after.staked_amount,100);
    assert_eq!(alice_ledger_after.bump,alice_ledger_bump);

    let alice_reward_info_0 = &alice_ledger_after.reward_infos[0 as usize];
    assert_eq!(alice_reward_info_0.pending_rewards_x64,0);
    assert_eq!(alice_reward_info_0.rewards_debt_x64,0);

    let farm_data_before = farm_data_after;
    let farm_reward_stream_0_before = &farm_data_before.reward_streams[0 as usize];
    let yash_reward_0_token_account_before = yash_reward_0_token_account;
    let reward_vault_0_account_before = reward_vault_0_token;

    let total_rewards_x64 = emission_per_second_x64.checked_mul(4).unwrap(); // 100.555 * 4 = 402.22.
    let expected_transfer_amount = ceil_div_x64(total_rewards_x64.checked_sub(farm_reward_stream_0.rewards_left_x64).unwrap());

    // Extending the reward_stream_0 duration by 1 second with the same emission rate
    end_time += 1;
    set_reward_ixn(&mut svm, SetRewardIxn { creator: &yash, staking_mint: &staking_mint, reward_stream_idx: 0, updated_reward_stream: RewardStreamArgs{
        open_time,
        end_time,
        emission_per_second_x64,
    } }).unwrap();

    assert_eq!(expected_transfer_amount,10055u64); // Prefunded the 0.005 tokens out of 100.555 to the vault.

    let farm_data_after = get_farm(&svm,&farm_pda);
    let farm_reward_stream_0_after = &farm_data_after.reward_streams[0 as usize];
    
    assert_eq!(farm_reward_stream_0_after.rewards_left_x64, farm_reward_stream_0_before.rewards_left_x64.checked_add((expected_transfer_amount as u128).checked_shl(64).unwrap()).unwrap());
    assert_eq!(farm_reward_stream_0_after.rewards_left_x64,40222u128.checked_shl(64).unwrap()); // 100.555 * 4 = 402.22 tokens.
    assert_eq!(farm_reward_stream_0_after.end_time, end_time);
    assert_eq!(farm_reward_stream_0_after.emission_per_second_x64, emission_per_second_x64);
    
    let yash_reward_0_token_account_after: TokenAccount = get_spl_account(&svm, &yash_reward_0_token).unwrap();
    assert_eq!(yash_reward_0_token_account_after.amount, yash_reward_0_token_account_before.amount.checked_sub(expected_transfer_amount).unwrap());

    let reward_vault_0_account_after: TokenAccount = get_spl_account(&svm, &reward_vault_0).unwrap();
    assert_eq!(reward_vault_0_account_after.amount, reward_vault_0_account_before.amount.checked_add(expected_transfer_amount).unwrap());


    //  Current state t = 0, Yash created a farm with "staking_mint" with one reward stream of "reward_mint" (2 decimals), Extended the end_time 
    //  with "set_rewards" ixn and Alice staked 100 tokens to the farm. 
    //  emission_per_second_x64 for the 1st reward stream = 100.555 * 2^64 (2 decimals reward token),
    //  acc_rewards_per_base_unit_x64 for the 1st reward stream = 0 * 2^64.
    //  farm.staked_amount = 100.
    //  rewards_left_x64[0] = 402.22 * 2^64 . (reward_vault[0] * 2^64 >= rewards_left_x64 )
    //  Alce's staked_amount = 100.
    //  Alice's pending_rewards_x64[0] of the 1st reward stream = 0 * 2^64 (Nothing is owed by the farm as the deposition happened at this exact instant),
    //  Alice's rewards_debt_x64[0] of the 1st reward stream = 0 * 2^64 (No rewards is missed or collected)
    
    let farm_data_before = farm_data_after;
    let alice_ledger_data_before = alice_ledger_after;
    let alice_reward_0_token_before:TokenAccount = get_spl_account(&svm, &alice_reward_tokens[0 as usize]).unwrap();

    time_travel(&mut svm, 1); 
    let clock = svm.get_sysvar::<Clock>();
    assert_eq!(clock.unix_timestamp,1);

    // t = 1, Alice harvests.
    harvest(&mut svm,HarvestIxn {
        staker:&alice,
        staking_mint:&staking_mint,
        reward_tokens:&alice_reward_tokens
    }).unwrap();

    // 6 Exhaustive critical checks:
    //  new_emission_x64[0] = (emission_per_second_x64[0] * duration(now,farm_before.last_update_time)) = 100.555 * 2^64 * 1
    //  1) new_acc_rewards_per_base_unit_x64[0]= acc_rewards_per_base_unit_x64[0] + new_emission_x64[0] / total_staked_amount = 0 * 2^64 + ((100.555 * 2^64) / 100) = 1.00555 ^ 2^64.
    //  2) new rewards_left_x64[0] = rewards_left_x64[0] - new_emission_x64[0] = 402.22 * 2^64 - 100.555 * 2^64 = 301.665 * 2^64
    //  new_alice_rewards = new_acc_rewards_per_base_unit_x64[0] * alice.staked_amount_before - rewards_debt_x64_before[0] = 1.00555 * 2^64 * 100 - 0 = 100.555 ^ 2^64 (100% of the emitted tokens of this second is owed to Alice as She is the only staker).
    //  3) Alice's harvested rewards = Alice's pending_rewards_x64[0] + new_alice_rewards = 0 + 100.555 * 2^64 = 100.555 * 2^64 out of which >>64 = 100.55 (note: if I do *_x64 >>64, then I will only retain the 2 decimal places for a 2 decimal mint, i.e, 100.555 >>64 = 10055u64) is transfered to the token account.
    //  4) While the amount to be transferred - Amount transferred = 0.005 (Lesser than denomination that can be transfered) is stored in Alice's pending_rewards_x64[0] = 0.005 * 2^64 (The last 64 bits holds the fractional value, follows Q64.64 fixed point precision notation).
    //  5) new_reward_vault_balance[0] = rewards_vault_balance[0] - Amount transfered to staker(s) = 402.22 - 100.55 = 301.67 (reward_vault[0] * 2^64 >= rewards_left_x64) but out of which 0.005 is owed to Alice and locked, If Alice held her stake for one more second she will receive it. Will see this in next test. 
    //  6) Alice's rewards_debt_x64[0] = Alice's rewards_debts_x64_old[0] + new_alice_rewards = 0 + 100.555 * 2^64 = 100.555 * 2^64 (This rewards_debt_x64 represents the missed or collected rewards of the respective stream)

    let farm_data_after = get_farm(&svm, &farm_pda);
    let alice_ledger_data_after = get_user_ledger(&svm, &alice_ledger_pda);
    let alice_reward_0_token_after:TokenAccount = get_spl_account(&svm, &alice_reward_tokens[0 as usize]).unwrap();


    time_travel(&mut svm,1);
    let clock = svm.get_sysvar::<Clock>();
    assert_eq!(clock.unix_timestamp,2);

    harvest(&mut svm,HarvestIxn {
        staker:&alice,
        staking_mint:&staking_mint,
        reward_tokens:&alice_reward_tokens
    }).unwrap();

    // t = 2
    // 6 Exhaustive critical checks:
    //  new_emission_x64[0] = (emission_per_second_x64[0] * duration(now,farm_before.last_update_time)) = 100.555 * 2^64 * 1
    //  1) new_acc_rewards_per_base_unit_x64[0]= acc_rewards_per_base_unit_x64[0] + new_emission_x64[0] / total_staked_amount = 1.00555 ^ 2^64 + ((100.555 * 2^64) / 100) = 2.0111 * 2^64.
    //  2) new rewards_left_x64[0] = rewards_left_x64[0] - new_emission_x64[0] = 301.665 * 2^64 - 100.555 * 2^64 = 201.11 * 2^64 
    //  new_alice_rewards = new_acc_rewards_per_base_unit_x64[0] * alice.staked_amount_before - rewards_debt_x64_before[0] = 2.0111 ^ 2^64 * 100 - 100.555 * 2^64  = 100.555 * 2^64 (100% of the emitted tokens of this second is owed to Alice as She is the only staker).
    //  3) Alice's harvested rewards = Alice's pending_rewards_x64[0] + new_alice_rewards = 0.005 * 2^64 + 100.555 * 2^64 = 100.56 * 2^64 >>64 = 100.56 is transfered to the token account.
    //  4) Alice's pending_rewards_x64[0] = 0 * 2^64. (IMPORTANT: Alice was rewarded 100.55 at t = 1 and 100.56 at t = 2 making the total 201.11 and since Alice is the only staker and emission rate is set to "201.11 tokens per 2 seconds (2 decimal reward mint)", She got all the reward but the interesting thing is how the reward was processed, at t = 1, 100.55 transferred + 0.005 pending and at t = 2, 100.56 added the pending and then transferred)
    //  5) new_reward_vault_balance[0] = rewards_vault_balance[0] - Amount harvested by staker(s) = 301.67 - 100.56 = 201.11.
    //  6) Alice's rewards_debt_x64[0] = Alice's rewards_debts_x64_old[0] + new_alice_rewards = 100.555 * 2^64 + 100.555 * 2^64 = 201.11 * 2^64.

    let farm_data_2 = get_farm(&svm, &farm_pda);
    let alice_ledger_data_2 = get_user_ledger(&svm, &alice_ledger_pda);
    let alice_reward_0_token_2:TokenAccount = get_spl_account(&svm, &alice_reward_tokens[0 as usize]).unwrap();

    // Alice stakes more 0.20 tokens, Bob stakes 0.80 tokens.
    // Alice's user_ledger -> 
    //  new_staked_amount = 100 + 20 = 120
    //  new_rewards_debt_x64[0] = rewards_debt_x64[0] + new_staked_amount * new_acc_rewards_per_base_unit_x64 = 201.11 * 2^64 + 20 * 2.0111 * 2^64 = 241.332 * 2^64
    
    // Bob's user_ledger -> 
    //  new_staked_amount = 80
    //  new_rewards_debt_x64[0] = rewards_debt_x64[0] + new_staked_amount * new_acc_rewards_per_base_unit_x64 = 0 + 80 * 2.0111 * 2^64 = 160.888 * 2^64

    // farm.staked_amount += 120 (20 + 80) = 200

    let farm_data_3 = get_farm(&svm, &farm_pda);

    let alice_ledger_data_3 = get_user_ledger(&svm, &alice_ledger_pda);
    let alice_reward_0_token_3:TokenAccount = get_spl_account(&svm, &alice_reward_tokens[0 as usize]).unwrap();

    time_travel(&mut svm,1);
    let clock = svm.get_sysvar::<Clock>();
    assert_eq!(clock.unix_timestamp,3);

    // t = 3, Alice withdraws all her staked assets.
    // Exhaustive critical checks:
    //  new_emission_x64[0] = (emission_per_second_x64[0] * duration(now,farm_before.last_update_time)) = 100.555 * 2^64 * 1. (NOTE: If staked amount is 0, new emission is 0)
    //  1) new_acc_rewards_per_base_unit_x64[0]= acc_rewards_per_base_unit_x64[0] + new_emission_x64[0] / total_staked_amount = 2.0111 * 2^64 + ((100.555 * 2^64) / 200) = 2.513875 * 2^64.
    //  2) new rewards_left_x64[0] = rewards_left_x64[0] - new_emission_x64[0] = 201.11 * 2^64 - 100.555 * 2^64 = 100.555 * 2^64 (left for last emission in the next second)

    //  new_alice_rewards = new_acc_rewards_per_base_unit_x64[0] * alice.staked_amount_before - rewards_debt_x64_before[0] = 2.513875 * 2^64 * 120 - 241.332 * 2^64  = 60.333 * 2^64
    //  3) Alice's harvested rewards = Alice's pending_rewards_x64[0] + new_alice_rewards = 0 * 2^64 + 60.333 * 2^64 = 60.333 * 2^64 >>64 = 60.33 is transfered to the token account.
    //  4) Alice's pending_rewards_x64[0] = 0.003 * 2^64. 
    //  6) Alice's rewards_debt_x64[0] = Alice's rewards_debts_x64_old[0] + new_alice_rewards - withdraw_amount * new_acc_rewards_per_base_unit_x64 = 241.332 * 2^64 + 60.333 * 2^64 - 120 * 2.513875 * 2^64 = 0 * 2^64.

    //  new_bob_rewards = new_acc_rewards_per_base_unit_x64[0] * bob.staked_amount_before - rewards_debt_x64_before[0] = 2.513875 * 2^64 * 80 - 160.888 * 2^64  = 40.222 * 2^64
    //  7) Bob's harvested rewards = Bob's pending_rewards_x64[0] + new_bob_rewards = 0 * 2^64 + 40.222 * 2^64 = 40.222 * 2^64 >>64 = 40.22 is transfered to the token account.
    //  8) Bob's pending_rewards_x64[0] = 0.002 * 2^64. 
    //  9) Bob's rewards_debt_x64[0] = Bob's rewards_debts_x64_old[0] + new_bob_rewards = 160.888 * 2^64 + 40.222 * 2^64 = 201.11 * 2^64.
    
    // Reward emitted was divided between the stakers(2) Alice and Bob, 60.333 and 40.222 respectively. total = 100.555. 
    //  10) new_reward_vault_balance[0] = rewards_vault_balance[0] - Amount harvested by staker(s) = 201.11 - (60.33 + 40.22)  = 100.56 .

    time_travel(&mut svm,1);
    let clock = svm.get_sysvar::<Clock>();
    assert_eq!(clock.unix_timestamp,4);

    // At t = 4, Adding a new reward stream with reward mint of 6 decimals open time to be t = 5 and close time to be t = 8, with this time simple emission per second = 1.000000 token per second (1 token/sec) 
    //  1) acc_rewards_per_base_unit_x64[1] = 0 * 2^64.
    //  7) rewards_left_x64[1] = transfer_amount = 3.000000 * 2^64.
    //  8) new_reward_vault_balance[1] = 3.000000 tokens.
    
    // Verify farm's reward_streams_count incremented by one and the farm's new reward stream initial state, especially the status
    // Verify the farm's 1st reward stream is closed.
    time_travel(&mut svm,1);
    let clock = svm.get_sysvar::<Clock>();
    assert_eq!(clock.unix_timestamp,5);

    // At t = 5, Bob harvests. Also restarting the 1st reward stream later.
    //  From the 1st reward stream -> Bob is rewarded for 1 second though 2 seconds have been passed since last harvest put the reward stream itself was closed a second ago.
    //  From the 2nd reward stream -> Nothing. It is opened just now. Status = Running.
    //  new_emission_x64[0] = (emission_per_second_x64[0] * duration(end_time,min(end_time,last_updated_time))) = 100.555 * 2^64 * 5 - min(5,4) = 100.555 * 2^64 * 1
    //  1) new_acc_rewards_per_base_unit_x64[0]= acc_rewards_per_base_unit_x64[0] + new_emission_x64[0] / total_staked_amount = 2.513875 * 2^64 + ((100.555 * 2^64) / 80) = 3.7708125 * 2^64.
    //  new_bob_rewards = new_acc_rewards_per_base_unit_x64[0] * bob.staked_amount_before - rewards_debt_x64_before[0] = 3.7708125 * 2^64 * 80 - 201.11 * 2^64  = 100.555 * 2^64 (100% of the emitted tokens of this second is owed to Bob as He is the only staker).
    //  2) Bob's harvested rewards = Bob's pending_rewards_x64[0] + new_bob_rewards = 0.002 * 2^64 + 100.555 * 2^64 = 100.557 * 2^64 >>64 = 100.55 is transfered to the token account.
    //  3) Bob's pending_rewards_x64[0] = 0.007 * 2^64.
    //  4) Bob's rewards_debt_x64[0] = Bob's rewards_debts_x64_old[0] + new_bob_rewards = 201.11 * 2^64 + 100.555 * 2^64 = 301.665 * 2^64.

    //  5) new rewards_left_x64[0] = rewards_left_x64[0] - new_emission_x64[0] = 100.555 * 2^64 - 100.555 * 2^64 = 0
    //  6) new_reward_vault_balance[0] = rewards_vault_balance[0] - Amount harvested by staker(s) = 100.56 - 100.55 = 0.01. (0.003 is locked for Alice and 0.0007 is locked for Bob. If no plan to restart this reward stream and all our stakers have claimed all their latest rewards, The creator can consider to withdraw them using "withdraw_funds" ixn if not harvest on behalf of every staker and withdraw. The official raydium's docs clear states to not call it early.)

    // Restarting the 1st reward stream for 3 seconds with emission per second to be 100.553 (2 decimals).
    //  transfer_amount = ceil((100.553 * 2^64 * 3) / 2^64) = ceil((301.659 * 2^64 ) / 2^64) = 301.66
    //  7) new_rewards_left_x64[0] = rewards_left_x64[0] + transfer_amount = 0 + 301.66 * 2^64 = 301.66 * 2^64.
    //  8) new_reward_vault_balance[0] = rewards_vault_balance[0] + transfer_amount = 0.01 + 301.66 = 301.67.

    time_travel(&mut svm,1);
    let clock = svm.get_sysvar::<Clock>();
    assert_eq!(clock.unix_timestamp,6);

    // At t = 6, Bob withdraws all his staked assets
    //  new_emission_x64[0] = (emission_per_second_x64[0] * duration(now,max(open_time,last_update_time))) = 100.553 * 2^64 * (6 - max(5,5)) = 100.553 * 2^64 * 1
    //  1) new_acc_rewards_per_base_unit_x64[0]= acc_rewards_per_base_unit_x64[0] + new_emission_x64[0] / total_staked_amount = 3.7708125 * 2^64 + ((100.553 * 2^64) / 80) = 5.027725 * 2^64.
    //  2) new rewards_left_x64[0] = rewards_left_x64[0] - new_emission_x64[0] = 301.66 * 2^64 - 100.553 * 2^64 = 201.107 * 2^64

    //  new_bob_rewards = new_acc_rewards_per_base_unit_x64[0] * bob.staked_amount_before - rewards_debt_x64_before[0] = 5.027725 * 2^64 * 80 - 301.665 * 2^64  = 100.553 * 2^64
    //  3) Bob's harvested rewards = Bob's pending_rewards_x64[0] + new_bob_rewards = 0.007 * 2^64 + 100.553 * 2^64 = 100.56 * 2^64 >>64 = 100.56 is transfered to the token account.
    //  4) Bob's pending_rewards_x64[0] = 0 * 2^64. 
    //  6) Bob's rewards_debt_x64[0] = Bob's rewards_debts_x64_old[0] + new_bob_rewards - withdraw_amount * new_acc_rewards_per_base_unit_x64 = 301.665 * 2^64 + 100.553 * 2^64 - 80 * 5.027725 * 2^64 = 0 * 2^64.


    //  new_emission_x64[1] = (emission_per_second_x64[1] * duration(now,max(open_time,last_update_time))) = 1.000000 * 2^ 64 * (6 - max(5,5)) = 1.000000 * 2^ 64 * 1.
    //  1) new_acc_rewards_per_base_unit_x64[1]= acc_rewards_per_base_unit_x64[1] + new_emission_x64[1] / total_staked_amount = 0 + ((1.000000 * 2^64) / 80) = 0.0125 * 2^64.
    //  2) new rewards_left_x64[1] = rewards_left_x64[1] - new_emission_x64[1] = 3.000000 * 2^64 - 1.000000 * 2^ 64 = 2.000000 * 2^ 64

    //  new_bob_rewards = new_acc_rewards_per_base_unit_x64[1] * bob.staked_amount_before - rewards_debt_x64_before[1] = 0.0125 * 2^64 * 80 - 0 * 2^64  = 1.000000 * 2^64
    //  3) Bob's harvested rewards = Bob's pending_rewards_x64[1] + new_bob_rewards = 0 * 2^64 + 1.000000 * 2^64 = 1.000000 * 2^64 >>64 = 1.000000 is transfered to the token account.
    //  4) Bob's pending_rewards_x64[1] = 0 * 2^64. 
    //  6) Bob's rewards_debt_x64[1] = Bob's rewards_debts_x64_old[1] + new_bob_rewards - withdraw_amount * new_acc_rewards_per_base_unit_x64 = 0 + 1.000000 * 2^64 - 80 * 0.0125 * 2^64 = 0 * 2^64.
  

}