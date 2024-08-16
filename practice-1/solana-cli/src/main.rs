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

#[tokio::main]
async fn main() {
    let matches = Command::new("Solana CLI")
        .version("0.1.0")
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
        .get_matches();
        
    if matches.get_flag("generate-keypair") {
        generate_keypair();
    } else if matches.get_flag("load-keypair") {
        load_keypair();
    } else if matches.get_flag("check-balance") {
        check_balance().await;
    } else if matches.get_flag("find-keypair") {
        find_keypair("Lev", 3);
    }
}

fn generate_keypair() {
    let keypair = Keypair::new();
    println!("The public key is: {}", bs58::encode(keypair.pubkey()).into_string());
    println!("The secret key is: {:?}", keypair.to_bytes());
    println!("✅ Finished!");
}

fn load_keypair() {
    dotenv().expect(".env file not found");
    let private_key = env::var("SECRET_KEY").expect("Add SECRET_KEY to .env!");
    let as_array: Vec<u8> = serde_json::from_str(&private_key)
        .expect("Failed to parse SECRET_KEY from .env");
    let keypair = Keypair::from_bytes(&as_array).expect("Failed to create Keypair from secret key");
    println!("Public key: {}", bs58::encode(keypair.pubkey()).into_string());
}

async fn check_balance() {
    let connection = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );
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
