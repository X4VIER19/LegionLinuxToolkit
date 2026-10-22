# Evaluación de estabilidad y compatibilidad de `platform_profile` (Fase 5C.4)

## Resumen ejecutivo

La lectura de `platform_profile` está **preparada para seguir como capacidad
experimental controlada**, pero no reúne aún evidencia suficiente para
declararse estable ni para habilitarla desde `legion-cli` como comportamiento
normal.

La única evidencia de hardware real es una ejecución explícitamente autorizada
el 2026-10-21 en la Lenovo Legion Slim 5 16ARP9 (`83EX`), BIOS `NUCN15WW`,
Nobara con kernel `7.2.0-202.nobara.fc44.x86_64`. El proveedor experimental
leyó `platform_profile_choices`, `platform_profile` y de nuevo
`platform_profile_choices`; obtuvo opciones iguales y el perfil
`performance`. El resultado fue `LinuxReadOnly` con confianza baja.

Las pruebas posteriores, incluida la integración de `legion-cli`, son
exclusivamente simuladas. Comprueban la arquitectura y los errores, pero no
amplían la evidencia de que sysfs sea legible, pasivo o compatible en otra
ejecución, BIOS, kernel, distribución o equipo.

Esta evaluación es estática y documental. No ejecutó el proveedor, no leyó
sysfs y no modifica la autorización de hardware.

## Inventario de capacidades verificadas

| Capacidad | Evidencia | Estado | Límite |
| --- | --- | --- | --- |
| Interpretar y normalizar texto de perfiles. | Pruebas de `PlatformProfileAdapter` con entradas válidas, ausentes, vacías, duplicadas, desconocidas, incoherentes y malformadas. | Verificado en simulación. | No demuestra que una fuente Linux entregue esos textos. |
| Una consulta tipada y explícita. | `PlatformProfileQuery::read_once()` y `LinuxPlatformProfileReader`. | Verificado en simulación. | No es una garantía de pasividad de una fuente real. |
| Separar simulación de lectura Linux. | Orígenes sellados; `LinuxReadOnlyOrigin` no tiene constructor público y el árbol simulado publica solo `StaticSimulation`. | Verificado estáticamente y con pruebas aisladas. | La garantía depende de conservar la privacidad y revisar cambios futuros. |
| Rechazar entradas de ruta desde la CLI. | Escenarios cerrados; la CLI solo convierte fixtures embebidas a un árbol en memoria. | Verificado estáticamente y con pruebas. | No sustituye una política de autorización para una futura fuente real. |
| Confinamiento y fallos cerrados de `openat2`. | Pruebas sobre directorios temporales para enlaces, escapes, sustitución, permisos, EOF, UTF-8 y tamaño. | Verificado en árboles de prueba. | sysfs no es un archivo regular convencional; no valida todas sus variantes. |
| Lectura real de los dos atributos fijos. | Una ejecución autorizada registrada en [Observaciones Linux](linux-observations.md). | Validado una vez, confianza baja. | No habilita repetición, monitorización ni integración permanente. |

## Arquitectura evaluada

```text
legion-cli (solo escenarios embebidos)
        |
        v
LinuxPlatformProfileReader<SimulatedPlatformProfileTree>
        |
        v
PlatformProfileQuery::read_once()
        |
        v
PlatformProfileAdapter en legion-core

Proveedor sysfs privado, solo feature experimental
        |
        v
openat2 confinado -> choices → active → choices
```

`legion-core` no conoce Linux, rutas ni descriptores. `legion-cli` depende de
`legion-linux` para el lector genérico, pero no importa ni puede nombrar el
proveedor sysfs privado. La única feature que compila el punto de activación
real es `approved-real-platform-profile-read`; no es predeterminada y
`legion-cli` no la solicita.

La unificación de features de Cargo no ejecuta código durante la compilación:
solo puede incluir código condicionado si se selecciona expresamente la
feature para `legion-linux` o un workspace que la incluya. Incluso entonces,
la lectura requiere invocar el binario experimental, proporcionar el literal
de confirmación y superar el guardia `AtomicBool` por proceso. Esto no vuelve
la feature apta para uso normal ni sustituye autorización humana previa.

