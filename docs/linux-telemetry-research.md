# Fase 5B.1: investigación de telemetría Linux de solo lectura

## Alcance y conclusión

Este informe diseña adaptadores futuros para la Lenovo Legion Slim 5 16ARP9
(`83EX`), BIOS `NUCN15WW`, en Nobara. Es investigación estática: no se leyó
ninguna ruta del host, no se invocó WMI/ACPI/EC, no se cargaron módulos y no se
implementó un adaptador.

La prioridad debe ser reutilizar las interfaces estándar del kernel, pero una
ruta `sysfs` visible no prueba que sea estable, pasiva ni que describa el
componente esperado. Un futuro adaptador solo podrá abrir una ruta después de
una aprobación de fase y de validar esa ruta, controlador, modelo, BIOS y
kernel concretos.

## Clases de evidencia

| Clase | Evidencia |
| --- | --- |
| Observación previa del 83EX | `platform_profile` y sus perfiles; firmware attributes de `lenovo_wmi_other`; `acpi_fan` y `yogafan` con `0 RPM`; mensaje de que `lenovo_wmi_other` no soporta reporte/ajuste de ventiladores. |
| Interfaz documentada | ABI genérica de `hwmon`, `thermal`, `power_supply`, `platform_profile` y documentación de `lenovo_wmi_other`. |
| Hipótesis pendiente | Qué entradas existen hoy en el kernel Nobara concreto, su controlador, etiqueta, permiso, frecuencia de actualización y significado físico para este 83EX. |

Ninguna ruta enumerada abajo se considera una lectura ya validada para el
equipo. Las rutas que contengan `N` o `<nombre>` son ejemplos de estructura,
no instrucciones de acceso ni promesas de existencia.

## Interfaces candidatas

| Interfaz | Ejemplos de estructura | Valor que podría aportar | Estado para 83EX / límite |
| --- | --- | --- | --- |
| `platform_profile` | `/sys/firmware/acpi/platform_profile`, `platform_profile_choices` | Perfil activo y opciones anunciadas. | Observada. Es una selección de política, no telemetría de rendimiento, potencia ni temperatura. |
| `hwmon` | `/sys/class/hwmon/hwmonN/{name,tempN_input,tempN_label,fanN_input,fanN_label,powerN_input}` | Sensores estandarizados, cuando el driver los publique. | Candidato. `hwmonN` es dinámico y el número de canal no identifica un componente. |
| `thermal` | `/sys/class/thermal/thermal_zoneN/{type,temp,trip_point_N_temp}` | Temperatura actual y límites de zona térmica. | Candidato. Puede duplicar una lectura expuesta también por hwmon; una zona no se debe asignar a CPU/GPU sin evidencia. |
| `power_supply` | `/sys/class/power_supply/<nombre>/{type,online,status,capacity,voltage_now,current_now,power_now,temp}` | Estado de CA/batería y, si el driver lo expone, telemetría de batería/cargador. | Candidato secundario; no representa por sí mismo consumo CPU/GPU ni los límites PPT. |
| Firmware attributes de Other Mode | `/sys/class/firmware-attributes/lenovo-wmi-other[-N]/attributes/<atributo>/{current_value,default_value,min_value,max_value,scalar_increment,type}` | Límites de política configurados por perfil. | Observado. No son sensores instantáneos. Dado que el driver se basa en WMI, hace falta revisar su comportamiento de lectura antes de tratarlo como pasivo. |
| HWMON de Other Mode | Un `hwmonN` asociado al driver, con `fanX_input`, `fanX_div`, `fanX_target` cuando el firmware los anuncie. | RPM actual y datos de canal, si se exponen. | No validado y no disponible según el mensaje observado. No forzar la exposición ni leer `fanX_target` como RPM actual. |

La documentación de `platform_profile` especifica que la API sirve para elegir
un perfil, no para conocer el rendimiento resultante. Por tanto, un adaptador
solo puede producir `EnergyProfiles`; no puede derivar de `performance` una
frecuencia, potencia o velocidad de ventilador.

