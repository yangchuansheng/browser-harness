# Spreadshirt — showroom and product designer starter

Starter guide based on Spreadshirt Help articles. Spreadshirt has two important
surfaces for harness work:

- the public **Showroom** / marketplace side
- the interactive **Customize Tool** / Product Designer

Treat public catalog browsing and interactive product customization as separate
workflows.

## Do this first

- Decide whether you need a public showroom/catalog page or the interactive
  product designer.
- Use direct showroom URLs when you know the seller's Spread Name.
- Use the browser for product customization. The designer is a stateful UI with
  print-area constraints, product switching, and cart flow.

## Stable URL patterns

Spreadshirt Help documents the showroom URL pattern directly:

```text
https://www.spreadshirt.com/shop/user/<SpreadName>
```

The locale can vary by regional domain:

```text
https://www.spreadshirt.com/shop/user/<SpreadName>
https://www.spreadshirt.de/shop/user/<SpreadName>
https://www.spreadshirt.es/shop/user/<SpreadName>
```

The seller's **Spread Name** is the showroom name and defines the showroom URL.

For the interactive designer, Spreadshirt Help consistently refers users to the
**Customize Tool** / **Product Designer** from a product flow rather than
documenting one canonical deep-link format. For automation, use a real product
page and enter the designer through the visible customize controls instead of
guessing an internal route.

## Customize Tool behavior

Spreadshirt Help documents a few durable rules that are important for browser
automation:

- the printable area is shown as a square
- a **red** design frame means the design cannot be printed
- a **green** design frame means the placement is valid
- changing the product can push a design out of the printable area
- some products accept only specific print methods and file types

If automation changes product type, color, or uploaded art, re-check the print
area and placement before assuming the design is still valid.

## Product-designer notes

- Product changes preserve design state where possible. The Help flow for
  "Design more products" says the design is carried into the next product and
  may need manual size or orientation adjustment.
- Print method is not always editable. Some materials or file formats lock the
  available print type.
- Certain products require vector uploads for flex or flock printing. JPG, PNG,
  and GIF uploads can be rejected on those products.
- The visual mockup is a standard-size preview, not a guarantee of exact final
  sizing across all garment sizes.

## Marketplace and AI notes

- Spreadshirt's AI customization feature lives inside the Product Designer's
  design area and on customizable marketplace designs.
- Spreadshirt Help is explicit that this AI customization feature does **not**
  apply to Spreadshop.
- If you are automating a designer flow and expect AI tools, verify that you
  are on a marketplace customization surface, not a Spreadshop-specific one.

## Good starting workflows

### Open a seller showroom

- Use the known showroom URL based on Spread Name.
- Browse catalog and seller metadata from that public surface before opening an
  individual product.

### Customize a product

- Open the product page in a real browser.
- Enter the Customize Tool from the visible customize/design control.
- After every product or design change, confirm the design frame is valid and
  the print method still matches the uploaded asset type.

### Reuse a design across products

Spreadshirt Help explicitly supports the "Design more products" flow. After
adding one customized item to the cart, use that path to continue with the same
design on another product, then verify placement again.

## Traps

- Showroom and Product Designer are different surfaces; do not assume the same
  selectors or URL rules apply to both.
- A valid placement on one product can become invalid after switching to a
  smaller or differently shaped product.
- Some products silently narrow allowed upload formats because of print-method
  restrictions.
- Marketplace AI customization does not imply the same feature exists in
  Spreadshop.
