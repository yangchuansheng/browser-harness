use std::cmp::Ordering;

use bh_guest_sdk::{cdp_raw, goto, js, page_info, press_key as cdp_press_key, wait, wait_for_load};
use serde::Deserialize;
use serde_json::{json, Value};

const TARGET_URL: &str = "https://play2048.co/?via=rust-2048-autoplay-guest";
const TARGET_URL_PREFIX: &str = "https://play2048.co/";
const CLASSIC_URL: &str = "https://classic.play2048.co/";
const CLASSIC_URL_PREFIX: &str = "https://classic.play2048.co/";
const POST_NAVIGATION_WAIT_MS: u64 = 3_000;
const TARGET_POLL_WAIT_MS: u64 = 250;
const MOVE_SETTLE_WAIT_MS: u64 = 120;
const CLASSIC_MOVE_SETTLE_WAIT_MS: u64 = 20;
const MAX_MOVE_SETTLE_POLLS: usize = 20;
const MAX_ATTEMPTS: usize = 8;
const MAX_TURNS_PER_ATTEMPT: usize = 4_000;

const HOOK_SCRIPT: &str = r#"
(() => {
  if (window.__bh2048HookInstalled) {
    return;
  }

  window.__bh2048HookInstalled = true;
  const hook = {
    created: 0,
    updateCount: 0,
    latestUpdate: null,
    lastMessages: [],
  };
  Object.defineProperty(window, "__bh2048Hook", {
    value: hook,
    configurable: true,
  });

  const pushMessage = (entry) => {
    hook.lastMessages.push(entry);
    if (hook.lastMessages.length > 24) {
      hook.lastMessages.shift();
    }
    if (
      entry &&
      entry.dir === "to" &&
      entry.message &&
      entry.message.type === "call" &&
      entry.message.call === "update" &&
      Array.isArray(entry.message.args) &&
      entry.message.args.length > 0
    ) {
      hook.latestUpdate = entry.message.args[0];
      hook.updateCount += 1;
    }
  };

  const OriginalWorker = window.Worker;
  const WrappedWorker = function (...args) {
    const worker = new OriginalWorker(...args);
    hook.created += 1;
    const originalPostMessage = worker.postMessage.bind(worker);

    worker.postMessage = function (message, transfer) {
      try {
        pushMessage({ dir: "to", message });
      } catch {}
      return transfer === undefined
        ? originalPostMessage(message)
        : originalPostMessage(message, transfer);
    };

    worker.addEventListener("message", (event) => {
      try {
        pushMessage({ dir: "from", message: event.data });
      } catch {}
    });

    return worker;
  };

  WrappedWorker.prototype = OriginalWorker.prototype;
  window.Worker = WrappedWorker;
})();
"#;

