# Conexión P2P del Modloader

Esta página explica cómo llega el Modloader a un servidor dedicado mediante Steam P2P, qué cifra Steam, cómo se intercambian mensajes entre cliente y servidor y qué permisos de los grupos de usuarios de Enshrouded permiten administrar el servidor.

## Conectarse dentro del juego

1. Inicia Enshrouded y entra en un mundo de juego alojado por el servidor dedicado.
2. Abre el Modloader y selecciona **Servidor** como destino.
3. Espera hasta que el estado del servidor indique **Conectado**. Esto significa que el cliente recibió recientemente un mensaje de estado de Steam P2P. No verifica que el remitente sea el servidor de Enshrouded previsto. La ventana de escritorio y el menú principal no confirman una conexión P2P dentro de un mundo, por lo que allí el campo de contraseña permanece oculto.
4. Introduce la contraseña de una entrada válida de <code>userGroups</code>. El servidor comprueba la contraseña mediante la conexión P2P.

## Cómo funciona la conexión P2P

El módulo Network se ejecuta dentro del proceso del cliente de Enshrouded o del servidor dedicado. El juego inicializa Steam y ShroudForge reutiliza la interfaz Steam Networking Messages ya cargada en ese proceso para enviar directamente a una identidad de Steam. ShroudForge no ejecuta un servidor de retransmisión ni incluye estos mensajes en los paquetes de juego de Enshrouded. Mientras el módulo Network está habilitado, el runtime procesa el estado P2P y los servicios remotos del Modloader. Consulta el [ciclo del runtime](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/mod-runtime/ingame/update_ingame_runtime.rs) y el [adaptador nativo de Steam](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/native/steam_network.cpp).

## Cifrado del transporte y canales

