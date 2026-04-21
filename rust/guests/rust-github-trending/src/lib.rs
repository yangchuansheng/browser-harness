use bh_guest_sdk::{ensure_real_tab, goto, js, page_info, wait, wait_for_load};
use serde::Deserialize;
use serde_json::Value;

const TARGET_URL: &str = "https://github.com/trending";
const HYDRATION_WAIT_MS: u64 = 2_000;
const MIN_TRENDING_ROWS: usize = 5;
const EXTRACTION_SCRIPT: &str = r#"
(function(){
  var rows = Array.from(document.querySelectorAll('article.Box-row'));
  return JSON.stringify(rows.map(function(el){
    var h2link = el.querySelector('h2 a');
    var starLink = el.querySelector('a[href*="/stargazers"]');
    var forkLink = el.querySelector('a[href*="/forks"]');
    var langEl = el.querySelector('[itemprop="programmingLanguage"]');
    var todayEl = el.querySelector('.d-inline-block.float-sm-right');
    var descEl = el.querySelector('p');
    return {
      name: h2link ? h2link.innerText.trim().replace(/\s+/g,' ') : null,
      url: h2link ? 'https://github.com' + h2link.getAttribute('href') : null,
      stars_total: starLink ? starLink.innerText.trim() : null,
      stars_period: todayEl ? todayEl.innerText.trim() : null,
      forks: forkLink ? forkLink.innerText.trim() : null,
      language: langEl ? langEl.innerText.trim() : null,
      desc: descEl ? descEl.innerText.trim() : null
    };
  }));
})()
"#;

#[derive(Debug, Deserialize)]
struct TrendingRepo {
    name: Option<String>,
    url: Option<String>,
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

    let slept = wait(HYDRATION_WAIT_MS).map_err(|_| 5)?;
    if slept.elapsed_ms < HYDRATION_WAIT_MS {
        return Err(6);
    }

    let page = page_info().map_err(|_| 7)?;
    let page_url = page.get("url").and_then(Value::as_str).unwrap_or("");
    if !page_url.contains("github.com/trending") {
        return Err(8);
    }

    let raw_repos: String = js(EXTRACTION_SCRIPT).map_err(|_| 9)?;
    let repos: Vec<TrendingRepo> = serde_json::from_str(&raw_repos).map_err(|_| 10)?;
    if repos.len() < MIN_TRENDING_ROWS {
        return Err(11);
    }

    let first = repos.first().ok_or(12)?;
    let first_name = first.name.as_deref().unwrap_or("").trim();
    let first_url = first.url.as_deref().unwrap_or("");
    if first_name.is_empty() {
        return Err(13);
    }
    if !first_url.starts_with("https://github.com/") {
        return Err(14);
    }

    Ok(())
}
