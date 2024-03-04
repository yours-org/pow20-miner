use super::*;
use std::path::Path;
use config::Config;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ticker {
    pub challenge: String,
    #[serde(rename(deserialize = "currentLocation"))]
    pub current_location: String,
    pub difficulty: i32,
    pub ticker: String,
    pub id: String,
}

#[derive(Debug)]
pub struct ApiClient {
    pub url: String,
    pub address: String,
}

impl ApiClient {
    pub fn new(url: String, address: String) -> ApiClient {
        ApiClient { url, address }
    }

    pub fn get(&self, path: String) -> reqwest::RequestBuilder {
        let client = reqwest::Client::new();
        client
            .get(format!("{}{}", self.url, path))
            .header("Address", self.address.clone())
            .header("Chain", "BSV")
            .header("Wallet", "PANDA")
    }

    pub fn post(&self, path: String) -> reqwest::RequestBuilder {
        let client = reqwest::Client::new();
        client
            .post(format!("{}{}", self.url, path))
            .header("Address", self.address.clone())
            .header("Chain", "BSV")
            .header("Wallet", "PANDA")
    }

    pub async fn submit_share(&self, solution: &Solution) -> Result<(u16, String)> {
        let payload = json!({
            "bsvContractLocation": solution.location,
            "nonce": solution.nonce,
            "tokenId": solution.token_id,
            "winningHash": solution.hash
        });

        println!("submit share: {:#?}", payload);

        let res = self
            .post(format!("/mint/save"))
            .json(&payload)
            .send()
            .await?;

        let status_code = res.status().as_u16();
        let text = res.text().await?;

        Ok((status_code, text))
    }

    
    pub async fn fetch_ticker_api(&self, slug: &String) -> Result<Ticker> {
        let res = self
            .get(format!("/token/search?ticker={}", slug))
            .send()
            .await?
            .json::<Value>()
            .await?;

        let ticker: Ticker = serde_json::from_value(res)?;

        Ok(ticker)
    }
    
    pub async fn fetch_ticker(&self, slug: &String) -> Result<Ticker> {
        // allow forcing a token using a config.json file
        // (for ex to force start even if API is down at the moment)
        let mut config = Config::default();
        config.merge(config::File::from(Path::new("config.json"))).ok();

        let ticker_config = match config.get::<Ticker>("ticker") {
            Ok(ticker_config) => ticker_config,
            Err(_) => self.fetch_ticker_api(slug).await?,
        };
        Ok(ticker_config)
    }
}
