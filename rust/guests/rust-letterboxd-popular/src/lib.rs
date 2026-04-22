use bh_guest_sdk::{ensure_real_tab, goto, js, page_info, wait, wait_for_load};
use serde::Deserialize;
use serde_json::Value;

const TARGET_URL: &str = "https://letterboxd.com/films/popular/";
const TARGET_URL_PREFIX: &str = "https://letterboxd.com/films/popular/";
const HYDRATION_WAIT_MS: u64 = 2_000;
const RETRY_WAIT_MS: u64 = 2_000;
const EXTRACTION_ATTEMPTS: usize = 4;
const MIN_POPULAR_FILMS: usize = 20;
const EXTRACTION_SCRIPT: &str = r#"
(function() {
  var seen = new Set();
  var posters = Array.from(document.querySelectorAll('[data-item-slug], [data-film-slug]'));
  return JSON.stringify(posters.map(function(poster) {
    var slug = (poster.dataset.itemSlug || poster.dataset.filmSlug || '').trim();
    if (!slug || seen.has(slug)) {
      return null;
    }
    seen.add(slug);
    var filmIdNode = poster.closest('[data-film-id]');
    return {
      name: ((poster.dataset.itemName || poster.dataset.filmName || '').trim()) || null,
      slug: slug,
      film_id: ((poster.dataset.filmId || (filmIdNode ? filmIdNode.dataset.filmId : '') || '').trim()) || null,
      url: 'https://letterboxd.com/film/' + slug + '/'
    };
  }).filter(function(item) {
    return item.slug;
  }).slice(0, 30));
})()
"#;

#[derive(Debug, Deserialize)]
struct PopularFilm {
    name: Option<String>,
    slug: Option<String>,
    film_id: Option<String>,
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
    if !page_url.starts_with(TARGET_URL_PREFIX) {
        return Err(8);
    }

    let mut films = Vec::new();
    for attempt in 0..EXTRACTION_ATTEMPTS {
        films = extract_films()?;
        if films.len() >= MIN_POPULAR_FILMS {
            break;
        }
        if attempt + 1 < EXTRACTION_ATTEMPTS {
            let waited = wait(RETRY_WAIT_MS).map_err(|_| 16)?;
            if waited.elapsed_ms < RETRY_WAIT_MS {
                return Err(17);
            }
        }
    }
    if films.len() < MIN_POPULAR_FILMS {
        return Err(11);
    }

    let first = films.first().ok_or(12)?;
    if first.name.as_deref().unwrap_or("").trim().is_empty() {
        return Err(13);
    }
    let slug = first.slug.as_deref().unwrap_or("").trim();
    if slug.is_empty() || slug.contains(' ') {
        return Err(14);
    }
    let url = first.url.as_deref().unwrap_or("").trim();
    if !url.starts_with("https://letterboxd.com/film/") {
        return Err(15);
    }
    let film_id = first.film_id.as_deref().unwrap_or("").trim();
    if !film_id.is_empty() && !film_id.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(18);
    }

    Ok(())
}

fn extract_films() -> Result<Vec<PopularFilm>, i32> {
    let raw_films: String = js(EXTRACTION_SCRIPT).map_err(|_| 9)?;
    serde_json::from_str(&raw_films).map_err(|_| 10)
}
