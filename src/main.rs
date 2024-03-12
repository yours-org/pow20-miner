use anyhow::Result;
use clap::Parser;
use rand::Rng;
use rayon::prelude::*;
use serde::*;
use serde_json::*;
use std::{sync::Arc, time::Instant};
use tokio::sync::Mutex;

mod api;
pub use api::*;
mod hash;
pub use hash::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[derive(Clone)]
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
    pub challenge: Vec<u8>,
}

#[derive(Clone, Default)]
pub struct Stats {
    pub accepted: i64,
    pub rejected: i64,
}

type Address = bitcoin::Address<bitcoin::address::NetworkUnchecked>;

#[derive(Clone)]
pub struct Context {
    work: Arc<Mutex<Ticker>>,
    stats: Arc<Mutex<Stats>>,
    api_client: ApiClient,
    args: Args,
}

pub async fn update_work(ctx: &Context) -> () {
    let mut lock = ctx.work.lock().await;

    if let Ok(new_work) = ctx.api_client.fetch_ticker(&ctx.args.tick).await {
        if lock.challenge != new_work.challenge {
            *lock = new_work;
            println!(
                "new job! ticker: {:?} difficulty: {:?}",
                lock.ticker, lock.difficulty,
            );
        }
    }
    drop(lock);
}

pub async fn submit_work(solution: &Solution, ctx: &Context) -> () {
    let submit_res = ctx.api_client.submit_share(solution).await;

    println!(
        "[{}] found solution! submitting... submit solution\n\tnonce: {:?}\n\thash: {:?}\n\tlocation: {:?}\n\tchallenge: {:?}",
        hex::encode(&solution.challenge[0..4]),
        solution.nonce,
        solution.hash,
        solution.location,
        hex::encode(&solution.challenge)
    );

    if let Ok((status_code, response)) = &submit_res {
        let mut stats_lock = ctx.stats.lock().await;

        if status_code.clone() == 201 {
            stats_lock.accepted = stats_lock.accepted + 1;
            println!(
                "[{}] ✅ accepted share",
                hex::encode(&solution.challenge[0..4])
            )
        } else {
            stats_lock.rejected = stats_lock.rejected + 1;

            println!(
                "[{}] ❌ rejected share {:?}",
                hex::encode(&solution.challenge[0..4]),
                response
            )
        }

        drop(stats_lock)
    }

    if let Err(r) = submit_res {
        println!("❌ reject share: {}", r)
    }

    update_work(ctx).await;
}

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

    let token = match api_client.fetch_ticker(&args.tick).await {
        Ok(v) => v,
        Err(e) => {
            println!("failed to fetch tick: {:?}", args.tick);
            println!("{:?}", e);
            return Ok(());
        }
    };

    let work = Arc::new(Mutex::new(token.clone()));

    let ctx = Context {
        work,
        stats: Arc::new(Mutex::new(Stats::default())),
        api_client: api_client.clone(),
        args: args.clone(),
    };

    println!(
        "new job! ticker: {:?} difficulty: {:?}",
        token.ticker, token.difficulty
    );

    let cloned = ctx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            update_work(&cloned).await;
        }
    });

    let mut nonce: u16 = 1;
    let bucket = (0..8_000_000).collect::<Vec<u32>>();

    loop {
        let start_time = Instant::now();

        let work_lock = ctx.work.lock().await;
        let work = work_lock.clone();
        drop(work_lock);

        let mut challenge_bytes = hex::decode(work.challenge.clone()).unwrap();
        challenge_bytes.reverse();

        let results = bucket
            .par_iter()
            .map(|prefix| {
                let random = rand::thread_rng().gen::<[u8; 4]>();

                let mut data = [0; 8];
                data[..4].copy_from_slice(&prefix.to_le_bytes());
                data[4..].copy_from_slice(&random);

                let mut preimage = [0_u8; 64];
                preimage[..challenge_bytes.len()].copy_from_slice(&challenge_bytes);
                preimage[challenge_bytes.len()..challenge_bytes.len() + 8].copy_from_slice(&data);

                let solution = Hash::sha256d(&preimage[..challenge_bytes.len() + 8]);

                for i in 0..work.difficulty {
                    let rshift = (1 - (i % 2)) << 2;
                    if (solution[(i / 2) as usize] >> rshift) & 0x0f != 0 {
                        return None;
                    }
                }

                return Some(Solution {
                    nonce: hex::encode(data),
                    hash: hex::encode(solution),
                    location: work.current_location.clone(),
                    token_id: work.id.clone(),
                    challenge: challenge_bytes.clone(),
                });
            })
            .filter_map(|e| match e {
                Some(e) => Some(e),
                None => None,
            })
            .collect::<Vec<_>>();

        let duration = start_time.elapsed().as_millis();
        let stats_lock = ctx.stats.lock().await;
        let stats = stats_lock.clone();
        drop(stats_lock);

        println!(
            "[{}] diff: {} accepted: {} rejected: {} hash: {:.2} MH/s",
            hex::encode(&challenge_bytes[0..4]),
            work.difficulty,
            stats.accepted,
            stats.rejected,
            bucket.len() as f64 / 1000.0 / duration as f64
        );

        if results.len() > 0 {
            submit_work(&results[0], &ctx).await;
        }

        nonce = nonce + 1;
    }
}
