# Shadow DOM

Prefer targeted DOM traversal first. Use coordinate input only when piercing the
component tree is more expensive than the action.

## DOM Traversal

Shadow DOM usually needs recursive `shadowRoot` access:

```python
from scripts._runner_cli import js

label = js(
    "document.querySelector('my-app').shadowRoot.querySelector('button').textContent"
)
print(label)
```

## When To Use Pointer Input Instead

Use `click`, `mouse-move`, or other low-level input when:

- the component tree is deeply nested
- the action is simple and visible
- DOM selectors are unstable but geometry is stable

## Rules

- verify the element exists before clicking through shadows
- if geometry depends on expansion, open the component and re-measure
- keep DOM discovery and pointer execution as separate steps
