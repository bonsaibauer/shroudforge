---
{
  "id": "extension-link-id",
  "locale": "en",
  "title": "The `links.<id>` key",
  "summary": "The link ID selects which recognized button the Modloader displays.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Link IDs"
    ],
    "overview": true,
    "label": "Overview"
  }
}
---

# The `links.<id>` key

The ID is the key inside `links`. Modloader displays only IDs registered by the UI. The IDs and their order are in [order.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/modules/modloader/mod_link_order.json).

Currently supported IDs include `source`, `source-github`, `source-gitlab`, `source-codeberg`, `issues`, `wiki`, `website`, `store`, `community-discord`, `video-youtube`, and the `support-*` IDs. An unknown key can pass the schema but is not registered as a visible button. The URL must use HTTPS.
