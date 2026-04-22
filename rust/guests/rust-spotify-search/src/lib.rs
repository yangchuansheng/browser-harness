use bh_guest_sdk::{ensure_real_tab, goto, js, page_info, wait, wait_for_load};
use serde::Deserialize;
use serde_json::Value;

const TARGET_URL: &str = "https://open.spotify.com/search/never%20gonna%20give%20you%20up";
const SEARCH_WAIT_MS: u64 = 3_000;
const MIN_TRACK_RESULTS: usize = 4;
const EXTRACTION_SCRIPT: &str = r#"
(function() {
  var trackLinks = Array.from(document.querySelectorAll('a[href*="/track/"]'));
  var seen = new Set();
  var tracks = trackLinks.map(function(link) {
    var href = link.href || '';
    if (!href || seen.has(href)) {
      return null;
    }
    seen.add(href);
    var text = (link.innerText || '').trim();
    if (!text) {
      text = (link.getAttribute('aria-label') || '').trim();
    }
    return {
      href: href,
      text: text || null
    };
  }).filter(Boolean);

  return JSON.stringify({
    url: location.href,
    trackResults: tracks
  });
})()
"#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchPayload {
    url: String,
    track_results: Vec<SearchTrack>,
}

#[derive(Debug, Deserialize)]
struct SearchTrack {
    href: String,
    text: Option<String>,
}

#[no_mangle]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run_inner() -> Result<(), i32> {
    let _ = ensure_real_tab().map_err(|_| 1)?;

    goto(TARGET_URL).map_err(|_| 2)?;

    let loaded = wait_for_load(15.0).map_err(|_| 3)?;
    if !loaded {
        return Err(4);
    }

    let waited = wait(SEARCH_WAIT_MS).map_err(|_| 5)?;
    if waited.elapsed_ms < SEARCH_WAIT_MS {
        return Err(6);
    }

    let page = page_info().map_err(|_| 7)?;
    let page_url = page.get("url").and_then(Value::as_str).unwrap_or("");
    if !page_url.contains("open.spotify.com/search") {
        return Err(8);
    }

    let raw_payload: String = js(EXTRACTION_SCRIPT).map_err(|_| 9)?;
    let payload: SearchPayload = serde_json::from_str(&raw_payload).map_err(|_| 10)?;
    if payload.track_results.len() < MIN_TRACK_RESULTS {
        return Err(11);
    }
    if !payload.url.contains("open.spotify.com/search") {
        return Err(12);
    }

    let first = payload.track_results.first().ok_or(13)?;
    if !first.href.starts_with("https://open.spotify.com/track/") {
        return Err(14);
    }
    if first.text.as_deref().unwrap_or("").trim().is_empty() {
        return Err(15);
    }

    Ok(())
}
