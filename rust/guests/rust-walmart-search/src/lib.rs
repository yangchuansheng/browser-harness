use bh_guest_sdk::http_get;
use serde::Deserialize;

const TARGET_URL: &str = "https://www.walmart.com/search?q=laptop";
const MIN_TOTAL_RESULTS: u64 = 1_000;
const MIN_PAGE_ITEMS: usize = 20;

#[derive(Debug, Deserialize)]
struct NextData {
    props: Props,
}

#[derive(Debug, Deserialize)]
struct Props {
    #[serde(rename = "pageProps")]
    page_props: PageProps,
}

#[derive(Debug, Deserialize)]
struct PageProps {
    #[serde(rename = "initialData")]
    initial_data: InitialData,
}

#[derive(Debug, Deserialize)]
struct InitialData {
    #[serde(rename = "searchResult")]
    search_result: SearchResult,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    #[serde(rename = "aggregatedCount")]
    aggregated_count: Option<u64>,
    #[serde(rename = "paginationV2")]
    pagination_v2: Option<PaginationV2>,
    #[serde(rename = "itemStacks", default)]
    item_stacks: Vec<ItemStack>,
}

#[derive(Debug, Deserialize)]
struct PaginationV2 {
    #[serde(rename = "maxPage")]
    max_page: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ItemStack {
    #[serde(default)]
    items: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    #[serde(rename = "usItemId")]
    us_item_id: Option<String>,
    name: Option<String>,
    brand: Option<String>,
    price: Option<f64>,
    #[serde(rename = "canonicalUrl")]
    canonical_url: Option<String>,
}

#[no_mangle]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run_inner() -> Result<(), i32> {
    let html = http_get(TARGET_URL, None, Some(20.0)).map_err(|_| 1)?;
    let script = extract_script_by_id(&html, "__NEXT_DATA__").ok_or(2)?;
    let data: NextData = serde_json::from_str(script).map_err(|_| 3)?;
    let search = data.props.page_props.initial_data.search_result;

    if search.aggregated_count.unwrap_or_default() < MIN_TOTAL_RESULTS {
        return Err(4);
    }
    if search
        .pagination_v2
        .as_ref()
        .and_then(|pagination| pagination.max_page)
        .unwrap_or_default()
        < 2
    {
        return Err(5);
    }

    let items = search
        .item_stacks
        .into_iter()
        .flat_map(|stack| stack.items)
        .filter(|item| {
            item.us_item_id
                .as_deref()
                .map(|id| !id.trim().is_empty())
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    if items.len() < MIN_PAGE_ITEMS {
        return Err(6);
    }

    let first = items.first().ok_or(7)?;
    if first.name.as_deref().unwrap_or("").trim().is_empty() {
        return Err(8);
    }
    if first.price.unwrap_or_default() <= 0.0 {
        return Err(9);
    }
    let canonical_url = first.canonical_url.as_deref().unwrap_or("").trim();
    if !canonical_url.starts_with("/ip/") {
        return Err(10);
    }
    if first
        .brand
        .as_deref()
        .map(|brand| brand.trim())
        .unwrap_or("")
        .len()
        > 128
    {
        return Err(11);
    }

    Ok(())
}

fn extract_script_by_id<'a>(html: &'a str, id: &str) -> Option<&'a str> {
    let marker = format!(r#"<script id="{id}""#);
    let start = html.find(&marker)?;
    let script_start = start + html[start..].find('>')? + 1;
    let script_end = script_start + html[script_start..].find("</script>")?;
    Some(&html[script_start..script_end])
}