## Matriz de compatibilidad

| Dimensión | Conocido | Desconocido o no validado | Regla propuesta |
| --- | --- | --- | --- |
| Modelo | Lenovo Legion Slim 5 16ARP9, tipo `83EX`. | Cualquier otro modelo o familia Legion. | Requerir coincidencia exacta. |
| DMI | No se documentó identificador DMI utilizable. | Valor DMI y estabilidad entre unidades. | No inventarlo ni tratar ausencia como comodín. |
| BIOS | `NUCN15WW`. | Otras versiones, incluso del mismo modelo. | Registrar y evaluar por versión exacta. |
| Distribución | Nobara. | CachyOS, Linux Mint y cualquier otra. | Mantener no validada hasta reunir evidencia propia. |
| Kernel | `7.2.0-202.nobara.fc44.x86_64`. | Otros kernels, configuración y ABI efectivo de `openat2`. | Registrar versión y resultado; no extrapolar. |
| Interfaz | Los dos atributos devolvieron texto coherente una vez. | Permisos, topología y semántica futura de sysfs. | Limitarse a esos dos nombres fijos. |
| Implementación | API segura `rustix::fs::openat2`, límite de 4096 bytes. | Comportamiento en todos los entornos objetivo. | Fallar cerrado ante incompatibilidad. |

## Modelo de amenazas y límites de seguridad

| Amenaza o fallo | Mitigación actual | Límite residual |
| --- | --- | --- |
| Ruta controlada por usuario, escape o enlace. | Sin parámetros de ruta; nombres constantes; `dirfd`, `O_NOFOLLOW`, `RESOLVE_BENEATH`, `RESOLVE_NO_MAGICLINKS` y `RESOLVE_NO_SYMLINKS`. | La apertura inicial de `/sys` y la semántica de enlaces de sysfs dependen del kernel. |
| Lectura demasiado grande, parcial o inválida. | Límite de 4096 bytes, detección de exceso, EOF explícito, UTF-8 y errores diferenciados. | EOF y `fstat` no prueban la semántica de todo atributo virtual. |
| `openat2` no disponible o no apto. | `ENOSYS` y errores de confinamiento se propagan; no hay `open()`/`openat()` de reserva. | La capacidad queda no disponible; incompatibilidad no queda demostrada. |
| Cambio concurrente de opciones y perfil. | Secuencia `choices → active → choices`; lista distinta produce error sin publicar resultado. | No es atómica y no detecta A→B→A ni cambios posteriores. |
| Simulación etiquetada como real. | Fuente sellada, constructor de origen real privado y comprobación origen/artefacto. | Una regresión de visibilidad futura exige auditoría específica. |
| Activación accidental. | Feature no predeterminada, binario separado, confirmación literal y `AtomicBool`; sin rutas, fallback, monitorización ni reintentos automáticos. | Una compilación o invocación explícita requiere autorización. |

No hay operaciones de escritura, WMI, ACPI, EC, control de ventiladores,
cambio de perfil, servicio ni proceso residente en esta capacidad.

## Matriz de pruebas recomendadas

| Área | Casos simulados requeridos | Resultado esperado |
| --- | --- | --- |
| Formato | Perfil válido, desconocido, ausente, vacío, duplicado, incoherente y caracteres de control. | Valor validado o error de interpretación; nunca normalización silenciosa. |
| Adquisición | Atributo ausente, permisos, UTF-8 inválido, lectura fallida, parcial y exceso de 4096 bytes. | Error de origen diferenciado; sin observación válida. |
| Confinamiento | Enlace simbólico, `..`, sustitución, `ELOOP`, `EXDEV`, `EAGAIN`, `ENOSYS`. | Rechazo cerrado y sin fallback. |
| Coherencia | Opciones iguales, cambiantes y secuencia agotada. | Publicar solo listas iguales; `ConcurrentChange` para el resto. |
| Procedencia | Fixture válida, ausente y error de fuente. | Solo `StaticSimulation`; ningún error produce `LinuxReadOnly`. |
| CLI | Nombres cerrados, texto/JSON, rutas y flags reales rechazados. | Salida o error claramente simulados; sin acceso al host. |
| Feature | Compilación predeterminada y comprobación de feature sin ejecutar binarios ni pruebas de hardware. | La compilación no dispara una lectura; el binario real sigue separado. |

