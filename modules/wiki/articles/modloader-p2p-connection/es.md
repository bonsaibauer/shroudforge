# Conexión P2P del Modloader

Descubre cuándo el Modloader puede conectarse a un servidor dedicado mediante Steam P2P y qué permisos de los grupos de usuarios de Enshrouded permiten administrar el servidor.

## Conectarse dentro del juego

1. Inicia Enshrouded y entra en un mundo de juego alojado por el servidor dedicado.
2. Abre el Modloader y selecciona **Servidor** como destino.
3. Espera hasta que el estado del servidor indique **Conectado**. La ventana de escritorio y el menú principal no confirman una conexión P2P dentro de un mundo, por lo que el campo de contraseña del servidor permanece oculto.
4. Introduce la contraseña de una entrada válida de `userGroups`. El servidor comprueba la contraseña mediante la conexión P2P.

## Permisos necesarios para el grupo del servidor

La contraseña introducida debe coincidir exactamente con un grupo que tenga un `name` no vacío. Ese mismo grupo debe establecer los cinco permisos siguientes en el valor booleano JSON `true`. Si falta un campo, es `false` o tiene otro tipo de dato, el grupo no podrá administrar el servidor desde el Modloader.

```json
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
```

`reservedSlots` define plazas reservadas para jugadores y no forma parte de la comprobación de permisos del Modloader. ShroudForge envía la contraseña al servidor para validarla y no la almacena. El servidor vuelve a comprobar los permisos del grupo al procesar acciones administrativas. Cinco intentos fallidos activan una espera de 60 segundos antes de volver a iniciar sesión.
