use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::bs58;

use dotenvy::dotenv;
use std::env;
use serde_json;

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
};
use tokio;
use std::str::FromStr;

use std::time::{Instant, Duration};

use clap::{Arg, Command, ArgAction};

use solana_sdk:: {
    system_instruction,
    transaction::Transaction,
};

use spl_token::{
    instruction::initialize_mint,
    state::Mint,
};
use solana_sdk::program_pack::Pack;

use spl_associated_token_account::instruction::create_associated_token_account;
use spl_associated_token_account::get_associated_token_address;

use spl_token::instruction::mint_to;

use mpl_token_metadata::types::DataV2;
use mpl_token_metadata::instructions::{CreateMetadataAccountV3, CreateMetadataAccountV3InstructionArgs};
use solana_sdk::system_program;

#[tokio::main]
async fn main() {
    let matches = Command::new("Solana CLI")
        .version("0.2.0")
        .author("vlevko")
        .about("A multi-function Solana tool")
        .arg(Arg::new("generate-keypair")
            .short('g')
            .long("generate-keypair")
            .action(ArgAction::SetTrue)
            .help("Generate a new keypair"))
        .arg(Arg::new("load-keypair")
            .short('l')
            .long("load-keypair")
            .action(ArgAction::SetTrue)
            .help("Load keypair from .env SECRET_KEY"))
        .arg(Arg::new("check-balance")
            .short('c')
            .long("check-balance")
            .action(ArgAction::SetTrue)
            .help("Check balance on devnet and request airdrop if required"))
        .arg(Arg::new("find-keypair")
            .short('f')
            .long("find-keypair")
            .action(ArgAction::SetTrue)
            .help("Find a new keypair with the public key starting with 'Lev' within 3 minutes"))
        .arg(Arg::new("send-sol")
            .short('s')
            .long("send-sol")
            .action(ArgAction::SetTrue)
            .help("Send 0.01 SOL to the hardcoded wallet address"))
        .arg(Arg::new("create-token-mint")
            .short('m')
            .long("create-token-mint")
            .action(ArgAction::SetTrue)
            .help("Create a new token mint"))
        .arg(Arg::new("create-token-account")
            .short('a')
            .long("create-token-account")
            .action(ArgAction::SetTrue)
            .help("Create a new token account"))
        .arg(Arg::new("mint-tokens")
            .short('t')
            .long("mint-tokens")
            .action(ArgAction::SetTrue)
            .help("Mint some tokens"))
        .arg(Arg::new("create-token-metadata")
            .short('d')
            .long("create-token-metadata")
            .action(ArgAction::SetTrue)
            .help("Create some token metadata"))
        .get_matches();
        
    if matches.get_flag("generate-keypair") {
        generate_keypair();
    } else if matches.get_flag("load-keypair") {
        load_keypair();
    } else if matches.get_flag("check-balance") {
        check_balance().await;
    } else if matches.get_flag("find-keypair") {
        find_keypair("Lev", 3);
    } else if matches.get_flag("send-sol") {
        if let Err(e) = send_sol() {
            println!("Sending SOL failed due to: {:?}", e);
        }
    } else if matches.get_flag("create-token-mint") {
        if let Err(e) = create_token_mint() {
            println!("Creating token mint failed due to: {:?}", e);
        }
    } else if matches.get_flag("create-token-account") {
        if let Err(e) = create_token_account() {
            println!("Creating token account failed due to: {:?}", e);
        }
    } else if matches.get_flag("mint-tokens") {
        if let Err(e) = mint_tokens() {
            println!("Minting tokens failed due to: {:?}", e);
        }
    } else if matches.get_flag("create-token-metadata") {
        if let Err(e) = create_token_metadata() {
            println!("Creating token metadata failed due to: {:?}", e);
        }
    }
}

