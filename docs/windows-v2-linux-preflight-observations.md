# Referencia V2 y estado registrado en Nobara — 2026-10-24

## Windows: evidencia aportada por el propietario

83EX/NUCN15WW, LLT 2.34.0: SmartFanVersion=7, LegionZoneVersion=4,
controlador GodMode V2, SupportedPowerModes=0x0001001F y
GodModeFnQSwitchable=1. FanFullSpeed=0 **en aquella captura Windows**.
Catálogo Custom Mode=255: coordenadas (1,4), (2,5), (1,1), valores nominales
[0,1700,1900,2100,2400,2900,3200,3500,3800,5000] RPM. Son valores de catálogo,
no mediciones actuales ni una demostración de selección FNT/FI en Linux.
No se asigna una identidad física CPU/GPU a los canales desde estas parejas.

Se aplicó Predefinido → Guardar en Personalizado con FullSpeed desactivado:
RPM observadas 1700–2400. Una segunda aplicación no produjo error ni cambio
visible; no hubo anomalías reportadas. Esto valida ese flujo funcional en
Windows. No se probó recuperación tras una escritura parcial ni se demuestra
que un reboot restablezca el EC. No se etiqueta la recuperación como validada.

El preflight incorpora esta referencia, verifica su coherencia con V2, los
mínimos LLT existentes y el preset [1..10]. No crea un registro alternativo
de modelos ni confunde el catálogo Windows con los paquetes AML efectivos.

## Linux: consultas de información realizadas en esta tarea

Se leyeron los cuatro atributos DMI fijos y las dos interfaces platform_profile;
se enumeraron nombres de dispositivos y atributos; se consultaron type/online
de power_supply, etiquetas térmicas y enlaces de módulo. No se abrieron
fan*_input ni temp*_input ni se invocaron métodos WMI/ACPI/EC explícitos.
Las lecturas platform_profile se atienden mediante el controlador kernel:
no se presentan como mera lectura de metadatos sin posibilidad de firmware.

| Requisito | Resultado registrado | Alcance |
|---|---|---|
| DMI | LENOVO / 83EX / Legion Slim 5 16ARP9 / NUCN15WW | Coincide; observación, no autorización |
| AC | ACAD type=Mains, online=1 | Conectada en la captura |
| Perfil | low-power | **Bloqueo: no está en Personalizado** |
| Opciones | low-power balanced performance max-power custom | No cambia el perfil |
| RPM | acpi_fan/fan1_input accesible, módulo fan | Sin lectura numérica |
| RPM yogafan | fan1_input accesible, módulo yogafan original | Incompatible; no se valida como quirk |
| Temperatura | acpitz/temp1_input accesible | Componente sin identificar |
| Temperatura | k10temp/temp1_input, etiqueta Tctl | Lectura/umbral no validados aquí |
| NVMe | Entradas térmicas disponibles | No se usan como temperatura CPU/GPU |
| WMI fan GUID | Dispositivo presente, sin enlace driver | No se vincula ni carga módulo |

Los números hwmon observados son efímeros y no se incorporan a reglas.
El reporte `--recorded-evidence` reproduce esta captura fechada sin consultar
el host. No certifica su estado en una ejecución posterior. El modo existente
`--simulate` permanece aislado, con datos sintéticos. Ambos conservan
execution_enabled=false y la misma generación de payload.

```bash
cargo run --locked -p legion-cli --features experimental-fan-control-preflight \
  --bin legion-cli-fan-control-preflight -- --recorded-evidence --format json
```

## FanFullSpeed y capacidades desde Linux

Se enumeraron únicamente los nombres de atributos de lenovo-wmi-other:
cpu_temp, gpu_temp, ppt_cpu_cl, ppt_pl1_spl, ppt_pl2_sppt, ppt_pl3_fppt,
gpu_nv_ac_offset, gpu_nv_cpu_boost, gpu_nv_ctgp y gpu_nv_ppab. No hay un
atributo FanFullSpeed ni SmartFan/LegionZone visible en esa interfaz. Los
límites térmicos no son sensores. No se consultaron sus current_value.

La ruta candidata ya investigada es GetFeatureValue para FanFullSpeed
(capacidad 0x04020000) en LENOVO_OTHER_METHOD; las capacidades SmartFan y
LegionZone usan sus consultas Lenovo documentadas. Son llamadas activas,
no lecturas pasivas. La presencia de sysfs WMI no ofrece una API de métodos
arbitrarios desde Rust. El módulo actual solo prepara método 5 y el 6 bloqueado;
no dispone de una consulta validada de OtherMethod. No se reutiliza el GUID
de ventiladores como si expusiera estas otras capacidades.

Para consultar FullSpeed en Nobara se requiere preparar/revisar el contrato
de **lectura** correspondiente y autorizar por separado su invocación exacta.
El dato Windows histórico no puede satisfacer el estado actual Linux. V2 ya
está respaldado en este firmware; no es necesario repetir en Windows la
investigación de selección de controlador.

## Propuesta concreta para llegar a la primera aplicación

1. Acreditar una consulta Linux actual de FullSpeed y una supervisión de
   temperaturas/RPM de ambos canales. El yogafan original no sirve como
   prueba de ambos ventiladores. No se sustituye automáticamente.
2. Con autorización independiente, el propietario selecciona Personalizado;
   confirmar custom mediante platform_profile y documentar la correspondencia
   GZ44=3 conocida. Verificar AC y FullSpeed=false, sin cambiarlos por script.
3. Completar PMID/PGID/DCID o evidencia equivalente que justifique el catálogo
   efectivo, y acordar umbrales térmicos de aborto e interrupción con sensores
   identificados. Definir cómo se descarta un cambio concurrente de perfil.
4. Revisar el cambio de activación/endpoint futuro y autorizar una sola llamada
   método 6, instancia 0, payload [1..10] de 64 bytes. No eliminar la barrera
   hasta esa autorización. No aplicar otras curvas ni hacer reintentos.
5. Registrar respuesta, perfil, RPM y temperaturas antes/después. Integer(0)
   no demuestra aplicación; GFAN puede inicializar con INIF si F10A=0 y su
   readback solo cubre una parte del estado. El preset puede no generar una
   diferencia visible. La evaluación requiere el conjunto de evidencias.

Restauración propuesta: Windows/LLT disponible, copia del preset inicial y
ajustes de potencia revisados; en Personalizado seleccionar Predefinido,
FullSpeed desactivado y Guardar mediante la UI, con aprobación específica
para esa acción. La prueba Windows ya acredita ese flujo normal; **no**
acredita recuperación de una aplicación parcial. No prometer rollback con
Fn+Q, reboot o descarga del módulo. El protocolo de contingencia ante una
anomalía térmica debe acordarse antes de autorizar la escritura.

El avance desbloquea la referencia V2 y valida la reaplicación funcional en
Windows. Persisten FullSpeed Linux, modo actual incorrecto, supervisión térmica,
dos canales válidos y recuperación parcial; no se necesita otra auditoría
general para identificar estos siguientes trabajos concretos.
