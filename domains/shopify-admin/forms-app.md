# Shopify Forms App

The Forms app at `admin.shopify.com/store/<store>/apps/shopify-forms` uses two
`forms.shopifyapps.com` iframes: the form editor and an App Bridge modal host.
Target the editor iframe for fields and the embedded modal iframe for discount
selection.

## Discount Attachment Trap

Attaching a discount overwrites teaser title, form title, success title, and
success content with generic copy. Restore all four fields before saving.

Text inputs respond reliably to focus, selection, and `type-text`. Rich-text
content uses a Lexical-style editor: triple-click the paragraph and enter text
through real keyboard events. Verify its visible character counter before
saving, then verify the saved toast.