fn generate_keypair() {
    let keypair = Keypair::new();
    println!("The public key is: {}", bs58::encode(keypair.pubkey()).into_string());
    println!("The secret key is: {:?}", keypair.to_bytes());
    println!("✅ Finished!");
}

fn load_keypair_from_env() -> Keypair {
    dotenv().expect(".env file not found");
    let private_key = env::var("SECRET_KEY").expect("Add SECRET_KEY to .env!");
    let as_array: Vec<u8> = serde_json::from_str(&private_key)
        .expect("Failed to parse SECRET_KEY from .env");
    Keypair::from_bytes(&as_array).expect("Failed to create Keypair from secret key")
}

fn load_keypair() {
    let keypair = load_keypair_from_env();
    println!("Public key: {}", bs58::encode(keypair.pubkey()).into_string());
}

fn create_connection() -> RpcClient {
    RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    )
}

async fn check_balance() {
    let connection = create_connection();
    println!("⚡️ Connected to devnet");
    let public_key = Pubkey::from_str("8cUNp6LJGfjN3M1mwk537CfY2WBtYUYQNnf4hVtPx7AB").unwrap();
    
    if let Err(e) = airdrop_if_required(&connection, &public_key, 0.5, 1.5).await {
        println!("Airdrop failed due to: {:?}", e);
    }
    
    let balance_in_lamports = connection.get_balance(&public_key).unwrap();
    let balance_in_sol = balance_in_lamports as f64 / LAMPORTS_PER_SOL as f64;
    println!(
        "💰 The balance for the wallet at address {} is: {} SOL",
        public_key, balance_in_sol
    );
}

async fn airdrop_if_required(
    connection: &RpcClient,
    public_key: &Pubkey,
    airdrop_amount: f64,
    min_balance: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_balance = connection.get_balance(public_key)?;
    if current_balance < (min_balance * LAMPORTS_PER_SOL as f64) as u64 {
        println!("Requesting airdrop...");

        let signature = connection
            .request_airdrop(public_key, (airdrop_amount * LAMPORTS_PER_SOL as f64) as u64)?;

        loop {
            let commitment_config = CommitmentConfig::processed();
            let confirmed = connection.confirm_transaction_with_commitment(&signature, commitment_config)?;
            if confirmed.value {
                break;
            }
        }

        println!("Airdrop complete");
    } else {
        println!("No airdrop required");
    }
    Ok(())
}

fn find_keypair(prefix: &str, max_minutes: u64) {
    let start_time = Instant::now();
    let max_duration = Duration::from_secs(max_minutes * 60);

    loop {
        if start_time.elapsed() > max_duration {
            println!("⏰ Time out! The public key starting with '{}' was not found within {} minutes.", prefix, max_minutes);
            break;
        }
        let keypair = Keypair::new();
        let public_key_base58 = bs58::encode(keypair.pubkey()).into_string();

        if public_key_base58.starts_with(prefix) {
            let elapsed_time = start_time.elapsed();
            println!("⌛ Found matching keypair in {} second(s) or {:.2} minute(s)!",
                elapsed_time.as_secs(),
                elapsed_time.as_secs_f64() / 60.0
            );
            println!("The public key is: {}", public_key_base58);
            println!("The secret key is: {:?}", keypair.to_bytes());
            println!("✅ Finished!");
            break;
        }
    }
}

