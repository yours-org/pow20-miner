use anyhow::Result;
use clap::Parser;
use rand::Rng;
use rayon::prelude::*;
use serde::*;
use serde_json::*;
use std::time::Instant;

mod api;
pub use api::*;
mod hash;
pub use hash::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    tick: String,
    #[arg(short, long)]
    address: String,
}

#[derive(Debug)]
pub struct Solution {
    pub nonce: String,
    pub hash: String,
    pub location: String,
    pub token_id: String,
}

type Address = bitcoin::Address<bitcoin::address::NetworkUnchecked>;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if let Err(_) = args.address.parse::<Address>() {
        println!("failed to parse address: {}", args.address);
        return Ok(());
    }

    let api_client = ApiClient {
        url: "http://api.pow20.io".to_string(),
        address: args.address.to_string(),
    };

    let mut token = match api_client.fetch_ticker(&args.tick).await {
        Ok(v) => v,
        Err(e) => {
            println!("failed to fetch tick: {:?}", args.tick);
            println!("{:?}", e);
            return Ok(());
        }
    };

    let mut current_difficulty = vec![0; token.difficulty as usize]
        .iter()
        .map(|_| "0")
        .collect::<String>();

    let mut token_challenge = hex::decode(token.challenge.clone()).unwrap();
    token_challenge.reverse();
    let mut current_challenge = hex::encode(token_challenge);

    println!(
        "new job! ticker: {:?} difficulty: {:?} challenge: {:?}",
        token.ticker, current_difficulty, current_challenge
    );

    let mut nonce: u16 = 1;
    let bucket = vec![0; 8_000_000];

    loop {
        let start_time = Instant::now();

        let challenge_bytes = hex::decode(&current_challenge).unwrap();

        let results = bucket
            .par_iter()
            .map(|_| {
                let data = rand::thread_rng().gen::<[u8; 4]>();

                let mut preimage = [0_u8; 1024];
                preimage[..challenge_bytes.len()].copy_from_slice(&challenge_bytes);
                preimage[challenge_bytes.len()..challenge_bytes.len() + 4].copy_from_slice(&data);

                let solution = Hash::sha256d(&preimage[..challenge_bytes.len() + 4]);

                for i in 0..token.difficulty {
                    let rshift = (1 - (i % 2)) << 2;
                    if (solution[(i / 2) as usize] >> rshift) & 0x0f != 0 {
                        return None;
                    }
                }

                return Some(Solution {
                    nonce: hex::encode(data),
                    hash: hex::encode(solution),
                    location: token.current_location.clone(),
                    token_id: token.id.clone(),
                });
            })
            .filter_map(|e| match e {
                Some(e) => Some(e),
                None => None,
            })
            .collect::<Vec<_>>();

        let duration = start_time.elapsed().as_millis();

        println!(
            "{:.2} MH/s",
            bucket.len() as f64 / 1000.0 / 100.0 / duration as f64 * 1000.0
        );

        let mut update_work = nonce % 12 == 0;

        for res in &results {
            update_work = true;

            println!("found share: {:?}", res);
            let submit_res = api_client.submit_share(res).await;

            if let Ok((status_code, response)) = &submit_res {
                if status_code.clone() == 201 {
                    println!("✅ accepted share")
                } else {
                    println!("❌ rejected share: {:#?}", response)
                }
            }

            if let Err(r) = submit_res {
                println!("❌ reject share: {}", r)
            }
        }

        if update_work {
            let res = api_client.fetch_ticker(&args.tick).await;

            if let Ok(t) = &res {
                if token.challenge != t.challenge {
                    token = t.clone();
                    current_difficulty = vec![0; token.difficulty as usize]
                        .iter()
                        .map(|_| "0")
                        .collect::<String>();
                    let mut token_challenge = hex::decode(token.challenge.clone()).unwrap();
                    token_challenge.reverse();
                    current_challenge = hex::encode(token_challenge);
                    println!(
                        "new job! ticker: {:?} difficulty: {:?} challenge: {:?}",
                        token.ticker, current_difficulty, current_challenge
                    );
                }
            }

            if let Err(e) = &res {
                println!("failed to fetch tick: {:?}", args.tick);
                println!("{}", e);
            }
        }

        nonce = nonce + 1;
    }
}
