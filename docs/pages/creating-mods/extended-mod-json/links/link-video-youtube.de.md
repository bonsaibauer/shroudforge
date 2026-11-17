---
{
  "id": "link-video-youtube",
  "locale": "de",
  "title": "Link-ID `video-youtube`",
  "summary": "Bekannte Modloader-Schaltfläche für video-youtube.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Link-IDs",
      "Unterstützte IDs"
    ]
  }
}
---

# Link-ID `video-youtube`

Diese ID wird in `extended.mod.json → links.video-youtube` verwendet. Trage darunter eine HTTPS-Adresse ein. Der Modloader zeigt den Link mit dem zugehörigen Symbol und übersetzten Namen.

```json
{
  "links": {
    "video-youtube": "https://www.youtube.com/@example"
  }
}
```

Die IDs und ihre Reihenfolge sind in [order.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/modules/modloader/mod_link_order.json) definiert.