fn send_sol() -> Result<(), Box<dyn std::error::Error>> {
    let sender = load_keypair_from_env();
 
    let connection = create_connection();
    println!("🔑 Our public key is: {}", sender.pubkey());

    let recipient = Pubkey::from_str("8cUNp6LJGfjN3M1mwk537CfY2WBtYUYQNnf4hVtPx7AB").unwrap();
    println!("💸 Attempting to send 0.01 SOL to {}...", recipient);

    let transfer_instruction = system_instruction::transfer(&sender.pubkey(), &recipient, (0.01 * LAMPORTS_PER_SOL as f64) as u64);

    let memo_program_id = Pubkey::from_str("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr")?;
    let memo_text = "Hello from Solana!";
    let memo_instruction = solana_sdk::instruction::Instruction::new_with_bytes(
        memo_program_id,
        memo_text.as_bytes(),
        vec![],
    );

    let mut transaction = Transaction::new_with_payer(
        &[transfer_instruction, memo_instruction],
        Some(&sender.pubkey()),
    );

    println!("📝 memo is: {}", memo_text);
    
    let recent_blockhash = connection.get_latest_blockhash()?;
    transaction.sign(&[&sender], recent_blockhash);

    let signature = connection.send_and_confirm_transaction_with_spinner_and_commitment(
        &transaction,
        CommitmentConfig::processed(),
    )?;

    println!("✅ Transaction confirmed, signature: {}!", signature);
    
    Ok(())
}

fn create_token_mint() -> Result<(), Box<dyn std::error::Error>> {
    let sender = load_keypair_from_env();
 
    let connection = create_connection();
    println!("🔑 Our public key is: {}", sender.pubkey());

    let mint_pubkey = create_mint(
        &connection,
        &sender,
        &sender.pubkey(),
        None,
        2,
    )?;
    
    let explorer_link = format!(
        "https://explorer.solana.com/address/{}?cluster=devnet",
        mint_pubkey
    );

    println!("✅ Token Mint: {}", explorer_link);

    Ok(())
}

fn create_mint(
    connection: &RpcClient,
    payer: &Keypair,
    mint_authority: &Pubkey,
    freeze_authority: Option<&Pubkey>,
    decimals: u8,
) -> Result<Pubkey, Box<dyn std::error::Error>> {
    let mint_account = Keypair::new();
    let mint_pubkey = mint_account.pubkey();
    let mint_rent_exempt_balance = connection.get_minimum_balance_for_rent_exemption(Mint::LEN)?;

    let create_account_instruction = solana_sdk::system_instruction::create_account(
        &payer.pubkey(),
        &mint_pubkey,
        mint_rent_exempt_balance,
        Mint::LEN as u64,
        &spl_token::id(),
    );

    let mint_instruction = initialize_mint(
        &spl_token::id(),
        &mint_pubkey,
        mint_authority,
        freeze_authority,
        decimals,
    )?;

    let transaction = Transaction::new_signed_with_payer(
        &[create_account_instruction, mint_instruction],
        Some(&payer.pubkey()),
        &[payer, &mint_account],
        connection.get_latest_blockhash()?,
    );

    connection.send_and_confirm_transaction(&transaction)?;

    Ok(mint_pubkey)
}

fn create_token_account() -> Result<(), Box<dyn std::error::Error>> {
    let sender = load_keypair_from_env();
 
    let connection = create_connection();
    println!("🔑 Our public key is: {}", sender.pubkey());

    let token_mint_account = Pubkey::from_str("ExJmrjcJj3FuHNvswLkLmAxiEBGcdW5g9WnZqb8VjCiz").unwrap();
    let recipient = Pubkey::from_str("8cUNp6LJGfjN3M1mwk537CfY2WBtYUYQNnf4hVtPx7AB").unwrap();

    let account_pubkey = get_or_create_associated_token_account(&connection, &sender, &token_mint_account, &recipient)?;

    println!("Token Account: {}", account_pubkey);

    let explorer_link = format!(
        "https://explorer.solana.com/address/{}?cluster=devnet",
        account_pubkey
    );

    println!("✅ Created token account: {}", explorer_link);

    Ok(())
}

