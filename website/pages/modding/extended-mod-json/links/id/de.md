# Schlüssel `links.<id>`

Die ID ist der Schlüssel innerhalb von `links`. Der Modloader zeigt nur IDs an, die in seiner Liste registriert sind. Die IDs und Reihenfolge stehen in [order.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/modules/modloader-ui/ui/src/links/order.json).

Die aktuell unterstützten IDs umfassen `source`, `source-github`, `source-gitlab`, `source-codeberg`, `issues`, `wiki`, `website`, `store` und die `support-*`-IDs. Ein unbekannter Schlüssel kann das Schema bestehen, wird aber nicht als sichtbarer Button registriert. Die Adresse muss HTTPS verwenden.
