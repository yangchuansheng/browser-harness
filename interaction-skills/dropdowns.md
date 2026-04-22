# Dropdowns

Split dropdowns into native selects, custom overlays, and searchable comboboxes.

## Native Select

Prefer DOM/JS selection if you can target the `<select>` directly.

## Custom Overlay Or Combobox

Use typed input primitives:

- `click`
- `type_text`
- `press_key`
- `dispatch_key`
- `scroll`

## Rules

- open the dropdown first
- re-measure or re-query after opening because option geometry often appears
  late
- for searchable comboboxes, type and then confirm with Enter or arrow keys
- for virtualized menus, verify loaded options through DOM state after scroll

## Example

Typical custom flow:

1. click the trigger
2. type to filter if needed
3. press ArrowDown / Enter
4. verify the selected label

`dispatch_key` is useful when the focused element expects DOM keyboard events
rather than browser-wide key sequences.