const INSTALL_RUNTIME_SCRIPT: &str = r##"
(() => {
  if (window.__bh2048Guest) {
    return "ready";
  }

  const runtime = {
    targetOverlayId: "bh2048-target-overlay",

    normalizeText(value) {
      return String(value || "").replace(/\s+/g, " ").trim();
    },

    clickButton(text) {
      const wanted = this.normalizeText(text);
      const candidate = Array.from(document.querySelectorAll("button")).find((button) => {
        const current = this.normalizeText(button.innerText || button.textContent || "");
        return (
          current === wanted ||
          current.startsWith(`${wanted} `) ||
          current.endsWith(` ${wanted}`) ||
          current.includes(` ${wanted} `)
        );
      });
      if (candidate) {
        candidate.click();
        return true;
      }
      return false;
    },

    press(key) {
      const event = new KeyboardEvent("keydown", {
        key,
        code: key,
        bubbles: true,
      });
      window.dispatchEvent(event);
      document.dispatchEvent(event);
      return true;
    },

    removeNode(node) {
      if (
        !node ||
        node === document.body ||
        node === document.documentElement ||
        node === document.head ||
        node.id === "app" ||
        node.id === this.targetOverlayId
      ) {
        return false;
      }
      if (node.closest && node.closest(`#${this.targetOverlayId}`)) {
        return false;
      }
      node.remove();
      return true;
    },

    removeAds() {
      let removed = 0;
      let clicked = 0;
      const app = document.getElementById("app");

      for (const iframe of Array.from(document.querySelectorAll("iframe"))) {
        if (this.removeNode(iframe)) {
          removed += 1;
        }
      }

      for (const selector of [
        "#cmpwrapper",
        "[id*='receptivity']",
        "[id*='pubnation']",
        "[id*='mediavine']",
        "[data-google-query-id]",
      ]) {
        for (const node of Array.from(document.querySelectorAll(selector))) {
          if (this.removeNode(node)) {
            removed += 1;
          }
        }
      }

      for (const phrase of [
        "A Message from Samsung",
        "LEARN MORE",
        "Get the App",
        "Privacy Settings",
      ]) {
        const matches = Array.from(document.querySelectorAll("body *")).filter((node) => {
          if (!node || !node.textContent) {
            return false;
          }
          const text = node.textContent.trim();
          if (!text || text !== phrase) {
            return false;
          }
          if (app && node.closest && node.closest("#app") === app && phrase === "Privacy Settings") {
            return false;
          }
          return true;
        });

        for (const match of matches) {
          const container =
            (match.closest &&
              match.closest("aside, section, article, button, a, dialog")) ||
            match;
          if (this.removeNode(container)) {
            removed += 1;
          }
        }
      }

      for (const buttonText of ["Privacy Settings", "Get the App"]) {
        if (this.clickButton(buttonText)) {
          clicked += 1;
        }
      }

      return { removed, clicked };
    },

    ensureTargetOverlay() {
      if (document.getElementById(this.targetOverlayId)) {
        return;
      }

      const overlay = document.createElement("div");
      overlay.id = this.targetOverlayId;
      overlay.style.cssText = [
        "position:fixed",
        "inset:0",
        "display:flex",
        "align-items:center",
        "justify-content:center",
        "background:rgba(17,24,39,0.65)",
        "z-index:2147483647",
        "font-family:Rubik,system-ui,sans-serif",
      ].join(";");

      const panel = document.createElement("div");
      panel.style.cssText = [
        "width:min(28rem,calc(100vw - 2rem))",
        "background:#fff7ed",
        "color:#111827",
        "border-radius:18px",
        "padding:20px",
        "box-shadow:0 18px 60px rgba(0,0,0,0.35)",
        "display:flex",
        "flex-direction:column",
        "gap:12px",
      ].join(";");

      panel.innerHTML = `
        <div style="font-size:1.35rem;font-weight:700;">2048 Target Score</div>
        <div style="font-size:0.95rem;line-height:1.45;">
          Enter the score target to reach. The guest will clear ads, keep playing,
          and restart after losses until one run hits the target.
        </div>
      `;

      const input = document.createElement("input");
      input.type = "number";
      input.min = "32";
      input.step = "32";
      input.value = "12000";
      input.placeholder = "12000";
      input.style.cssText = [
        "font-size:1rem",
        "padding:0.8rem 0.9rem",
        "border:1px solid #cbd5e1",
        "border-radius:12px",
        "background:white",
      ].join(";");

      const button = document.createElement("button");
      button.textContent = "Start Bot";
      button.style.cssText = [
        "font-size:1rem",
        "font-weight:700",
        "padding:0.85rem 1rem",
        "border:none",
        "border-radius:12px",
        "background:#111827",
        "color:white",
        "cursor:pointer",
      ].join(";");

      const helper = document.createElement("div");
      helper.style.cssText = "font-size:0.85rem;color:#475569;";
      helper.textContent =
        "Automation can skip this prompt by setting localStorage['bh2048GuestTarget'] first.";

      const accept = () => {
        const parsed = Number.parseInt(input.value, 10);
        if (!Number.isFinite(parsed) || parsed < 32) {
          input.focus();
          input.select();
          return;
        }
        window.__bh2048GuestTarget = parsed;
        overlay.remove();
      };

      button.addEventListener("click", accept);
      input.addEventListener("keydown", (event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          accept();
        }
      });

      panel.appendChild(input);
      panel.appendChild(button);
      panel.appendChild(helper);
      overlay.appendChild(panel);
      document.body.appendChild(overlay);
      input.focus();
      input.select();
    },

    consumeTarget() {
      if (Number.isFinite(window.__bh2048GuestTarget)) {
        return {
          status: "ready",
          target: window.__bh2048GuestTarget,
        };
      }

      try {
        const raw = window.localStorage.getItem("bh2048GuestTarget");
        if (raw !== null) {
          window.localStorage.removeItem("bh2048GuestTarget");
          const parsed = Number.parseInt(raw, 10);
          if (Number.isFinite(parsed) && parsed >= 32) {
            window.__bh2048GuestTarget = parsed;
            return {
              status: "ready",
              target: parsed,
            };
          }
        }
      } catch {}

      this.ensureTargetOverlay();
      return { status: "waiting" };
    },
  };

  Object.defineProperty(window, "__bh2048Guest", {
    value: runtime,
    configurable: true,
  });

  return "ready";
})();
"##;