fn get_or_create_associated_token_account(
    connection: &RpcClient,
    sender: &Keypair,
    mint: &Pubkey,
    recipient: &Pubkey,
) -> Result<Pubkey, Box<dyn std::error::Error>> {
    let associated_token_address = get_associated_token_address(recipient, mint);

    if connection.get_account(&associated_token_address).is_err() {
        let create_ata_instruction = create_associated_token_account(
            &sender.pubkey(),
            recipient,
            mint,
            &spl_token::id(),
        );

        let transaction = Transaction::new_signed_with_payer(
            &[create_ata_instruction],
            Some(&sender.pubkey()),
            &[sender],
            connection.get_latest_blockhash()?,
        );

        connection.send_and_confirm_transaction(&transaction)?;
    }

    Ok(associated_token_address)
}

fn mint_tokens() -> Result<(), Box<dyn std::error::Error>> {
    let sender = load_keypair_from_env();

    let connection = create_connection();
    
    const MINOR_UNITS_PER_MAJOR_UNITS: u64 = 10_u64.pow(2);

    let token_mint_account = Pubkey::from_str("ExJmrjcJj3FuHNvswLkLmAxiEBGcdW5g9WnZqb8VjCiz").unwrap();

    let recipient_associated_token_account = Pubkey::from_str("CtWYrszfioSrDA8G9GTGMmwjcs1J6LFzTVkkByT5daYy").unwrap();

    let mint_to_instruction = mint_to(
        &spl_token::id(),
        &token_mint_account,
        &recipient_associated_token_account,
        &sender.pubkey(),
        &[],
        10 * MINOR_UNITS_PER_MAJOR_UNITS,
    )?;

    let mut transaction = Transaction::new_with_payer(
        &[mint_to_instruction],
        Some(&sender.pubkey()),
    );

    let recent_blockhash = connection.get_latest_blockhash()?;
    transaction.sign(&[&sender], recent_blockhash);
    let signature = connection.send_and_confirm_transaction(&transaction)?;

    let explorer_link = format!(
        "https://explorer.solana.com/transaction/{}?cluster=devnet",
        signature
    );

    println!("✅ Success! Mint Token Transaction: {}", explorer_link);

    Ok(())
}

fn create_token_metadata() -> Result<(), Box<dyn std::error::Error>> {
    let user = load_keypair_from_env();

    let connection = create_connection();
    
    let token_metadata_program_id = Pubkey::from_str("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s").unwrap();

    let token_mint_account = Pubkey::from_str("ExJmrjcJj3FuHNvswLkLmAxiEBGcdW5g9WnZqb8VjCiz").unwrap();

    let (metadata_pda, _bump) = Pubkey::find_program_address(
        &[
            b"metadata",
            token_metadata_program_id.as_ref(),
            token_mint_account.as_ref(),
        ],
        &token_metadata_program_id,
    );

    let metadata_data = DataV2 {
        name: "Solana UA Bootcamp 2024-08-06".to_string(),
        symbol: "UAB-2".to_string(),
        uri: "https://arweave.net/1234".to_string(),
        seller_fee_basis_points: 0,
        creators: None,
        collection: None,
        uses: None,
    };

    let create_metadata_account_instruction = CreateMetadataAccountV3 {
        metadata: metadata_pda,
        mint: token_mint_account,
        mint_authority: user.pubkey(),
        payer: user.pubkey(),
        update_authority: (user.pubkey(), true),
        system_program: system_program::ID,
        rent: None,
    };
    let create_metadata_account_instruction = create_metadata_account_instruction.instruction(
        CreateMetadataAccountV3InstructionArgs {
            data: metadata_data,
            is_mutable: true,
            collection_details: None,
        }
    );
    
    let mut transaction = Transaction::new_with_payer(
        &[create_metadata_account_instruction],
        Some(&user.pubkey()),
    );

    let recent_blockhash = connection.get_latest_blockhash()?;
    transaction.sign(&[&user], recent_blockhash);

    let _signature = connection.send_and_confirm_transaction(&transaction)?;

    let explorer_link = format!(
        "https://explorer.solana.com/address/{}?cluster=devnet",
        token_mint_account
    );

    println!("✅ Look at the token mint again: {}", explorer_link);

    Ok(())
}
