# Modloader P2P connection

Learn when the Modloader can reach a dedicated server over Steam P2P and which Enshrouded user-group permissions qualify for server administration.

## Connect in game

1. Start Enshrouded and join a game world hosted by the dedicated server.
2. Open the Modloader and select **Server** as the target.
3. Wait for the server status to show **Connected**. The desktop window and the game’s main menu do not confirm an in-world P2P connection, so the server password field stays hidden.
4. Enter the password for a qualifying `userGroups` entry. The server checks the password over the P2P connection.

## Required server group permissions

The entered password must exactly match a group with a non-empty `name`. That same group must set all five permission fields below to the JSON boolean `true`. A missing field, `false`, or a non-boolean value does not qualify the group for Modloader server administration.

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

`reservedSlots` controls reserved player slots and is not part of the Modloader permission check. ShroudForge sends the submitted password to the server for validation and does not store it. The server checks the group permissions again when it handles admin actions. Five failed password attempts start a 60-second login cooldown.
