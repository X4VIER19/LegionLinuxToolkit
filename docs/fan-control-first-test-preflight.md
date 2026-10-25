# Preparación one-shot de la curva predeterminada

Herramienta exclusivamente simulada, sin lecturas de host ni invocaciones:

```bash
cargo run --locked -p legion-cli --features experimental-fan-control-preflight \
  --bin legion-cli-fan-control-preflight -- --simulate --format json
```

`--scenario complete-fixture` modela requisitos satisfechos; no acredita el
host y nunca permite ejecutar. También existen `missing-evidence`, `extreme`,
`full-speed` y `wrong-host`. No admite rutas, modo real, payload arbitrario,
sondeo ni fallback. El payload se obtiene de OfflineCurveProposal y el
destino de BlockedFanControlPlan. La evaluación utiliza el registro existente.

## Qué puede comprobarse antes de escribir

| Requisito | Vía Linux futura | Evidencia manual/Windows pendiente |
|---|---|---|
| DMI exacto | Reutilizar proveedor fijo de cuatro atributos | No se requiere Windows para DMI |
| Personalizado | platform_profile=custom mediante lector existente | GZ44=3 y ausencia de transición concurrente; `custom` solo no acredita todos los estados EC |
| FanFullSpeed=false | No existe lector validado de esta capacidad en el proyecto | Consultar estado LLT; una observación Windows anterior no acredita el estado actual en Linux |
| Variante efectiva LLT | No detectada; no inferir del editor | MachineInformation y controlador seleccionado en LLT 2.34.0: SmartFanVersion, LegionZoneVersion, flags y precedencia |
| Dos RPM | Reutilizar hwmon con procedencia yogafan_83ex | Cada consulta fan_input puede ejecutar ACPI: requiere autorización separada |
| Temperaturas actuales | Futuro lector validado hwmon/thermal con labels/unidades | No usar cpu_temp/gpu_temp de lenovo_wmi_other: son límites. Falta acreditar sensores, frescura y umbrales de aborto |
| Catálogos | PMID/PGID/DCID y selección FNT/FI aún sin adquisición validada | Correlacionar con firmware específico; no extrapolar catálogo por modelo |
| Alimentación AC | power_supply candidato; lector no integrado aquí | Confirmación manual; verificar antes de ejecutar |
| Recuperación | Sin operación Linux de rollback validada | Preparar LLT en Windows y revisar restauración del preset |

Leer metadatos DMI o enlaces de driver es diferente de leer atributos de
sensores o invocar métodos. Ninguna consulta real está autorizada por ejecutar
esta simulación. Los proveedores reales siguen requiriendo sus confirmaciones
y autorizaciones independientes; esta herramienta no los invoca.

## Diseño del camino real pendiente

Antes de cualquier activación se necesita una modificación explícita y
revisable: proveedor de precondiciones observadas y recientes, descarte de
cambios concurrentes, endpoint kernel restringido al preset, permisos y
confirmación literal de método/instancia/payload, una guardia por intento y
supervisión independiente. No bastan flags declarados por el usuario ni un
informe simulado. El transporte debe verificar de nuevo las precondiciones
antes de llamar y consumir la autorización antes del intento, incluso si falla.
No reintentar tras respuesta ambigua o error. No aplicar cambios al cargar,
detectar o descargar el módulo. Se conserva la constante kernel `false`.

La primera operación propuesta sería exclusivamente instancia 0, método 6,
64 bytes oficiales de [1,2,3,4,5,6,7,8,9,10], con Personalizado/GZ44=3,
FanFullSpeed=false, AC y supervisión ya acreditadas. No se cambian estos
estados automáticamente para satisfacer la prueba. La autorización de carga,
las consultas previas y la escritura deben detallar su alcance por separado.

## Qué permitiría evaluar la aplicación

Registrar estado previo, bytes exactos, respuesta ACPI, observaciones posteriores
y ausencia de cambios de perfil. Integer(0) no acredita aceptación: WMAB también
lo devuelve si omite SFAN. GFAN devuelve los índices almacenados o EFST según
GZ44; además puede inicializar mediante INIF si F10A=0. No usarlo como readback
inocuo universal. Un readback coincidente solo acredita parte de los destinos,
no todos los campos FI ni la semántica CMRD.

Con el preset ya activo, no observar cambios es un resultado esperado y no
prueba de ejecución. RPM coherentes y temperaturas dentro de límites son
comprobaciones de funcionamiento, no demostración de la curva aplicada.
Valores iguales en los dos canales no prueban independencia. Ningún fallo
justifica repetir la escritura para intentar obtener un cambio visible.

## Recuperación manual propuesta en Windows, aún por ensayar y autorizar

1. Antes de probar Linux, confirmar que Windows 11 arranca, LLT 2.34.0 funciona
   en este equipo y conserva los ajustes iniciales; guardar una copia del preset
   actual con la función disponible en esa instalación y anotar su nombre.
2. Localizar Personalizado/GodMode y el preset predeterminado en LLT. Revisar
   sus parámetros de potencia/temperatura y overclock: aplicar un preset puede
   modificar más que ventiladores. Si no hay preset inequívoco, abortar y
   documentar los controles reales de esa versión antes de continuar.
3. Ante recuperación autorizada: iniciar Windows, abrir LLT, seleccionar el
   preset revisado con FanFullSpeed desactivado y curva [1..10], aplicar mediante
   su interfaz y comprobar ventiladores/temperaturas. El paso cambia firmware
   y necesita aprobación específica también en Windows.
4. Volver al perfil automático normal previamente documentado mediante LLT,
   solo con autorización para ese cambio; registrar lo observado. No usar
   scripts EC, Fan_Set_Table directo ni resets improvisados.

Reiniciar puede tardar y no garantiza restablecer el EC; el procedimiento LLT
tampoco demuestra rollback de una escritura parcial. Ante anomalía térmica se
necesita un protocolo de interrupción revisado antes de la primera prueba.
Fn+Q y descargar módulos no son recuperación garantizada. Sin ese protocolo,
sensores validados, variante y estado actual completo, la escritura sigue
bloqueada aunque el payload y el transporte estén preparados.
