# play2048.co — 2048 Game Automation

`https://play2048.co/` is a canvas-rendered 2048 implementation. DOM tile
selectors are not useful for board reads; use the persisted game store for
state and normal keyboard events for moves.

## Fast Path

- Open a fresh game with `new_tab("https://play2048.co/")`, then
  `wait_for_load()`.
- Read the board from localStorage instead of OCR. The active standard-mode
  store is the persisted key for `k-standard`.
- Send moves through keyboard events: `ArrowUp`, `ArrowRight`, `ArrowDown`,
  `ArrowLeft`. The page listens at `window` capture phase and maps those keys
  to normal game moves.
- Re-read the persisted state after each move. The store updates with `score`,
  `moveCount`, `state`, and a 4x4 `board`.

## State Store

The relevant app bundle imports the persisted store as:

```text
X(`${ve}-${mode}`, ..., { serializer })
```

For the standard game, `ve` is `"k"` and `mode` is `"standard"`, so the logical
store name is `k-standard`. The app hashes logical store names before writing
them to localStorage.

Use this page-side helper to find and decode the standard game state:

```js
const te = new TextEncoder();
const td = new TextDecoder();
const xorKey = "dGhlIGJyb3duIGZveCBqdW1wcyBvdmVyIHRoZSBsYXp5IGRvZw==";
const pads = ["", atob("PQ=="), atob("PT0=")];

function storageKey(name) {
  const sum = te.encode(name).reduce((acc, byte) => acc + Math.sin(byte), 0);
  const suffix = Math.floor(1e7 * sum).toString(36);
  return te.encode(`${name}${suffix}`).reduce(
    (acc, byte) => acc + byte.toString(36).split("").reverse().join(""),
    "",
  );
}

function restorePadding(value) {
  const len = value.length;
  let trimmed = 0;
  if (len >= 2 && value.charCodeAt(len - 2) === 61) trimmed = 2;
  else if (len >= 1 && value.charCodeAt(len - 1) === 61) trimmed = 1;
  return value.substring(0, len - trimmed) + pads[(trimmed + 1) % 3];
}

function decodeStore(value) {
  const raw = atob(restorePadding(value));
  const key = te.encode(xorKey);
  const out = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i += 1) {
    out[i] = raw.charCodeAt(i) ^ key[i % key.length];
  }
  return JSON.parse(td.decode(out));
}

const state = decodeStore(localStorage.getItem(storageKey("k-standard")));
const board = state.board.map((row) => row.map((cell) => cell?.value ?? 0));
```

State shape:

```text
{
  state: "fresh" | "playing" | "gameOver" | "gameWon",
  score: number,
  moveCount: number,
  board: TileOrNull[][],
  highestReachedTile: number
}
```

Each non-null tile has `value` and `position: { x, y }`; the array is addressed
as `board[y][x]`.

## Move Dispatch

Prefer normal keyboard input over localStorage mutation. A page-side dispatch
works reliably:

```js
function move(key) {
  const keyCode = { ArrowLeft: 37, ArrowUp: 38, ArrowRight: 39, ArrowDown: 40 }[key];
  const init = { key, code: key, keyCode, which: keyCode, bubbles: true, cancelable: true };
  window.dispatchEvent(new KeyboardEvent("keydown", init));
  window.dispatchEvent(new KeyboardEvent("keyup", init));
}

move("ArrowLeft");
```

Wait a short interval after each move before reading state again. The game state
usually updates immediately, but animations and persistence can lag by a frame.

## Gotchas

- The board is drawn on a `<canvas>`; `.tile` selectors from the original 2048
  implementation will return nothing.
- The welcome/tutorial popover does not block keyboard moves.
- The page has additional powerups. For ordinary 2048 automation, ignore them
  and drive only arrow-key moves.
- Avoid writing raw localStorage state unless the task is explicitly about
  import/reset/debugging; it bypasses normal gameplay.
