use litesvm::LiteSVM;
use litesvm_token::{
    get_spl_account,
    spl_token::{native_mint::DECIMALS, state::Account as TokenAccount},
    CreateAccount, CreateMint, MintTo, Transfer,
};
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL,
    signature::{Keypair, Signer},
};

#[test]
fn test_raydium_farm() {
    let mut svm = LiteSVM::new();

    // Create payer account and fund it
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();

    // Create a new SPL token mint with the payer as the mint authority
    let mint = CreateMint::new(&mut svm, &payer)
        .authority(&payer.pubkey())
        .decimals(DECIMALS)
        .send()
        .unwrap();

    // Create a token account for the payer
    let token_account = CreateAccount::new(&mut svm, &payer, &mint)
        .owner(&payer.pubkey())
        .send()
        .unwrap();

    // Mint tokens into the payer's token account
    MintTo::new(&mut svm, &payer, &mint, &token_account, 1000)
        .owner(&payer)
        .send()
        .unwrap();

    // Verify balance
    let token_account: TokenAccount = get_spl_account(&svm, &token_account).unwrap();
    let account_balance = token_account.amount;
    assert_eq!(account_balance, 100)
}