use bh_guest_sdk::{js, new_tab, page_info, wait, wait_for_load};
use serde::Deserialize;
use serde_json::Value;

const TARGET_URL: &str = "https://www.etsy.com/search?q=handmade+candle&explicit=1";
const TARGET_URL_PREFIX: &str = "https://www.etsy.com/search";
const HYDRATION_WAIT_MS: u64 = 3_000;
const RETRY_WAIT_MS: u64 = 2_000;
const EXTRACTION_ATTEMPTS: usize = 4;
const MIN_RESULTS: usize = 24;
const EXTRACTION_SCRIPT: &str = r#"
(function() {
  function extractFromJsonLd() {
    var scripts = Array.from(document.querySelectorAll('script[type="application/ld+json"]'));
    for (var i = 0; i < scripts.length; i++) {
      try {
        var data = JSON.parse(scripts[i].textContent);
        if (data && data['@type'] === 'ItemList' && Array.isArray(data.itemListElement)) {
          return data.itemListElement.map(function(item) {
            return {
              position: item.position || null,
              url: item.url || null,
              name: item.name || null
            };
          }).filter(function(item) {
            return item.url && item.name;
          });
        }
      } catch (err) {}
    }
    return [];
  }

  var jsonLdItems = extractFromJsonLd();
  if (jsonLdItems.length) {
    return JSON.stringify(jsonLdItems);
  }

  return JSON.stringify(
    Array.from(document.querySelectorAll('[data-listing-id]')).map(function(el) {
      var link = el.querySelector('a[href*="/listing/"]');
      var title = el.querySelector('h3, h2');
      return {
        position: null,
        url: link ? link.href : null,
        name: title ? title.innerText.trim() : null
      };
    }).filter(function(item) {
      return item.url && item.name;
    })
  );
})()
"#;

#[derive(Debug, Deserialize)]
struct SearchItem {
    position: Option<u64>,
    url: Option<String>,
    name: Option<String>,
}

#[no_mangle]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run_inner() -> Result<(), i32> {
    let tab = new_tab(TARGET_URL).map_err(|_| 1)?;
    if tab.target_id.trim().is_empty() {
        return Err(2);
    }

    let loaded = wait_for_load(15.0).map_err(|_| 3)?;
    if !loaded {
        return Err(4);
    }

    let initial_wait = wait(HYDRATION_WAIT_MS).map_err(|_| 5)?;
    if initial_wait.elapsed_ms < HYDRATION_WAIT_MS {
        return Err(6);
    }

    let page = page_info().map_err(|_| 7)?;
    let page_url = page.get("url").and_then(Value::as_str).unwrap_or("");
    if !page_url.starts_with(TARGET_URL_PREFIX) {
        return Err(8);
    }

    let mut items = Vec::new();
    for attempt in 0..EXTRACTION_ATTEMPTS {
        items = extract_items()?;
        if items.len() >= MIN_RESULTS {
            break;
        }
        if attempt + 1 < EXTRACTION_ATTEMPTS {
            let waited = wait(RETRY_WAIT_MS).map_err(|_| 16)?;
            if waited.elapsed_ms < RETRY_WAIT_MS {
                return Err(17);
            }
        }
    }

    if items.len() < MIN_RESULTS {
        return Err(9);
    }

    let first = items.first().ok_or(10)?;
    if first.name.as_deref().unwrap_or("").trim().is_empty() {
        return Err(11);
    }
    let first_url = first.url.as_deref().unwrap_or("").trim();
    if !first_url.starts_with("https://www.etsy.com/listing/") {
        return Err(12);
    }
    if let Some(position) = first.position {
        if position == 0 {
            return Err(13);
        }
    }

    Ok(())
}

fn extract_items() -> Result<Vec<SearchItem>, i32> {
    let raw_items: String = js(EXTRACTION_SCRIPT).map_err(|_| 14)?;
    serde_json::from_str(&raw_items).map_err(|_| 15)
}
