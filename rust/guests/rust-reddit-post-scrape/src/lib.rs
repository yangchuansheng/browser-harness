use bh_guest_sdk::{ensure_real_tab, goto, js, page_info, scroll, wait, wait_for_load};
use serde::Deserialize;
use serde_json::Value;

const TARGET_URL: &str = "https://www.reddit.com/r/vibecoding/comments/1kwuqpz/";
const TARGET_URL_PREFIX: &str = "https://www.reddit.com/r/vibecoding/comments/1kwuqpz";
const INITIAL_WAIT_MS: u64 = 3_000;
const SCROLL_WAIT_MS: u64 = 1_000;
const EXTRACTION_SCRIPT: &str = r#"
(()=>{
  const loginWall = !!document.querySelector('a[href*="/login"], [data-testid="login-button"]');
  const ageGate = !!document.querySelector('[data-testid="nsfw-gate"], shreddit-interstitial');
  const postEl = document.querySelector('shreddit-post');
  if(!postEl){
    return JSON.stringify({
      loginWall,
      ageGate,
      postFound: false,
      subreddit: '',
      title: '',
      author: '',
      score: '',
      body: '',
      comments: [],
      url: location.href
    });
  }
  const title = (postEl.querySelector('h1, [slot="title"]')||{}).innerText?.trim() || '';
  const bodyEl = postEl.querySelector('[slot="text-body"] .md, [slot="text-body"]');
  const body = bodyEl ? bodyEl.innerText.trim() : '';
  const author = (postEl.querySelector('[slot="authorName"] a, a[data-testid="post_author_link"]')||{}).innerText?.trim() || '';
  const subM = location.pathname.match(/^\/r\/([^\/]+)/);
  const subreddit = subM ? subM[1] : '';
  const scoreEl = postEl.querySelector('faceplate-number');
  const score = scoreEl ? scoreEl.getAttribute('number') || scoreEl.innerText || '' : '';
  const comments = [];
  for(const c of document.querySelectorAll('shreddit-comment[depth="0"]')){
    const cBodyEl = c.querySelector('[slot="comment"] .md, [slot="comment"]');
    const cBody = cBodyEl ? cBodyEl.innerText.trim() : '';
    if(!cBody) continue;
    comments.push({
      author: c.getAttribute('author') || '',
      score: c.getAttribute('score') || '',
      body: cBody
    });
    if(comments.length >= 10) break;
  }
  return JSON.stringify({
    loginWall,
    ageGate,
    postFound: true,
    subreddit,
    title,
    author,
    score,
    body,
    comments,
    url: location.href
  });
})()
"#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RedditPost {
    login_wall: bool,
    age_gate: bool,
    post_found: bool,
    subreddit: String,
    title: String,
    author: String,
    score: String,
    body: String,
    comments: Vec<RedditComment>,
    url: String,
}

#[derive(Debug, Deserialize)]
struct RedditComment {
    author: String,
    score: String,
    body: String,
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

    let goto_result = goto(TARGET_URL).map_err(|_| 2)?;
    if goto_result
        .get("errorText")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
    {
        return Err(28);
    }

    let loaded = wait_for_load(15.0).map_err(|_| 3)?;
    if !loaded {
        return Err(4);
    }

    let initial_wait = wait(INITIAL_WAIT_MS).map_err(|_| 5)?;
    if initial_wait.elapsed_ms < INITIAL_WAIT_MS {
        return Err(6);
    }

    scroll(500.0, 500.0, 2_000.0, 0.0).map_err(|_| 7)?;
    let first_scroll_wait = wait(SCROLL_WAIT_MS).map_err(|_| 8)?;
    if first_scroll_wait.elapsed_ms < SCROLL_WAIT_MS {
        return Err(9);
    }

    scroll(500.0, 500.0, 2_000.0, 0.0).map_err(|_| 10)?;
    let second_scroll_wait = wait(SCROLL_WAIT_MS).map_err(|_| 11)?;
    if second_scroll_wait.elapsed_ms < SCROLL_WAIT_MS {
        return Err(12);
    }

    let page = page_info().map_err(|_| 13)?;
    let page_url = page.get("url").and_then(Value::as_str).unwrap_or("");
    if !page_url.starts_with(TARGET_URL_PREFIX) {
        return Err(14);
    }

    let raw_post: String = js(EXTRACTION_SCRIPT).map_err(|_| 15)?;
    let post: RedditPost = serde_json::from_str(&raw_post).map_err(|_| 16)?;
    if post.login_wall {
        return Err(17);
    }
    if post.age_gate {
        return Err(18);
    }
    if !post.post_found {
        return Err(19);
    }
    if post.subreddit != "vibecoding" {
        return Err(20);
    }
    if !post.url.starts_with(TARGET_URL_PREFIX) {
        return Err(21);
    }
    if post.title.trim().is_empty() {
        return Err(22);
    }
    if post.author.trim().is_empty() {
        return Err(23);
    }
    if post.comments.is_empty() {
        return Err(24);
    }
    let first_comment = post.comments.first().ok_or(25)?;
    if first_comment.author.trim().is_empty() {
        return Err(26);
    }
    if first_comment.body.trim().is_empty() {
        return Err(27);
    }

    let _ = &post.score;
    let _ = &post.body;
    let _ = &first_comment.score;

    Ok(())
}
