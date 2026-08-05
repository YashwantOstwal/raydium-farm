
use anchor_lang::{prelude::system_program, AccountDeserialize};
use anchor_spl::{associated_token,token};
use litesvm::LiteSVM;
use litesvm_token::{
    get_spl_account, spl_token::{native_mint::DECIMALS, state::Account as TokenAccount}, CreateAccount, CreateAssociatedTokenAccount, CreateMint, MintTo, Transfer,TOKEN_ID
};
use raydium_farm::{create_farm, Farm, RewardStreamArgs, RewardStreamStatus};
use sha2::{Sha256, Digest};
use solana_sdk::{
    account::Account, clock::{self, Clock}, message::{AccountMeta, Instruction}, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::{read_keypair_file, Keypair, Signer}, transaction::Transaction
};

pub mod client;
pub use client::*;

#[test]
pub fn test_raydium_farm() {
    let mut svm =   LiteSVM::new();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), LAMPORTS_PER_SOL).unwrap();


    let raydium_farm_keypair = read_keypair_file("/home/yashwant/Desktop/web3/raydium-farm/target/deploy/raydium_farm-keypair.json").unwrap();
    let raydium_farm_id = raydium_farm_keypair.pubkey();

    let raydium_farm_bytes = include_bytes!("/home/yashwant/Desktop/web3/raydium-farm/target/deploy/raydium_farm.so");

    svm.add_program(raydium_farm_id, raydium_farm_bytes).unwrap();

    assert!(svm.get_account(&raydium_farm_id).is_some());
    assert!(svm.get_account(&raydium_farm_id).unwrap().executable);

    let staking_mint_authority = Keypair::new();
    let staking_mint_program = token::ID;
    let staking_mint = CreateMint::new(&mut svm, &payer).authority(&staking_mint_authority.pubkey()).token_program_id(&staking_mint_program).send().unwrap();

    let alice = Keypair::new();
    let bob = Keypair::new();
    svm.airdrop(&alice.pubkey(), LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&bob.pubkey(), LAMPORTS_PER_SOL).unwrap();

    let alice_staking_ata = CreateAssociatedTokenAccount::new(&mut svm,&alice,&staking_mint).owner(&alice.pubkey()).send().unwrap();
    let bob_staking_ata = CreateAssociatedTokenAccount::new(&mut svm,&bob,&staking_mint).owner(&bob.pubkey()).send().unwrap();

    MintTo::new(&mut svm,&payer,&staking_mint,&alice_staking_ata,140).owner(&staking_mint_authority).send().unwrap();
    MintTo::new(&mut svm,&payer,&staking_mint,&bob_staking_ata,150).owner(&staking_mint_authority).send().unwrap();

    let alice_staking_token: TokenAccount = get_spl_account(&svm, &alice_staking_ata).unwrap();
    assert_eq!(alice_staking_token.amount,140);
    let bob_staking_token: TokenAccount = get_spl_account(&svm, &bob_staking_ata).unwrap();
    assert_eq!(bob_staking_token.amount,150);

    
    let farm_seeds: &[&[u8]] = &[raydium_farm::Farm::STATIC_SEED,&staking_mint.to_bytes()];
    let (farm_pda,farm_bump) = Pubkey::find_program_address(farm_seeds, &raydium_farm_id);

    let reward_mint_0 = CreateMint::new(&mut svm, &payer).authority(&payer.pubkey()).decimals(2).token_program_id(&token::ID).send().unwrap();
    let payer_reward_token_0 = CreateAssociatedTokenAccount::new(&mut svm,&payer,&reward_mint_0).owner(&payer.pubkey()).send().unwrap();

    MintTo::new(&mut svm,&payer,&reward_mint_0,&payer_reward_token_0, 2000).owner(&payer).send().unwrap();
    
    
    // Payer creating a farm for staking mint with one reward stream of reward_mint_0.
    let clock:Clock = svm.get_sysvar();
    let block_timestamp = clock.unix_timestamp;

    create_farm(&mut svm,CreateFarmIxn {
        creator:&payer,
        staking_mint:&staking_mint,
        staking_mint_program:&staking_mint_program,
        reward_streams:[Some(CreateFarmRewardStreams {
            reward_mint:&reward_mint_0,
            reward_mint_program:&token::ID,
            open_time: block_timestamp,
            end_time: block_timestamp + 6, // This reward stream ends at t = 6.
            emission_per_second_x64: 152u128.checked_shl(64).unwrap() // 1.52 token/sec emission.
        }),None,None,None,None]
    }).unwrap();

    let farm_data = get_farm(&svm, &farm_pda);

    assert_eq!(farm_data.authority,payer.pubkey());
    assert_eq!(farm_data.staked_amount,0);
    assert_eq!(farm_data.last_updated_time,block_timestamp);
    assert_eq!(farm_data.reward_streams_count,1);
    assert_eq!(farm_data.bump,farm_bump);


    let farm_reward_stream_0 = &farm_data.reward_streams[0 as usize];
    assert_eq!(farm_reward_stream_0.reward_mint, reward_mint_0);
    assert_eq!(farm_reward_stream_0.open_time, block_timestamp);
    assert_eq!(farm_reward_stream_0.end_time, block_timestamp + 6);
    assert_eq!(farm_reward_stream_0.emission_per_second_x64.checked_shr(64).unwrap() as u64,152);
    assert_eq!(farm_reward_stream_0.acc_rewards_per_base_unit_x64,0);
    verify_updated_farm_status(&svm, &farm_data, 0);

    let mut alice_reward_tokens:Vec<Pubkey> = vec![];
    for i in 0..farm_data.reward_streams_count {
        let alice_reward_token = CreateAssociatedTokenAccount::new(&mut svm,&alice,&farm_data.reward_streams[i as usize].reward_mint).owner(&alice.pubkey()).token_program_id(&farm_data.reward_streams[i as usize].reward_mint_program).send().unwrap();
        alice_reward_tokens.push(alice_reward_token);
    }
    stake(&mut svm,StakeIxn {
        staker:&alice,staking_mint:&staking_mint,staking_mint_program:&staking_mint_program,staker_staking_token:&alice_staking_ata,reward_tokens:&alice_reward_tokens,deposit_amount:100
    }).unwrap();

    let farm = get_farm(&svm,&farm_pda);
    assert_eq!(farm.staked_amount,100);

    let (alice_ledger_pda,alice_ledger_bump) = derive_user_ledger_pda(&farm_pda,&alice.pubkey());
    let alice_ledger = get_user_ledger(&svm,&alice_ledger_pda);

    assert_eq!(alice_ledger.user,alice.pubkey());
    assert_eq!(alice_ledger.staked_amount,100);
    assert_eq!(alice_ledger.bump,alice_ledger_bump);

    let user_ledger_reward_info = &alice_ledger.reward_infos[0 as usize];
    assert_eq!(user_ledger_reward_info.pending_rewards_x64,0);

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = clock.unix_timestamp.checked_add(1).unwrap();
    svm.set_sysvar(&clock);

    let clock = svm.get_sysvar::<Clock>();
    assert_eq!(clock.unix_timestamp,1);

    let alice_ledger_before_harvest_0 = get_user_ledger(&svm,&alice_ledger_pda);

    harvest(&mut svm,HarvestIxn {
        staker:&alice,
        staking_mint:&staking_mint,
        reward_tokens:&alice_reward_tokens
    }).unwrap();

    let alice_ledger_after_harvest_0 = get_user_ledger(&svm,&alice_ledger_pda);

}