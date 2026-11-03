# El inicio se detiene por copias de seguridad de recursos incompatibles

Solución de problemas para versiones antiguas de ShroudForge. Las versiones 1.6.8, 1.7.0 y 1.7.3 pueden detener el inicio antes de abrir Enshrouded si sus copias de seguridad no coinciden con los archivos del juego instalados.

<!-- wiki-category: troubleshooting -->

## Por qué se detiene el inicio

ShroudForge conserva copias de trabajo de `enshrouded.kfc` y `enshrouded.kfc_resources` para restaurar los archivos del juego antes de preparar mods de recursos. Los archivos `.bak` pueden proceder de una versión anterior de Enshrouded o de otro mod. Si la copia KFC no coincide con el archivo actual del juego, las versiones antiguas de ShroudForge no pueden restaurarla con seguridad y detienen el inicio.

## Si usas ShroudForge 1.7.5 o una versión posterior

Cuando la copia KFC es incompatible, estas versiones pueden mover ambas copias de trabajo a la papelera de reciclaje de ShroudForge y crear copias nuevas a partir de los archivos actuales del juego. Esta recuperación requiere `shroudforge/shroudforge-updater.exe` en la instalación de Enshrouded. Si el registro indica que el actualizador no está disponible, repara o reinstala ShroudForge con el paquete completo y vuelve a iniciar el juego. Mantén los archivos `.bak` mientras reparas la instalación.

## Solución manual para las versiones 1.6.8, 1.7.0 y 1.7.3

Sigue estos pasos únicamente con las versiones antiguas de ShroudForge indicadas.

1. Cierra ShroudForge y Enshrouded por completo, incluido cualquier proceso de servidor dedicado.
2. Abre la carpeta de instalación de Enshrouded. Una ruta habitual de Steam es `SteamLibrary\steamapps\common\Enshrouded`, pero usa la biblioteca donde instalaste el juego.
3. Si otro mod pudo modificar los archivos `.kfc` actuales, verifica primero los archivos de Enshrouded desde Steam. Si quieres conservar una copia de las copias antiguas, guárdalas fuera de la carpeta del juego antes de continuar.
4. Elimina únicamente estos archivos, si existen:
   - `enshrouded.kfc.bak`
   - `enshrouded.kfc_resources.bak`
5. Vuelve a iniciar ShroudForge. Debería crear copias de trabajo nuevas a partir de los archivos actuales del juego.

No elimines los archivos del juego `.kfc` o `.kfc_resources` ni ninguna otra copia de seguridad. Si el inicio sigue fallando, conserva los demás archivos y envía el registro de ShroudForge al soporte.
