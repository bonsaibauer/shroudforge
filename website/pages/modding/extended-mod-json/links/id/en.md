# The `links.<id>` key

The ID is the key inside `links`. Modloader displays only IDs registered by the UI. The IDs and their order are in [order.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/modules/modloader-ui/ui/src/links/order.json).

Currently supported IDs include `source`, `source-github`, `source-gitlab`, `source-codeberg`, `issues`, `wiki`, `website`, `store`, and the `support-*` IDs. An unknown key can pass the schema but is not registered as a visible button. The URL must use HTTPS.
