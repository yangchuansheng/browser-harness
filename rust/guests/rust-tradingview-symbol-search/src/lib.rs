use bh_guest_sdk::http_get;
use serde::Deserialize;
use std::collections::BTreeMap;

const TARGET_URL: &str = "https://symbol-search.tradingview.com/symbol_search/v3/?text=AAPL&hl=1&exchange=NASDAQ&lang=en&search_type=stock&domain=production";

#[derive(Debug, Deserialize)]
struct SymbolSearchResponse {
    symbols_remaining: u64,
    symbols: Vec<SymbolItem>,
}

#[derive(Debug, Deserialize)]
struct SymbolItem {
    symbol: String,
    description: Option<String>,
    #[serde(rename = "type")]
    symbol_type: Option<String>,
    exchange: Option<String>,
    isin: Option<String>,
    currency_code: Option<String>,
    is_primary_listing: Option<bool>,
}

#[no_mangle]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run_inner() -> Result<(), i32> {
    let mut headers = BTreeMap::new();
    headers.insert("Origin".to_string(), "https://www.tradingview.com".to_string());
    let body = http_get(TARGET_URL, Some(headers), Some(20.0)).map_err(|_| 1)?;
    let response: SymbolSearchResponse = serde_json::from_str(&body).map_err(|_| 2)?;

    if !response.symbols_remaining.eq(&0) {
        return Err(3);
    }
    let first = response.symbols.first().ok_or(4)?;
    if !first.symbol.contains("AAPL") {
        return Err(5);
    }
    if first.description.as_deref() != Some("Apple Inc.") {
        return Err(6);
    }
    if first.symbol_type.as_deref() != Some("stock") {
        return Err(7);
    }
    if first.exchange.as_deref() != Some("NASDAQ") {
        return Err(8);
    }
    if first
        .isin
        .as_deref()
        .unwrap_or("")
        .strip_prefix("US")
        .is_none()
    {
        return Err(9);
    }
    if first.currency_code.as_deref() != Some("USD") {
        return Err(10);
    }
    if first.is_primary_listing != Some(true) {
        return Err(11);
    }

    Ok(())
}