Las solicitudes y respuestas del Modloader se codifican como bytes JSON. Los mods del runtime pueden enviar cadenas Lua binarias. ShroudForge no añade aquí una sesión TLS propia, un cifrado del contenido ni un intercambio de claves de extremo a extremo. El código depende de Steam Networking para autenticar al peer y cifrar el transporte. Valve documenta que <code>ISteamNetworkingMessages</code> utiliza las conexiones subyacentes de <code>ISteamNetworkingSockets</code> y describe el cifrado y la autenticación de Steam Networking. Esto protege el transporte hasta una identidad de Steam. No demuestra que el peer sea el servidor de Enshrouded previsto ni protege los extremos. Los números de canal solo enrutan mensajes y no aplican autorización. La API Lua acepta los canales del 0 al 65533 y rechaza los dos canales reservados. Los mensajes internos del Modloader solicitan entrega fiable. Esto afecta a la entrega y el orden, pero no es cifrado ni autorización. Consulta la documentación oficial de [ISteamNetworkingMessages](https://partner.steamgames.com/doc/api/ISteamNetworkingMessages) y [ISteamNetworkingSockets](https://partner.steamgames.com/doc/api/ISteamNetworkingSockets), además de las [definiciones de canales del transporte](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/lib.rs) y la [API Lua de Network](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/lua/network.rs).

| Canal | Uso actual |
|---|---|
| <code>0</code> a <code>65533</code> | Mensajes entre mods Lua del runtime |
| <code>65534</code> | Réplica interna del servidor y control remoto del Modloader |
| <code>65535</code> | Comprobaciones del estado Steam P2P |

## Flujo de mensajes entre cliente y servidor

| Dirección | Qué envía el código actual |
|---|---|
| Cliente al servidor | El cliente selecciona <code>modules/network/serverSteamId</code> o, si el ajuste está vacío, utiliza la última Steam ID de servidor remoto descubierta. Envía una suscripción aproximadamente cada 10 segundos. Después puede enviar la contraseña de administrador y, luego, un token de sesión con cambios de ajustes, solicitudes de lista de mods y operaciones de mods. El servidor elimina a los peers que dejan de renovar durante 30 segundos. |
| Servidor al cliente | Una nueva suscripción recibe de inmediato una instantánea de la configuración del loader y los últimos 64 KiB de cada registro replicado. Después, el servidor envía cambios de configuración y fragmentos sin procesar de los registros a los peers suscritos. Las solicitudes autenticadas como administrador también reciben listas de mods, datos de compatibilidad del servidor y resultados de sesiones y operaciones. |
| Mod del runtime a otro mod | Un mod Lua envía el mensaje a una Steam ID explícita. <code>send_mod</code> añade un sobre JSON y elige un canal determinista a partir del ID del mod destinatario. El receptor comprueba el protocolo y el ID de destino. El valor <code>from_mod</code> lo proporciona el mod remitente y no demuestra qué mod creó el mensaje. |

El cliente envía las solicitudes locales de la interfaz a través de un puente dentro del proceso. El proceso del juego realiza el envío por Steam. En el servidor, la identidad Steam del remitente debe estar en el conjunto de peers autorizados actual. El adaptador nativo construye este conjunto a partir de eventos del registro del servidor dedicado sobre peers conectados y autenticación de Steam. El ajuste opcional <code>allowedClientSteamIds</code> restringe más ese conjunto. Si está vacío, no añade una restricción adicional de lista permitida. El servidor envía la instantánea y los registros antes del inicio de sesión de administrador, por lo que esta comprobación de peers también controla el acceso a esos datos. Consulta el [servicio de interfaz remota](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/remote_ui.rs), el [puente local](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/local_bridge.rs) y la [lista permitida de Network](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/read_network_allowlist.rs).

## Permisos necesarios para el grupo del servidor

La contraseña introducida debe coincidir exactamente con un grupo que tenga un <code>name</code> no vacío. Ese mismo grupo debe establecer los cinco permisos siguientes en el valor booleano JSON <code>true</code>. Si falta un campo, es <code>false</code> o tiene otro tipo de dato, el grupo no podrá administrar el servidor desde el Modloader.

~~~json
"userGroups": [
  {
    "name": "Admin",
    "password": "REPLACE_WITH_A_PRIVATE_PASSWORD",
    "canKickBan": true,
    "canAccessInventories": true,
    "canEditWorld": true,
    "canEditBase": true,
    "canExtendBase": true,
    "reservedSlots": 0
  }
]
~~~

<code>reservedSlots</code> define plazas reservadas para jugadores y no forma parte de la comprobación de permisos del Modloader. ShroudForge envía la contraseña en una solicitud JSON al peer Steam seleccionado y no la almacena. El servidor la compara directamente con la contraseña de <code>enshrouded_server.json</code>. Este código no aplica hash ni cifrado a ese valor de configuración en reposo. Tras validarla, el servidor genera un token vinculado al peer Steam y lo conserva en memoria en ambos extremos. Las escrituras administrativas vuelven a comprobar el token, el peer y los permisos del grupo. Cinco intentos fallidos activan una espera de 60 segundos antes de volver a iniciar sesión. Consulta el [código de autorización del servidor](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/remote_ui.rs).

## Nota de seguridad

Si <code>serverSteamId</code> está vacío, el cliente puede usar el último peer descubierto mediante un mensaje de estado del canal <code>65535</code>. Este ajuste está vacío de forma predeterminada. Un mensaje recibido que comience por <code>SFN1:PING:</code> actualiza ese ID descubierto. **Conectado** refleja tráfico de estado reciente, no una identidad verificada del servidor de Enshrouded. Si un peer puede establecer una sesión de mensajes Steam y enviar ese contenido de estado, podría seleccionarse como destino. El cifrado de Steam protegería entonces la conexión con ese peer, pero no demostraría que sea el servidor previsto. Comprueba o fija la Steam ID del servidor antes de introducir la contraseña. Considera también de confianza a los peers suscritos para los datos de configuración y registros que reciben antes del inicio de sesión de administrador. Los mods del runtime que reciben mensajes de <code>send_mod</code> deben validar por su cuenta el peer Steam, el mod remitente declarado y el contenido. Son observaciones del código fuente, no una confirmación de explotación en directo. Consulta la [política de seguridad de ShroudForge y las instrucciones para informar de forma privada](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/SECURITY.md).
