use bh_guest_sdk::{js, new_tab, page_info, wait, wait_for_load};
use serde::Deserialize;
use serde_json::Value;

const TARGET_URL: &str = "https://www.producthunt.com/";
const TARGET_URL_PREFIX: &str = "https://www.producthunt.com";
const HYDRATION_WAIT_MS: u64 = 4_000;
const RETRY_WAIT_MS: u64 = 2_000;
const EXTRACTION_ATTEMPTS: usize = 4;
const MIN_PRODUCTS: usize = 20;
const EXTRACTION_SCRIPT: &str = r#"
JSON.stringify((() => {
  function normalizeName(text) {
    return (text || '').trim().replace(/^\d+\.\s*/, '');
  }

  function textLines(node) {
    return ((node && (node.innerText || node.textContent)) || '')
      .split('\n')
      .map(line => line.trim())
      .filter(Boolean);
  }

  function extractFromPostItems() {
    return Array.from(document.querySelectorAll('[data-test^="post-item-"]')).map(el => {
      var id = (el.getAttribute('data-test') || '').replace('post-item-', '');
      var nameEl = el.querySelector('[data-test^="post-name-"]');
      var productLink = el.querySelector('a[href^="/products/"]');
      var voteBtn = el.querySelector('[data-test="vote-button"]');
      var voteCount = voteBtn ? voteBtn.textContent.trim() : null;
      var topicLinks = Array.from(el.querySelectorAll('a[href^="/topics/"]')).map(a => a.textContent.trim()).filter(Boolean);
      var rawName = nameEl ? nameEl.textContent.trim() : '';
      var name = normalizeName(rawName);
      var lines = textLines(el);
      var tagline = lines.find(line =>
        line !== rawName &&
        line !== name &&
        !topicLinks.includes(line) &&
        line !== '•' &&
        !/^[0-9,—]+$/.test(line) &&
        line.length > 5
      );
      return {
        id: id,
        name: name,
        slug: productLink ? productLink.getAttribute('href') : null,
        votes: voteCount,
        topics: topicLinks,
        tagline: tagline || null
      };
    });
  }

  function findCardContainer(anchor) {
    for (var node = anchor; node && node !== document.body; node = node.parentElement) {
      var voteButtons = node.querySelectorAll('[data-test="vote-button"]');
      var productLinks = node.querySelectorAll('a[href^="/products/"]');
      if (voteButtons.length === 1 && productLinks.length >= 1 && productLinks.length <= 3) {
        return node;
      }
    }
    return anchor.parentElement || anchor;
  }

  function extractFromProductLinks() {
    var seen = new Set();
    var anchors = document.querySelectorAll('[data-test^="homepage-section-"] a[href^="/products/"]');
    if (!anchors.length) {
      anchors = document.querySelectorAll('a[href^="/products/"]');
    }
    return Array.from(anchors).map(anchor => {
      var href = anchor.getAttribute('href') || '';
      var normalizedHref = href.split('?')[0].split('#')[0].replace(/\/$/, '');
      if (!normalizedHref || seen.has(normalizedHref)) {
        return null;
      }
      var rawLabel = (anchor.textContent || '').trim();
      if (!rawLabel) {
        return null;
      }

      var container = findCardContainer(anchor);
      var voteBtn = container.querySelector('[data-test="vote-button"]');
      if (!voteBtn) {
        return null;
      }

      var topics = Array.from(container.querySelectorAll('a[href^="/topics/"]')).map(a => a.textContent.trim()).filter(Boolean);
      var lines = textLines(container);
      var name = normalizeName(rawLabel);
      if (!name) {
        name = normalizeName(lines.find(line => /^\d+\.\s+/.test(line)) || lines[0] || '');
      }
      if (!name) {
        return null;
      }

      var voteText = voteBtn.textContent ? voteBtn.textContent.trim() : null;
      var tagline = lines.find(line =>
        line !== rawLabel &&
        line !== name &&
        normalizeName(line) !== name &&
        !topics.includes(line) &&
        line !== '•' &&
        !/^[0-9,—]+$/.test(line) &&
        line.length > 10 &&
        !/^Promoted$/i.test(line) &&
        !/^Top Products /i.test(line) &&
        !/^Welcome to Product Hunt/i.test(line)
      );

      seen.add(normalizedHref);
      return {
        id: normalizedHref.replace(/^\/products\//, ''),
        name: name,
        slug: normalizedHref,
        votes: voteText,
        topics: topics,
        tagline: tagline || null
      };
    }).filter(Boolean);
  }

  var postItems = document.querySelectorAll('[data-test^="post-item-"]');
  if (postItems.length > 0) {
    return extractFromPostItems();
  }
  return extractFromProductLinks();
})())
"#;

#[derive(Debug, Deserialize)]
struct ProductHuntProduct {
    id: String,
    name: String,
    slug: Option<String>,
    votes: Option<String>,
    topics: Vec<String>,
    tagline: Option<String>,
}

#[no_mangle]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run_inner() -> Result<(), i32> {
    let new_tab_result = new_tab(TARGET_URL).map_err(|_| 1)?;
    if new_tab_result.target_id.trim().is_empty() {
        return Err(2);
    }

    let loaded = wait_for_load(15.0).map_err(|_| 3)?;
    if !loaded {
        return Err(4);
    }

    let slept = wait(HYDRATION_WAIT_MS).map_err(|_| 5)?;
    if slept.elapsed_ms < HYDRATION_WAIT_MS {
        return Err(6);
    }

    let page = page_info().map_err(|_| 7)?;
    let page_url = page.get("url").and_then(Value::as_str).unwrap_or("");
    if !page_url.starts_with(TARGET_URL_PREFIX) {
        return Err(8);
    }

    let mut products = Vec::new();
    for attempt in 0..EXTRACTION_ATTEMPTS {
        products = extract_products()?;
        if products.len() >= MIN_PRODUCTS {
            break;
        }
        if attempt + 1 < EXTRACTION_ATTEMPTS {
            let slept = wait(RETRY_WAIT_MS).map_err(|_| 18)?;
            if slept.elapsed_ms < RETRY_WAIT_MS {
                return Err(19);
            }
        }
    }
    if products.len() < MIN_PRODUCTS {
        return Err(11);
    }

    let first = products.first().ok_or(12)?;
    if first.id.trim().is_empty() {
        return Err(13);
    }
    if first.name.trim().is_empty() {
        return Err(14);
    }
    if !first
        .slug
        .as_deref()
        .unwrap_or("")
        .starts_with("/products/")
    {
        return Err(15);
    }
    if products
        .iter()
        .all(|product| product.topics.is_empty() && product.tagline.as_deref().unwrap_or("").is_empty())
    {
        return Err(16);
    }
    if products.iter().all(|product| {
        product
            .votes
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    }) {
        return Err(17);
    }

    Ok(())
}

fn extract_products() -> Result<Vec<ProductHuntProduct>, i32> {
    let raw_products: String = js(EXTRACTION_SCRIPT).map_err(|_| 9)?;
    serde_json::from_str(&raw_products).map_err(|_| 10)
}