## Semántica y unidades

| Fuente / atributo | Significado documentado | Unidad de origen | Conversión segura al modelo |
| --- | --- | --- | --- |
| `hwmon/tempN_input` | Temperatura de entrada medida. | Normalmente miligrados Celsius. Algunos drivers de termistor pueden usar milivoltios; revisar documentación del driver. | Solo `m°C / 1000 → °C` cuando el driver confirme temperatura. En caso contrario, rechazar como temperatura. |
| `thermal_zoneN/temp` | Temperatura actual de zona. | miligrados Celsius. | `m°C / 1000 → °C`, clasificación `InstantaneousMeasurement`. |
| `thermal_zoneN/trip_point_N_temp` | Umbral de zona, no lectura actual. | miligrados Celsius. | `m°C / 1000 → °C`, clasificación `ConfiguredLimit`, nunca sensor. |
| `hwmon/fanN_input` | RPM reportado. | RPM. | Sin escala; conservar valor y validación de fuente. |
| `hwmon/powerN_input` | Potencia instantánea. | microvatios. | `µW / 1_000_000 → W`, `PowerReadingKind::InstantaneousConsumption`. |
| `hwmon/powerN_cap` | Límite de potencia. | microvatios. | `µW / 1_000_000 → W`, `PowerReadingKind::ConfiguredLimit`. |
| `power_supply/voltage_now`, `current_now`, `power_now` | Telemetría del objeto batería/cargador, si existe. | µV, µA y µW. | Convertir individualmente si se incorporan esos modelos. No calcular potencia de CPU/GPU ni multiplicar voltaje/corriente sin contrato de sincronización y signo del driver. |
| `power_supply/temp` | Temperatura de batería/cargador, si existe. | décimas de °C. | `0.1 °C / 10 → °C`; no mezclar con temperatura de CPU/GPU. |
| `lenovo_wmi_other/*/current_value` | Valor vigente de un atributo de firmware. | Definida por el atributo y su `type`; no hay unidad genérica segura. | Conservar como límite o valor nominal solo después de una tabla de unidades validada por atributo. Nunca convertirlo en consumo instantáneo. |

La norma de `hwmon` define `powerN_input` como potencia instantánea y
`powerN_cap` como límite. También advierte que los sensores de una placa no
tienen una correspondencia estándar entre su ordinal y CPU, GPU o ventilador.

Frecuencias quedan deshabilitadas inicialmente: `freqN_input` no es una fuente
universal de CPU/GPU con semántica y unidad portable. Solo se añadirá tras
documentar el driver concreto y su unidad; no se inferirá desde un nombre de
archivo ni desde una frecuencia nominal.

## Atributos ya observados de `lenovo_wmi_other`

El inventario previo del 83EX contiene `cpu_temp`, `gpu_temp`, `ppt_cpu_cl`,
`ppt_pl1_spl`, `ppt_pl2_sppt`, `ppt_pl3_fppt`, `gpu_nv_ac_offset`,
`gpu_nv_cpu_boost`, `gpu_nv_ctgp` y `gpu_nv_ppab`. La documentación pública de
Other Mode indica que sus atributos tienen `current_value`, `default_value`,
`min_value`, `max_value`, `scalar_increment` y `type`, con páginas por perfil.

Para este proyecto su clasificación inicial es estricta:

- `cpu_temp` y `gpu_temp`: límites térmicos configurables, **no** temperaturas
  instantáneas.
- Los valores PPT y GPU NV: límites o parámetros de política, **no** consumo
  eléctrico instantáneo, frecuencia ni potencia medida.
- `current_value`: el valor configurado vigente de cada atributo; su nombre no
  cambia su clasificación.