const SNAPSHOT_SCRIPT: &str = r#"
JSON.stringify((() => {
  const hook = window.__bh2048Hook || null;
  return {
    hookInstalled: !!window.__bh2048HookInstalled,
    hookCreated: hook ? hook.created : 0,
    updateCount: hook ? hook.updateCount : 0,
    latestUpdate: hook ? hook.latestUpdate : null,
    bodyHead: document.body ? document.body.innerText.slice(0, 600) : null,
  };
})())
"#;

const TARGET_STATUS_SCRIPT: &str = r#"
JSON.stringify(window.__bh2048Guest.consumeTarget())
"#;

const DOM_STATE_SCRIPT: &str = r#"
JSON.stringify((() => {
  const text = document.body ? document.body.innerText || "" : "";
  const lines = text.split(/\n+/).map((line) => line.trim()).filter(Boolean);
  const parseValue = (index) => {
    if (index < 0 || index + 1 >= lines.length) {
      return 0;
    }
    const digits = String(lines[index + 1] || "").replace(/[^\d]/g, "");
    return digits ? Number.parseInt(digits, 10) : 0;
  };
  const scoreIndex = lines.findIndex((line) => line.toUpperCase() === "SCORE");
  const bestIndex = lines.findIndex((line) => line.toUpperCase() === "BEST");
  return {
    score: parseValue(scoreIndex),
    best: parseValue(bestIndex),
    gameOver: lines.some((line) => /^game over$/i.test(line)),
  };
})())
"#;