Antes de toda prueba real futura deben pasar las pruebas aisladas aplicables.
La prueba real requerirá además autorización separada, comando fijo, identidad
registrada y revisión del resultado completo.

## Hallazgos y gravedad

| Gravedad | Hallazgo | Efecto | Acción |
| --- | --- | --- | --- |
| Crítica | Ningún defecto crítico identificado en esta revisión estática. | — | Mantener revisión por cambio. |
| Alta — bloqueo de estabilidad | Solo existe una lectura real y su confianza es baja. | No puede declararse soporte estable ni compatibilidad multi-distribución. | Mantener experimental; reunir evidencia autorizada y específica por entorno. |
| Alta — bloqueo de estabilidad | No hay DMI documentado ni matriz de resultados por BIOS/kernel/distribución. | Una coincidencia futura podría ser demasiado amplia. | Registrar solo valores observados; ausencia no coincide. |
| Media | La secuencia de tres lecturas no es una transacción. | Puede no detectar ciertos cambios concurrentes. | Informar el límite y rechazar diferencias observables; no inferir atomicidad. |
| Media | Sysfs puede diferir de archivos regulares en tipo, tamaño, EOF y permisos. | Una política correcta en fixtures puede fallar o rechazar un entorno válido. | Preferir fallo cerrado y documentar resultados, no relajar banderas automáticamente. |
| Baja | La CLI comparte el lector Linux aunque use solo fixtures. | El límite de revisión entre capas exige mantenerse claro. | Conservar API sellada y pruebas que rechazan rutas y procedencias reales. |

## Criterios de aceptación y condiciones de bloqueo

Una integración real controlada solo podrá proponerse si se cumplen todos estos
criterios:

1. La CLI conserva un selector de fuente cerrado y explícito; el modo simulado
   no puede cambiar a Linux por argumento, entorno ni fallback.
2. El proveedor real permanece privado, limitado a los dos atributos y sin
   operaciones de escritura, monitorización, servicios ni reintentos.
3. Las pruebas aisladas de esta matriz pasan con compilación predeterminada;
   una comprobación de feature no ejecuta binarios ni pruebas de hardware.
4. La documentación registra modelo, BIOS, DMI si se conoce, kernel,
   distribución, interfaz, versión de implementación y procedencia.
5. Cada lectura real durante la transición recibe autorización concreta y no
   se interpreta como permiso permanente.
6. Los resultados reales conservan éxito o clase de error, sin etiquetar
   fallos como datos válidos ni como incompatibilidad demostrada.

Bloquean el avance: una ruta configurable, constructor público del proveedor
real, procedencia Linux fabricable desde fixtures, fallback de apertura,
operación de escritura, defecto de confinamiento o inconsistencia que publique
un resultado.

## Recomendación y siguiente fase

**Recomendación: mantener experimental y preparar una integración controlada;
no declarar estable todavía.** La arquitectura y pruebas permiten diseñar una
fase posterior sin duplicar lógica, pero la evidencia de hardware no alcanza
para habilitar lectura Linux desde la CLI de uso normal.

La siguiente fase más segura sería una **Fase 5C.5 documental de política de
integración controlada**: definir selección de fuente cerrada, cambio mínimo
de permisos en `AGENTS.md`, registro de compatibilidad y protocolo de revisión
previa. No debe implementar el selector, modificar `AGENTS.md`, habilitar una
feature ni ejecutar una lectura real. Solo tras aprobar ese diseño podría
autorizarse por separado una implementación y auditoría.