No se asumirá la unidad de un atributo Lenovo sin confirmar su `type`, la
documentación del driver para el kernel objetivo y una evidencia por atributo.
Si un acceso a esos atributos ejecuta una consulta WMI, deja de ser una lectura
pasiva según `AGENTS.md` y requerirá autorización explícita adicional.

## Identidad de sensores y ventiladores

Un adaptador debe descubrir primero el dispositivo, no escoger `hwmon0`,
`temp1` o `fan1` por posición. La documentación de hwmon recalca que no existe
un estándar que conecte el segundo canal de temperatura con la CPU ni el
segundo ventilador con un componente concreto.

La política propuesta es:

1. Resolver la ruta de clase dinámicamente y capturar el `name` del driver,
   la etiqueta global y el enlace de dispositivo cuando existan.
2. Exigir `tempN_label` o `fanN_label` y una regla de compatibilidad específica
   antes de asignar el canal a CPU, GPU o ventilador físico.
3. Si falta etiqueta o la regla no coincide, publicar solo un sensor genérico
   con estado `Unvalidated`, o suprimirlo de la vista de componentes.
4. Conservar el nombre de driver, etiqueta, ruta de clase, kernel, modelo y
   BIOS como procedencia; los números `N` no son identidad persistente.
5. Evitar duplicados entre thermal y hwmon: si se demuestra que describen la
   misma fuente, elegir una fuente canónica y registrar la relación; si no, no
   agregarlos ni promediarlos.

Una lectura `fanN_input = 0` solo puede exponerse como RPM reportado. Mientras
el canal y su telemetría no estén validados para el 83EX, debe tener
`FanValidation::Unvalidated` y `FanInterpretation::NonConclusive`. No existe
un estado `Stopped` en el modelo y no se debe introducir por inferencia. Un
atributo `fanN_fault=1`, si existe, tampoco prueba que el ventilador esté
detenido: invalida la confianza de la medición.

## Estados, errores y procedencia

| Situación del adaptador | `TelemetryState` | Valor | Diagnóstico/procedencia |
| --- | --- | --- | --- |
| Archivo permitido, entero válido, unidad y mapeo validados | `Available` | `Some(valor convertido)` | Driver, atributo, unidad original, conversión, kernel y nivel de confianza. |
| Archivo opcional no existe | `Unavailable` | `None` | Motivo `not-present`; no usar cero como sustituto. |
| Canal existe pero no tiene etiqueta/regla de identidad suficiente | `Unvalidated` | `Some(valor)` o `None` | Motivo `unmapped-channel`; conservar origen sin atribuirlo a CPU/GPU. |
| Permiso denegado, E/S, entero malformado, unidad desconocida o `fault` | `ReadError` | `None` | Motivo de error estructurado; nunca degradar a cero o a límite. |

El modelo actual conserva estado y procedencia. Antes de fuentes reales debe
añadirse una envoltura de diagnóstico de adaptador (ruta lógica, atributo,
error de sistema y conversión) que no se serialice como una medición válida.
La `TelemetrySnapshot::validate()` debe ejecutarse tras la conversión y antes
de que CLI o GUI serialicen la instantánea.

## Permisos y riesgos de lectura

Los atributos estándar de hwmon se documentan normalmente como legibles por
usuarios sin privilegios; las escrituras estándar, cuando existan, requieren
privilegios. Aun así, el adaptador no debe solicitar `sudo`, cambiar permisos,
abrir atributos RW/WO para escribir ni usar interfaces de control como
`pwmN`, `fanN_target`, `*_enable`, `emul_temp`, `policy` o perfiles.

`power_supply`, thermal y hwmon pueden omitir atributos opcionales. Un fallo de
permisos o de lectura es un resultado normal del adaptador, no un motivo para
usar WMI, EC, `/proc`, herramientas externas o una ruta de respaldo. Leer una
ruta sysfs puede provocar trabajo del controlador; para una fuente Lenovo
basada en WMI se debe demostrar por código que no ejecuta un método activo, o
solicitar una autorización específica antes de usarla.

