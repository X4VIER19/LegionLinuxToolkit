# Contrato LLT y preparación de control experimental

Evidencia aportada por el propietario: LLT 2.34.0.0, commit
`6f19ef48095a32afe439474a65e5b95cf8fa1b24`, usa MethodId 6 con un payload
de 64 bytes. Esta etapa implementa independientemente el contrato indicado;
no incorpora código GPLv3 de LLT. La consulta pública de ese commit no pudo
recuperarse durante esta etapa; no se presenta como una inspección nueva de
su implementación. Los golden bytes son una fixture derivada del contrato
documentado, no una captura binaria de Windows.

## Modelo y bytes

`OfflineCurveProposal` exige elegir expresamente las reglas declaradas
`DeclaredLltV2ToV4`. No detecta la variante del equipo. Reutiliza el parser
de los primeros 52 bytes de SFAN y sus límites conservadores, y después exige
los mínimos software `[1,1,1,1,1,1,1,1,3,5]`, máximo 10 y orden no decreciente.
Estas reglas son de LLT, no garantías térmicas ni validaciones del firmware.
Solo se incorpora el preset predeterminado respaldado por la evidencia;
otros presets necesitan sus valores documentados antes de añadirse.

El serializador siempre genera selector 1, cero en 1–5 y 26–63, y diez
UInt16 little-endian en 6–25. Ejemplo predeterminado, filas de 16 bytes:

```text
01 00 00 00 00 00 01 00 02 00 03 00 04 00 05 00
06 00 07 00 08 00 09 00 0a 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
```

`BlockedFanControlPlan` reutiliza GUID, MethodId y la instancia del transporte
de lectura. Su feature no predeterminada solo expone el plan offline: no hay
método de ejecución, endpoint de escritura, extensión del WmiTransport de
lectura ni control en CLI. `execution_enabled()` siempre devuelve false.

## Condiciones pendientes

- Identificar realmente la variante LLT de GodMode y documentar su secuencia
  de aplicación. Declarar reglas V2–V4 en una prueba no verifica el host.
- Determinar el estado de Personalizado/GodMode antes y después de la llamada.
  En el DSDT, `WMAB` omite SFAN si GZ44=7 y devuelve cero de todos modos;
  no asumir que permanecer en Personalizado produce una escritura efectiva.
- Confirmar FanFullSpeed desactivado y comprender su interacción con SFAN.
  No se implementa su modificación ni se atribuye automáticamente al campo
  FFON sin establecer el recorrido completo de LLT y firmware.
- Establecer PMID/PGID/DCID y los catálogos FNT/FI efectivos. Selector 1 toma
  el paquete FNT 6; el catálogo depende del estado del equipo. Las unidades
  físicas y todos los límites térmicos siguen incompletos.
- Resolver CMRD, aplicación parcial y restauración. El preset predeterminado
  de índices no garantiza restaurar todos los parámetros EC ni un modo
  automático. Ver `sfan-control-evidence.md`.

## Procedimiento futuro para revisión, todavía no ejecutable

1. Con aprobación separada, registrar identidad exacta, versión del módulo,
   perfil/GodMode, FanFullSpeed, catálogos y estado inicial. Resolver primero
   las consultas que pueden tener efectos de inicialización.
2. Preparar Windows 11 y LLT funcionales como vía de recuperación: identificar
   previamente las acciones oficiales para restaurar la curva predeterminada
   y el perfil automático. El dual boot requiere reiniciar y no recupera
   inmediatamente una configuración peligrosa en Linux.
3. Definir temperaturas máximas, comportamiento RPM mínimo, supervisión
   independiente validada de ambos canales y cancelación antes de la llamada.
   Los umbrales deben tener evidencia; este documento no inventa cifras.
4. Revisar el futuro diff de escritura y autorizar por separado método 6,
   instancia, 64 bytes exactos y una única invocación. No habrá reintentos,
   fallback, inicio automático ni escrituras al cargar/descargar el driver.
5. Ante resultado incierto, detener cambios y aplicar el protocolo de
   recuperación aprobado. No repetir ni usar Fn+Q como rollback garantizado.
   Registrar respuesta, RPM, temperaturas y resultado de restauración.

No se proporciona un comando de escritura: no existe transporte invocable.
Restaurar con LLT requiere validación independiente; apagar o reiniciar no
es una garantía demostrada de reset EC. Estos puntos bloquean la primera
escritura, aunque la generación offline del contrato ya está implementada.
