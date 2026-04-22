use bh_guest_sdk::http_get;
use serde::de::DeserializeOwned;
use serde::Deserialize;

const API_KEY: &str = "1MOZgmNFxvmljaQR1X9KAij9Mo4xAY3u";
const PRODUCT_URL: &str = "https://backend.metacritic.com/games/metacritic/the-last-of-us/web?componentName=product&componentType=Product&apiKey=1MOZgmNFxvmljaQR1X9KAij9Mo4xAY3u";
const USER_STATS_URL: &str = "https://backend.metacritic.com/reviews/metacritic/user/games/the-last-of-us/stats/web?componentName=user-score-summary&componentType=ScoreSummary&apiKey=1MOZgmNFxvmljaQR1X9KAij9Mo4xAY3u";

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    data: ApiData<T>,
}

#[derive(Debug, Deserialize)]
struct ApiData<T> {
    item: T,
}

#[derive(Debug, Deserialize)]
struct ProductItem {
    title: String,
    platform: String,
    #[serde(rename = "criticScoreSummary")]
    critic_score_summary: ScoreSummary,
}

#[derive(Debug, Deserialize)]
struct ScoreSummary {
    score: Option<i64>,
    #[serde(rename = "reviewCount")]
    review_count: i64,
    sentiment: String,
}

#[derive(Debug, Deserialize)]
struct UserScoreSummary {
    score: Option<f64>,
    #[serde(rename = "reviewCount")]
    review_count: i64,
    sentiment: String,
}

#[no_mangle]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run_inner() -> Result<(), i32> {
    let product: ApiResponse<ProductItem> = fetch_json(PRODUCT_URL, 1, 2)?;
    let user_stats: ApiResponse<UserScoreSummary> = fetch_json(USER_STATS_URL, 3, 4)?;
    let product = product.data.item;
    let user_stats = user_stats.data.item;

    if product.title != "The Last of Us" {
        return Err(5);
    }
    if product.platform.trim().is_empty() {
        return Err(6);
    }
    if product.critic_score_summary.score.unwrap_or_default() < 90 {
        return Err(7);
    }
    if product.critic_score_summary.review_count < 50 {
        return Err(8);
    }
    if product.critic_score_summary.sentiment.trim().is_empty() {
        return Err(9);
    }
    if user_stats.score.unwrap_or_default() < 8.0 {
        return Err(10);
    }
    if user_stats.review_count < 1_000 {
        return Err(11);
    }
    if user_stats.sentiment.trim().is_empty() {
        return Err(12);
    }
    if API_KEY.len() < 10 {
        return Err(13);
    }

    Ok(())
}

fn fetch_json<T: DeserializeOwned>(
    url: &str,
    fetch_err: i32,
    parse_err: i32,
) -> Result<T, i32> {
    let body = http_get(url, None, Some(20.0)).map_err(|_| fetch_err)?;
    serde_json::from_str(&body).map_err(|_| parse_err)
}