## Arquitectura propuesta de adaptadores

```text
raíz de clase inyectada
        │
        ▼
descubridor pasivo ──► manifiesto de fuentes permitidas
        │                         │
        ▼                         ▼
adaptador solo lectura ──► parser y conversor ──► validador de snapshot
                                                          │
                                                          ▼
                                           TelemetrySnapshot para CLI/GUI
```

- La raíz de clase será una dependencia inyectada: fixtures en pruebas y una
  raíz explícita aprobada en producción. No habrá rutas suministradas por CLI,
  variables de entorno ni fallback al host.
- El descubridor producirá descriptores, no mediciones. Cada descriptor deberá
  pasar una lista blanca de clase, driver y atributo de solo lectura.
- Cada adaptador será pequeño: `PlatformProfileReader`, `ThermalReader`,
  `HwmonReader`, `PowerSupplyReader` y, separadamente, un lector descriptivo
  de firmware attributes. Ninguno contendrá operaciones de escritura.
- Las conversiones serán funciones puras, cubiertas con fixtures que incluyan
  ausencia, permisos, texto malformado, unidad errónea, etiquetas ambiguas,
  cero RPM y duplicados thermal/hwmon.
- Un adaptador no habilita controles, no cambia la compatibilidad estática y
  no puede activar WMI/ACPI/EC como plan alternativo.

## Orden recomendado para Fase 5B.2

1. **Infraestructura de pruebas y descubrimiento simulado.** Raíz inyectada,
   lista blanca, descriptores, conversión y diagnósticos; sin acceso host.
2. **Lector de `platform_profile` estrictamente de lectura.** Solo después de
   revisar la ruta concreta y obtener la autorización de hardware aplicable.
   Publica perfil y opciones, no métricas derivadas.
3. **Lector thermal genérico.** Empieza con zonas etiquetadas o descritas por
   una regla de compatibilidad; muestra otras como no validadas.
4. **Lector hwmon de temperatura y potencia.** Requiere nombre de driver,
   etiqueta, unidad y mapeo documentados. `powerN_input` y `powerN_cap` se
   mantienen como clases distintas.
5. **`power_supply` de batería/CA.** Útil para contexto de alimentación, pero
   separado de CPU/GPU/PPT.
6. **Firmware attributes Lenovo de solo lectura.** Solo tras revisión del
   comportamiento WMI de la versión Nobara y autorización específica; expone
   límites, no telemetría instantánea.

Permanecen deshabilitados: RPM Lenovo no validadas, `fanN_target`, PWM y curvas,
frecuencias CPU/GPU sin fuente identificada, lecturas WMI directas, EC, ACPI,
escrituras y cualquier cambio de perfil. Ningún paso de esta lista autoriza
una operación real por sí mismo.

## Fuentes públicas consultadas

- [Interfaz sysfs de hwmon](https://docs.kernel.org/hwmon/sysfs-interface.html)
- [Interfaz sysfs térmica](https://docs.kernel.org/driver-api/thermal/sysfs-api.html)
- [Clase Linux power_supply](https://docs.kernel.org/power/power_supply_class.html)
- [Platform Profile Selection](https://docs.kernel.org/userspace-api/sysfs-platform_profile.html)
- [Lenovo WMI Other Mode](https://docs.kernel.org/wmi/devices/lenovo-wmi-other.html)
- [Lenovo WMI Gamezone](https://docs.kernel.org/wmi/devices/lenovo-wmi-gamezone.html)
- [ABI de firmware attributes](https://github.com/torvalds/linux/blob/master/Documentation/ABI/testing/sysfs-class-firmware-attributes)

La documentación pública describe ABI y comportamiento genérico. No se ha
inspeccionado la fuente empaquetada por Nobara `7.2.0-202.nobara.fc44.x86_64`,
no se ha confirmado la presencia de estos nodos en el equipo actual y no se ha
copiado código externo.