#[derive(Debug, Deserialize)]
struct AddScriptResponse {
    identifier: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuestSnapshot {
    hook_installed: bool,
    hook_created: u64,
    update_count: u64,
    latest_update: Option<GameUpdate>,
    #[serde(rename = "bodyHead")]
    _body_head: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameUpdate {
    state: String,
    board: Vec<Vec<Option<TileSnapshot>>>,
    score: u32,
    move_count: u32,
    powerups: Option<PowerupSnapshot>,
}

#[derive(Debug, Clone, Deserialize)]
struct TileSnapshot {
    value: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PowerupSnapshot {
    undo: Option<UndoPowerup>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UndoPowerup {
    uses_remaining: u32,
}

#[derive(Debug, Deserialize)]
struct TargetStatus {
    status: String,
    target: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DomState {
    score: u32,
    _best: u32,
    game_over: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClassicState {
    ready: bool,
    score: u32,
    over: bool,
    won: bool,
    keep_playing: bool,
    board: Vec<Vec<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Move {
    Up,
    Left,
    Right,
    Down,
}

impl Move {
    fn key(self) -> &'static str {
        match self {
            Self::Up => "ArrowUp",
            Self::Left => "ArrowLeft",
            Self::Right => "ArrowRight",
            Self::Down => "ArrowDown",
        }
    }

    fn ordered() -> [Self; 4] {
        [Self::Up, Self::Left, Self::Right, Self::Down]
    }
}

#[derive(Debug, Clone)]
struct Board {
    cells: [u32; 16],
}

impl Board {
    fn from_snapshot(snapshot: &GameUpdate) -> Result<Self, i32> {
        if snapshot.board.len() != 4 || snapshot.board.iter().any(|row| row.len() != 4) {
            return Err(45);
        }

        let mut cells = [0u32; 16];
        for (y, row) in snapshot.board.iter().enumerate() {
            for (x, tile) in row.iter().enumerate() {
                cells[y * 4 + x] = tile.as_ref().map(|tile| tile.value).unwrap_or(0);
            }
        }

        Ok(Self { cells })
    }

    fn from_rows(rows: &[Vec<u32>]) -> Result<Self, i32> {
        if rows.len() != 4 || rows.iter().any(|row| row.len() != 4) {
            return Err(63);
        }

        let mut cells = [0u32; 16];
        for (y, row) in rows.iter().enumerate() {
            for (x, value) in row.iter().enumerate() {
                cells[y * 4 + x] = *value;
            }
        }

        Ok(Self { cells })
    }

    fn empty_count(&self) -> usize {
        self.cells.iter().filter(|value| **value == 0).count()
    }

    fn highest_tile(&self) -> u32 {
        self.cells.iter().copied().max().unwrap_or(0)
    }

    fn get(&self, x: usize, y: usize) -> u32 {
        self.cells[y * 4 + x]
    }

    fn set(&mut self, x: usize, y: usize, value: u32) {
        self.cells[y * 4 + x] = value;
    }

    fn apply_move(&self, direction: Move) -> Option<(Self, u32)> {
        let mut next = self.clone();
        let mut changed = false;
        let mut gained = 0;

        match direction {
            Move::Left => {
                for y in 0..4 {
                    let line = [self.get(0, y), self.get(1, y), self.get(2, y), self.get(3, y)];
                    let (updated, line_score, line_changed) = compress_line(line);
                    gained += line_score;
                    changed |= line_changed;
                    for (x, value) in updated.into_iter().enumerate() {
                        next.set(x, y, value);
                    }
                }
            }
            Move::Right => {
                for y in 0..4 {
                    let line = [self.get(3, y), self.get(2, y), self.get(1, y), self.get(0, y)];
                    let (updated, line_score, line_changed) = compress_line(line);
                    gained += line_score;
                    changed |= line_changed;
                    for (index, value) in updated.into_iter().enumerate() {
                        next.set(3 - index, y, value);
                    }
                }
            }
            Move::Up => {
                for x in 0..4 {
                    let line = [self.get(x, 0), self.get(x, 1), self.get(x, 2), self.get(x, 3)];
                    let (updated, line_score, line_changed) = compress_line(line);
                    gained += line_score;
                    changed |= line_changed;
                    for (y, value) in updated.into_iter().enumerate() {
                        next.set(x, y, value);
                    }
                }
            }
            Move::Down => {
                for x in 0..4 {
                    let line = [self.get(x, 3), self.get(x, 2), self.get(x, 1), self.get(x, 0)];
                    let (updated, line_score, line_changed) = compress_line(line);
                    gained += line_score;
                    changed |= line_changed;
                    for (index, value) in updated.into_iter().enumerate() {
                        next.set(x, 3 - index, value);
                    }
                }
            }
        }

        changed.then_some((next, gained))
    }

    fn available_moves(&self) -> Vec<(Move, Self, u32)> {
        Move::ordered()
            .into_iter()
            .filter_map(|direction| {
                self.apply_move(direction)
                    .map(|(board, gained)| (direction, board, gained))
            })
            .collect()
    }

    fn spawn_positions(&self) -> Vec<usize> {
        self.cells
            .iter()
            .enumerate()
            .filter_map(|(index, value)| (*value == 0).then_some(index))
            .collect()
    }

    fn with_spawn(&self, index: usize, value: u32) -> Self {
        let mut next = self.clone();
        next.cells[index] = value;
        next
    }

    fn heuristic(&self) -> f64 {
        let empty = self.empty_count() as f64;
        let smoothness = smoothness(self);
        let monotonicity = monotonicity(self);
        let snake = snake_score(self);
        let merges = merge_potential(self) as f64;
        let highest = self.highest_tile() as f64;
        let highest_log = if highest > 0.0 { highest.log2() } else { 0.0 };
        let highest_in_corner = {
            let highest = self.highest_tile();
            [self.cells[0], self.cells[3], self.cells[12], self.cells[15]]
                .into_iter()
                .any(|corner| corner == highest)
        };
        let corner_bonus = if highest_in_corner {
            highest_log * 240.0
        } else {
            0.0
        };

        empty * 320.0
            + smoothness * 6.0
            + monotonicity * 90.0
            + snake * 3.0
            + merges * 35.0
            + highest_log * 40.0
            + corner_bonus
    }
}

#[no_mangle]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run_inner() -> Result<(), i32> {
    let hook_identifier = install_worker_hook()?;
    let result = run_game_loop();
    let _ = cdp_raw(
        "Page.removeScriptToEvaluateOnNewDocument",
        Some(json!({ "identifier": hook_identifier })),
        None,
    );
    result
}

fn install_worker_hook() -> Result<String, i32> {
    let response = cdp_raw(
        "Page.addScriptToEvaluateOnNewDocument",
        Some(json!({ "source": HOOK_SCRIPT })),
        None,
    )
    .map_err(|_| 1)?;

    serde_json::from_value::<AddScriptResponse>(response)
        .map(|response| response.identifier)
        .map_err(|_| 2)
}

fn run_game_loop() -> Result<(), i32> {
    goto(TARGET_URL).map_err(|_| 3)?;
    let _ = wait_for_load(20.0).map_err(|_| 4)?;
    let slept = wait(POST_NAVIGATION_WAIT_MS).map_err(|_| 5)?;
    if slept.elapsed_ms < POST_NAVIGATION_WAIT_MS {
        return Err(6);
    }

    install_runtime()?;
    ensure_target_prefix()?;
    let target = wait_for_target_input()?;
    cleanup_ads()?;

    if run_modern_game_loop(target).is_ok() {
        cleanup_ads()?;
        return Ok(());
    }

    run_classic_game_loop(target)
}

fn run_modern_game_loop(target: u32) -> Result<(), i32> {
    for _ in 0..MAX_ATTEMPTS {
        cleanup_ads()?;
        let dom_state = read_dom_state()?;
        if dom_state.score >= target {
            cleanup_ads()?;
            return Ok(());
        }
        if dom_state.game_over {
            restart_game()?;
            cleanup_ads()?;
        }

        let mut snapshot = bootstrap_snapshot()?;

        for turn in 0..MAX_TURNS_PER_ATTEMPT {
            if turn % 12 == 0 {
                cleanup_ads()?;
            }

            let latest = snapshot.latest_update.as_ref().ok_or(8)?;
            if latest.score >= target {
                cleanup_ads()?;
                return Ok(());
            }

            if is_game_over(latest) {
                if latest
                    .powerups
                    .as_ref()
                    .and_then(|powerups| powerups.undo.as_ref())
                    .map(|undo| undo.uses_remaining)
                    .unwrap_or(0)
                    > 0
                    && click_button("Undo").map_err(|_| 9)?
                {
                    wait(MOVE_SETTLE_WAIT_MS).map_err(|_| 10)?;
                    snapshot = wait_for_snapshot_progress(snapshot.update_count, latest.move_count)?;
                    continue;
                }
                break;
            }

            let board = Board::from_snapshot(latest)?;
            let chosen_move = choose_best_move(&board).ok_or(11)?;
            press_move(chosen_move)?;
            snapshot = wait_for_snapshot_progress(snapshot.update_count, latest.move_count)?;
        }

        let dom_state = read_dom_state()?;
        if dom_state.score >= target {
            cleanup_ads()?;
            return Ok(());
        }
        restart_game()?;
    }

    Err(7)
}

fn run_classic_game_loop(target: u32) -> Result<(), i32> {
    goto(CLASSIC_URL).map_err(|_| 64)?;
    let _ = wait_for_load(20.0).map_err(|_| 65)?;
    let slept = wait(1_000).map_err(|_| 66)?;
    if slept.elapsed_ms < 1_000 {
        return Err(67);
    }
    ensure_classic_prefix()?;

    let mut state = read_classic_state(target)?;
    if !state.ready {
        return Err(68);
    }

    for _ in 0..MAX_ATTEMPTS {
        for _ in 0..MAX_TURNS_PER_ATTEMPT {
            if state.score >= target {
                return Ok(());
            }

            if state.won && !state.keep_playing && state.score < target {
                state = read_classic_state(target)?;
                continue;
            }

            if state.over {
                state = restart_classic_game(target)?;
                continue;
            }

            let board = Board::from_rows(&state.board)?;
            let chosen_move = choose_best_move(&board).ok_or(69)?;
            state = apply_classic_move(chosen_move, target)?;
        }

        if state.score >= target {
            return Ok(());
        }

        state = restart_classic_game(target)?;
    }

    Err(70)
}

fn install_runtime() -> Result<(), i32> {
    let status: String = js(INSTALL_RUNTIME_SCRIPT).map_err(|_| 16)?;
    if status != "ready" {
        return Err(17);
    }
    Ok(())
}

fn ensure_target_prefix() -> Result<(), i32> {
    let page = page_info().map_err(|_| 18)?;
    let url = page.get("url").and_then(Value::as_str).unwrap_or("");
    if url.starts_with(TARGET_URL_PREFIX) {
        return Ok(());
    }
    Err(19)
}

fn ensure_classic_prefix() -> Result<(), i32> {
    let page = page_info().map_err(|_| 71)?;
    let url = page.get("url").and_then(Value::as_str).unwrap_or("");
    if url.starts_with(CLASSIC_URL_PREFIX) {
        return Ok(());
    }
    Err(72)
}

fn cleanup_ads() -> Result<(), i32> {
    let _: bool = js(
        r#"
(() => {
  window.__bh2048Guest.removeAds();
  return true;
})()
"#,
    )
    .map_err(|_| 20)?;
    Ok(())
}

fn wait_for_target_input() -> Result<u32, i32> {
    loop {
        let raw: String = js(TARGET_STATUS_SCRIPT).map_err(|_| 21)?;
        let status: TargetStatus = serde_json::from_str(&raw).map_err(|_| 22)?;
        if status.status == "ready" {
            return status.target.ok_or(23);
        }
        let waited = wait(TARGET_POLL_WAIT_MS).map_err(|_| 24)?;
        if waited.elapsed_ms < TARGET_POLL_WAIT_MS {
            return Err(25);
        }
    }
}

fn read_snapshot() -> Result<GuestSnapshot, i32> {
    let raw: String = js(SNAPSHOT_SCRIPT).map_err(|_| 29)?;
    serde_json::from_str(&raw).map_err(|_| 30)
}

fn wait_for_snapshot_progress(
    previous_update_count: u64,
    previous_move_count: u32,
) -> Result<GuestSnapshot, i32> {
    for _ in 0..MAX_MOVE_SETTLE_POLLS {
        let waited = wait(MOVE_SETTLE_WAIT_MS).map_err(|_| 31)?;
        if waited.elapsed_ms < MOVE_SETTLE_WAIT_MS {
            return Err(32);
        }
        let snapshot = read_snapshot()?;
        let update = snapshot.latest_update.as_ref();
        if snapshot.update_count > previous_update_count {
            return Ok(snapshot);
        }
        if let Some(update) = update {
            if update.move_count != previous_move_count || is_game_over(update) {
                return Ok(snapshot);
            }
        }
    }
    Err(33)
}

fn read_dom_state() -> Result<DomState, i32> {
    let raw: String = js(DOM_STATE_SCRIPT).map_err(|_| 34)?;
    serde_json::from_str(&raw).map_err(|_| 35)
}

fn read_classic_state(target: u32) -> Result<ClassicState, i32> {
    let raw: String = js(&format!(
        r#"
JSON.stringify((() => {{
  if (!window.__bhClassicGM) {{
    if (
      typeof window.GameManager !== "function" ||
      typeof window.KeyboardInputManager !== "function" ||
      typeof window.HTMLActuator !== "function" ||
      typeof window.LocalStorageManager !== "function"
    ) {{
      return {{
        ready: false,
        score: 0,
        over: false,
        won: false,
        keepPlaying: false,
        board: [],
      }};
    }}
    window.__bhClassicGM = new window.GameManager(
      4,
      window.KeyboardInputManager,
      window.HTMLActuator,
      window.LocalStorageManager
    );
  }}

  const gm = window.__bhClassicGM;
  if (gm.won && !gm.keepPlaying && gm.score < {target}) {{
    gm.keepPlaying = true;
    if (typeof gm.actuate === "function") {{
      gm.actuate();
    }}
  }}

  const board = [];
  for (let y = 0; y < 4; y += 1) {{
    const row = [];
    for (let x = 0; x < 4; x += 1) {{
      const column = gm.grid && Array.isArray(gm.grid.cells) ? gm.grid.cells[x] : null;
      const cell = column && column[y] ? column[y] : null;
      row.push(cell ? cell.value : 0);
    }}
    board.push(row);
  }}

  return {{
    ready: true,
    score: Number(gm.score) || 0,
    over: !!gm.over,
    won: !!gm.won,
    keepPlaying: !!gm.keepPlaying,
    board,
  }};
}})())
"#,
    ))
    .map_err(|_| 73)?;
    serde_json::from_str(&raw).map_err(|_| 74)
}

fn apply_classic_move(direction: Move, target: u32) -> Result<ClassicState, i32> {
    let index = match direction {
        Move::Up => 0,
        Move::Right => 1,
        Move::Down => 2,
        Move::Left => 3,
    };

    let raw: String = js(&format!(
        r#"
JSON.stringify((() => {{
  const gm = window.__bhClassicGM;
  if (!gm) {{
    return {{
      ready: false,
      score: 0,
      over: false,
      won: false,
      keepPlaying: false,
      board: [],
    }};
  }}

  if (gm.won && !gm.keepPlaying && gm.score < {target}) {{
    gm.keepPlaying = true;
  }}
  gm.move({index});

  const board = [];
  for (let y = 0; y < 4; y += 1) {{
    const row = [];
    for (let x = 0; x < 4; x += 1) {{
      const column = gm.grid && Array.isArray(gm.grid.cells) ? gm.grid.cells[x] : null;
      const cell = column && column[y] ? column[y] : null;
      row.push(cell ? cell.value : 0);
    }}
    board.push(row);
  }}

  return {{
    ready: true,
    score: Number(gm.score) || 0,
    over: !!gm.over,
    won: !!gm.won,
    keepPlaying: !!gm.keepPlaying,
    board,
  }};
}})())
"#,
    ))
    .map_err(|_| 75)?;
    let waited = wait(CLASSIC_MOVE_SETTLE_WAIT_MS).map_err(|_| 76)?;
    if waited.elapsed_ms < CLASSIC_MOVE_SETTLE_WAIT_MS {
        return Err(77);
    }
    serde_json::from_str(&raw).map_err(|_| 78)
}

fn restart_classic_game(target: u32) -> Result<ClassicState, i32> {
    let raw: String = js(&format!(
        r#"
JSON.stringify((() => {{
  const gm = window.__bhClassicGM;
  if (!gm) {{
    return {{
      ready: false,
      score: 0,
      over: false,
      won: false,
      keepPlaying: false,
      board: [],
    }};
  }}

  gm.restart();
  if (gm.won && !gm.keepPlaying && gm.score < {target}) {{
    gm.keepPlaying = true;
  }}

  const board = [];
  for (let y = 0; y < 4; y += 1) {{
    const row = [];
    for (let x = 0; x < 4; x += 1) {{
      const column = gm.grid && Array.isArray(gm.grid.cells) ? gm.grid.cells[x] : null;
      const cell = column && column[y] ? column[y] : null;
      row.push(cell ? cell.value : 0);
    }}
    board.push(row);
  }}

  return {{
    ready: true,
    score: Number(gm.score) || 0,
    over: !!gm.over,
    won: !!gm.won,
    keepPlaying: !!gm.keepPlaying,
    board,
  }};
}})())
"#,
    ))
    .map_err(|_| 79)?;
    let waited = wait(120).map_err(|_| 80)?;
    if waited.elapsed_ms < 120 {
        return Err(81);
    }
    serde_json::from_str(&raw).map_err(|_| 82)
}

fn bootstrap_snapshot() -> Result<GuestSnapshot, i32> {
    for _ in 0..3 {
        for direction in Move::ordered() {
            let snapshot = read_snapshot()?;
            if snapshot.hook_installed
                && snapshot.hook_created > 0
                && snapshot.latest_update.is_some()
            {
                return Ok(snapshot);
            }

            press_move(direction)?;
            if let Ok(snapshot) = wait_for_snapshot_progress(snapshot.update_count, 0) {
                if snapshot.hook_installed
                    && snapshot.hook_created > 0
                    && snapshot.latest_update.is_some()
                {
                    return Ok(snapshot);
                }
            }
        }
    }

    Err(36)
}

fn restart_game() -> Result<(), i32> {
    if click_button("Play Again").map_err(|_| 37)?
        || click_button("New Game").map_err(|_| 38)?
        || click_button("Start New Game").map_err(|_| 39)?
    {
        wait(600).map_err(|_| 40)?;
        return Ok(());
    }

        let _ = cdp_press_key("r", 0);
        let _ = js_press_key("r");
        wait(600).map_err(|_| 41)?;
        Ok(())
}

fn is_game_over(update: &GameUpdate) -> bool {
    update.state.eq_ignore_ascii_case("gameOver")
}

fn click_button(text: &str) -> Result<bool, bh_guest_sdk::GuestError> {
    js(&format!(
        "window.__bh2048Guest.clickButton({})",
        json!(text)
    ))
}

fn js_press_key(key: &str) -> Result<bool, bh_guest_sdk::GuestError> {
    js(&format!("window.__bh2048Guest.press({})", json!(key)))
}

fn press_move(direction: Move) -> Result<(), i32> {
    cdp_press_key(direction.key(), 0).map_err(|_| 42)?;
    Ok(())
}

fn choose_best_move(board: &Board) -> Option<Move> {
    let empty = board.empty_count();
    let depth: usize = if empty >= 8 {
        3
    } else if empty >= 5 {
        4
    } else {
        5
    };

    Move::ordered()
        .into_iter()
        .filter_map(|direction| {
            let (next_board, gained_score) = board.apply_move(direction)?;
            let score = gained_score as f64 * 15.0 + expectimax(&next_board, depth.saturating_sub(1), false);
            Some((direction, score))
        })
        .max_by(|(_, left), (_, right)| left.partial_cmp(right).unwrap_or(Ordering::Equal))
        .map(|(direction, _)| direction)
}

fn expectimax(board: &Board, depth: usize, is_chance_node: bool) -> f64 {
    if depth == 0 {
        return board.heuristic();
    }

    if is_chance_node {
        let empties = board.spawn_positions();
        if empties.is_empty() {
            return board.heuristic();
        }

        let mut total = 0.0;
        let mut weight_sum = 0.0;
        for index in empties {
            for (probability, value) in [(0.9, 2u32), (0.1, 4u32)] {
                let next_board = board.with_spawn(index, value);
                total += probability * expectimax(&next_board, depth - 1, false);
                weight_sum += probability;
            }
        }
        if weight_sum > 0.0 {
            total / weight_sum
        } else {
            board.heuristic()
        }
    } else {
        let moves = board.available_moves();
        if moves.is_empty() {
            return -1_000_000.0;
        }
        moves
            .into_iter()
            .map(|(_, next_board, gained_score)| {
                gained_score as f64 * 12.0 + expectimax(&next_board, depth - 1, true)
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }
}

fn compress_line(line: [u32; 4]) -> ([u32; 4], u32, bool) {
    let original = line;
    let mut compact = Vec::with_capacity(4);
    for value in line {
        if value != 0 {
            compact.push(value);
        }
    }

    let mut merged = Vec::with_capacity(4);
    let mut gained = 0u32;
    let mut index = 0usize;
    while index < compact.len() {
        if index + 1 < compact.len() && compact[index] == compact[index + 1] {
            let value = compact[index] * 2;
            merged.push(value);
            gained += value;
            index += 2;
        } else {
            merged.push(compact[index]);
            index += 1;
        }
    }

    while merged.len() < 4 {
        merged.push(0);
    }

    let updated = [merged[0], merged[1], merged[2], merged[3]];
    let changed = updated != original;
    (updated, gained, changed)
}

fn log_cell(value: u32) -> f64 {
    if value == 0 {
        0.0
    } else {
        (value as f64).log2()
    }
}

fn smoothness(board: &Board) -> f64 {
    let mut total = 0.0;
    for y in 0..4 {
        for x in 0..4 {
            let value = board.get(x, y);
            if value == 0 {
                continue;
            }
            let current = log_cell(value);
            if x + 1 < 4 {
                let right = board.get(x + 1, y);
                if right != 0 {
                    total -= (current - log_cell(right)).abs();
                }
            }
            if y + 1 < 4 {
                let down = board.get(x, y + 1);
                if down != 0 {
                    total -= (current - log_cell(down)).abs();
                }
            }
        }
    }
    total
}

fn monotonicity(board: &Board) -> f64 {
    let mut totals = [0.0f64; 4];

    for y in 0..4 {
        let mut current = 0usize;
        let mut next = current + 1;
        while next < 4 {
            while next < 4 && board.get(next, y) == 0 {
                next += 1;
            }
            if next >= 4 {
                break;
            }
            let current_value = log_cell(board.get(current, y));
            let next_value = log_cell(board.get(next, y));
            if current_value > next_value {
                totals[0] += next_value - current_value;
            } else if next_value > current_value {
                totals[1] += current_value - next_value;
            }
            current = next;
            next += 1;
        }
    }

    for x in 0..4 {
        let mut current = 0usize;
        let mut next = current + 1;
        while next < 4 {
            while next < 4 && board.get(x, next) == 0 {
                next += 1;
            }
            if next >= 4 {
                break;
            }
            let current_value = log_cell(board.get(x, current));
            let next_value = log_cell(board.get(x, next));
            if current_value > next_value {
                totals[2] += next_value - current_value;
            } else if next_value > current_value {
                totals[3] += current_value - next_value;
            }
            current = next;
            next += 1;
        }
    }

    totals[0].max(totals[1]) + totals[2].max(totals[3])
}

fn merge_potential(board: &Board) -> usize {
    let mut total = 0usize;
    for y in 0..4 {
        for x in 0..4 {
            let value = board.get(x, y);
            if value == 0 {
                continue;
            }
            if x + 1 < 4 && board.get(x + 1, y) == value {
                total += 1;
            }
            if y + 1 < 4 && board.get(x, y + 1) == value {
                total += 1;
            }
        }
    }
    total
}

fn snake_score(board: &Board) -> f64 {
    const PATTERNS: [[f64; 16]; 4] = [
        [
            15.0, 14.0, 13.0, 12.0, 8.0, 9.0, 10.0, 11.0, 7.0, 6.0, 5.0, 4.0, 0.0, 1.0, 2.0,
            3.0,
        ],
        [
            12.0, 13.0, 14.0, 15.0, 11.0, 10.0, 9.0, 8.0, 4.0, 5.0, 6.0, 7.0, 3.0, 2.0, 1.0,
            0.0,
        ],
        [
            3.0, 2.0, 1.0, 0.0, 4.0, 5.0, 6.0, 7.0, 11.0, 10.0, 9.0, 8.0, 12.0, 13.0, 14.0,
            15.0,
        ],
        [
            0.0, 1.0, 2.0, 3.0, 7.0, 6.0, 5.0, 4.0, 8.0, 9.0, 10.0, 11.0, 15.0, 14.0, 13.0,
            12.0,
        ],
    ];

    PATTERNS
        .iter()
        .map(|pattern| {
            board
                .cells
                .iter()
                .zip(pattern.iter())
                .map(|(value, weight)| log_cell(*value) * *weight)
                .sum::<f64>()
        })
        .fold(f64::NEG_INFINITY, f64::max)
}
