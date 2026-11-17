---
{
  "id": "link-video-youtube",
  "locale": "en",
  "title": "Link ID `video-youtube`",
  "summary": "Recognized Modloader button for video-youtube.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Link IDs",
      "Supported IDs"
    ]
  }
}
---

# Link ID `video-youtube`

Use this ID in `extended.mod.json → links.video-youtube` and provide an HTTPS address. The Modloader displays the link with its registered icon and translated label.

```json
{
  "links": {
    "video-youtube": "https://www.youtube.com/@example"
  }
}
```

The IDs and their order are defined in [order.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/modules/modloader/mod_link_order.json).
