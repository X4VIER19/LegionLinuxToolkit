# Modo Lenovo y transporte de control en preparación

## Correspondencia demostrada en NUCN15WW

DSDT `WMAA`, método 0x2C (38769), y 0x37 (39132):

| Modo | Argumento Lenovo | GZ44 |
|---|---|---|
| Reposo/Quiet | 1 | 2 |
| Equilibrado | 2 | 0 |
| Rendimiento | 3 | 1 |
| Personalizado/Custom | FF | 3 |
| Extremo | E0 | 7 |

**Corrección de la incertidumbre anterior:** `GZ44=7` corresponde a Extremo,
no a Personalizado. `WMAB` método 6 omite SFAN en Extremo; devuelve Integer(0)
tanto al omitirlo como tras ejecutarlo. El retorno no acredita aplicación.
El motivo de diseño de Lenovo para excluir Extremo no está documentado.

En alimentación AC, la entrada Custom guarda el perfil previo en RXMS/WXMS
0x7A cuando procede, escribe GZ44=3 y WXMS(0x76,3), ejecuta SSFM(0,3), ajusta
0x7B y escribe WXMS(0x65,0x11). La ruta _Q0E (10547), asociada al cambio de
perfil, recorre estados según alimentación y RXMS(0x7B), llama SSFM y emite
notificaciones D5/D9. No acredita rollback íntegro de SFAN por pulsar Fn+Q.

## LLT exacto inspeccionado, sin copiar código GPLv3

Revisión 6f19ef48095a32afe439474a65e5b95cf8fa1b24, repositorio oficial actual
LenovoLegionToolkit-Team/LenovoLegionToolkit. Se consultaron estáticamente:

- [PowerModeFeature.cs](https://github.com/LenovoLegionToolkit-Team/LenovoLegionToolkit/blob/6f19ef48095a32afe439474a65e5b95cf8fa1b24/LenovoLegionToolkit.Lib/Features/PowerModeFeature.cs): SetSmartFanMode mediante base WMI; la base usa offset 1. Tras cambiar el estado notifica PowerModeListener. EnsureGodModeStateIsApplied llama al controlador solo si el estado consultado es GodMode.
- [GodModeController.cs](https://github.com/LenovoLegionToolkit-Team/LenovoLegionToolkit/blob/6f19ef48095a32afe439474a65e5b95cf8fa1b24/LenovoLegionToolkit.Lib/Controllers/GodMode/GodModeController.cs): prioridad V1, V2, V3, V4, primer SupportsGodModeVn verdadero; luego mantiene el controlador seleccionado.
- [Compatibility.cs](https://github.com/LenovoLegionToolkit-Team/LenovoLegionToolkit/blob/6f19ef48095a32afe439474a65e5b95cf8fa1b24/LenovoLegionToolkit.Lib/Utils/Compatibility.cs): todos requieren soporte GodMode. V1 usa SmartFan 4/5 o LegionZone 1/2 y exclusiones de BIOS; V2 SmartFan 6/7 o LegionZone 3/4; V3 requiere SmartFan 8/9 o LegionZone 5/6, generación >=10 y series/modelos específicos; V4 SmartFan 8/9 o LegionZone 5/6. Los OR y el orden importan si hay versiones discordantes. No basta el editor.
- [GodModeControllerV2.cs](https://github.com/LenovoLegionToolkit-Team/LenovoLegionToolkit/blob/6f19ef48095a32afe439474a65e5b95cf8fa1b24/LenovoLegionToolkit.Lib/Controllers/GodMode/GodModeControllerV2.cs): si se aplica una curva, intenta SetFanFullSpeed(false) antes de FanSetTable; captura errores, de modo que ese intento no garantiza el estado. Linux deberá exigir estado false confirmado y abortar si falta. V2 también exige gestionar conflictos con Vantage/LegionZone.
- [Enums.cs](https://github.com/LenovoLegionToolkit-Team/LenovoLegionToolkit/blob/6f19ef48095a32afe439474a65e5b95cf8fa1b24/LenovoLegionToolkit.Lib/Enums.cs): PowerModeState GodMode=254, Extreme=223; el offset WMI explica FF/E0. FanFullSpeed tiene CapabilityID 0x04020000.

En el DSDT, la ruta de esa capacidad consulta FFON (40334) y lo cambia
(40889/40893). No se implementa ese cambio en Linux. La selección del
controlador real del 83EX requiere el registro MachineInformation/Controller
de LLT de esa revisión: versiones SmartFan/LegionZone, modelos y flags.
Los cuerpos V1/V3/V4 y la entrega concreta de eventos del listener requieren
revisión adicional antes de atribuirles la secuencia V2 completa.

## Transporte C en esta etapa

CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING está desactivado por
defecto. Compila una función interna sin llamadores ni endpoint, protegida
además por una constante de autorización false, sin setter ni parámetro de
módulo. Habilitar compilación nunca habilita invocación. El driver no cambia
el modo ni FanFullSpeed. Init/probe/remove no llaman esta función.

Se acepta únicamente el payload de 64 bytes del serializador Rust para
[1,2,3,4,5,6,7,8,9,10]. Se comprueban los 64 bytes, identidad DMI exacta,
modo 3 y FanFullSpeed=0; después, guardia atómica de un intento, mutex WMI,
instancia 0/método 6 y liberación del objeto ACPI incluso ante error.
Los argumentos de modo/FullSpeed son aún datos de preparación: no son una
prueba del host. Su adquisición independiente deberá implementarse antes
de retirar la barrera. -EIO distingue fallo WMI y -EPROTO respuesta inesperada;
Integer(0) solo valida respuesta, no éxito físico. No hay ioctl ni write.

Pruebas aisladas: `control_policy_test.c` usa la misma política C que el
transporte y verifica 72 casos (preset, identidad, modo, FullSpeed, longitud,
puntero nulo y alteración individual de cada byte). Las 89 pruebas relevantes
Rust siguen aprobadas. La operación permanece bloqueada estructuralmente por
la constante false y ausencia de llamadores; no se ejecuta el módulo en tests.
Los errores WMI y la liberación de output/mutex se revisaron estáticamente:
no se afirma cobertura dinámica mediante inyección de fallos del kernel.
La compilación aislada tuvo avisos de pahole/BTF ausente, sin errores C.

## Condiciones antes de la primera aplicación

Se requieren: DMI exacto; controlador LLT efectivo identificado; AC y
Personalizado GZ44=3 confirmado; FanFullSpeed=false confirmado; PMID/PGID/DCID
y catálogos establecidos; revisión del endpoint futuro y sus permisos;
supervisión RPM/temperaturas validada; ausencia de cambios concurrentes;
payload exclusivamente predeterminado y una autorización específica.

Abortar antes de invocar si cualquier estado es desconocido, incompatible,
Extremo, hay error de identidad, FullSpeed activo o fallo de supervisión.
Después de un fallo no reintentar. El objeto ACPI se libera, pero eso no
revierte el EC. Descargar el módulo tampoco restaura curvas.
Windows y LLT deben estar disponibles con el procedimiento oficial de
restauración revisado. Reiniciar a Windows demora la recuperación y aún
no demuestra rollback ante aplicación parcial; Fn+Q no está acreditado
como sustituto. Estos bloqueos impiden autorizar ya la primera escritura.
