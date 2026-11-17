---
{
  "id": "extension-link-id",
  "locale": "de",
  "title": "Schlüssel `links.<id>`",
  "summary": "Die Link-ID entscheidet, welche bekannte Schaltfläche der Modloader anzeigt.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Link-IDs"
    ],
    "overview": true,
    "label": "Übersicht"
  }
}
---

# Schlüssel `links.<id>`

Die ID ist der Schlüssel innerhalb von `links`. Der Modloader zeigt nur IDs an, die in seiner Liste registriert sind. Die IDs und Reihenfolge stehen in [order.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/modules/modloader/mod_link_order.json).

Die aktuell unterstützten IDs umfassen `source`, `source-github`, `source-gitlab`, `source-codeberg`, `issues`, `wiki`, `website`, `store`, `community-discord`, `video-youtube` und die `support-*`-IDs. Ein unbekannter Schlüssel kann das Schema bestehen, wird aber nicht als sichtbarer Button registriert. Die Adresse muss HTTPS verwenden.
