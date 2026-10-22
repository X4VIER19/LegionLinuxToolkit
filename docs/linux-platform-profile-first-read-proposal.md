# Propuesta previa a una única lectura real de `platform_profile` (Fase 5B.5B-3B)

## Alcance

La propuesta original fue estática y partió de
`e6455a773823ae6cc39021804517d3c524ae4aa7`. Su mecanismo se ha preparado en
el árbol de trabajo, pero permanece desactivado por defecto: esta fase no
compila el binario de activación ni abre rutas del host.

> **Estado de implementación posterior:** la fase de preparación puede añadir
> exactamente la feature, la fachada y el binario descritos aquí, pero deben
> permanecer desactivados por defecto y no se compilarán ni ejecutarán durante
> esa fase. Su mera presencia no autoriza una lectura real.

> **Hito posterior autorizado:** el 2026-10-21 se ejecutó una vez el binario
> experimental con las dos rutas fijas y sin `sudo`. Terminó con código `0`.
> Este documento conserva el diseño previo; el resultado y sus límites se
> registran a continuación y no convierten el mecanismo en una función
> permanente.

La única operación futura considerada sería leer una vez, sin escritura:

- `/sys/firmware/acpi/platform_profile`;
- `/sys/firmware/acpi/platform_profile_choices`.

No cubre WMI, ACPI, EC, ventiladores, potencia, firmware, módulos, perfiles de escritura, privilegios ni rutas alternativas.

| Afirmación | Clase | Límite |
| --- | --- | --- |
| El 83EX devolvió antes `performance` y `low-power balanced performance max-power custom` en esas rutas. | Observación manual histórica. | No prueba permisos, topología, tipo, EOF, pasividad ni atomicidad actuales. |
| `openat2` y `RESOLVE_*` restringen la resolución. | Propiedad documentada en los diseños previos. | No garantiza que sysfs acepte la política. |
| El proveedor conserva nombres internos, límite y parser. | Hecho del código actual. | Solo se compila con una feature experimental no predeterminada. |
| Una lectura real debe ser opt-in, única y separada de `legion-cli`. | Decisión propuesta. | Requiere autorización futura independiente. |

## Barrera y activación experimental preparada

Sin la feature, el proveedor, su capacidad de activación y las funciones que
contienen la ruta `/sys` ni siquiera se compilan. Con la feature,
`ApprovedRealReadActivation::ApprovedOneShot` sigue siendo privado y solo la
fachada estrecha puede construirlo. El proveedor, sus descriptores y
`ConfinedOpenat2PlatformProfileSource` son privados. El binario aislado actual
sigue usando únicamente escenarios embebidos y `StaticSimulation`.

Un binario Rust es un crate distinto y solo puede usar API pública de su librería. Por ello, un binario futuro no puede llamar al proveedor privado sin una fachada pública muy estrecha. La propuesta es compilar esa fachada solo con una feature no predeterminada. La feature no es autorización: su diff y su ejecución requerirían revisión y aprobación nuevas.

`ApprovedOneShot` no sería una garantía automática de «una sola vez»: solo representaría una capacidad de código disponible bajo la feature. La futura fachada debe consumir además un guardado privado de proceso, por ejemplo un `AtomicBool` inicializado en `false` y cambiado con `compare_exchange(false, true, ...)` antes de abrir `/sys`. Una segunda llamada en el mismo proceso debe devolver `AlreadyAttempted` sin abrir nada. Esto limita una ejecución a una consulta explícita; no es un contador global entre procesos ni autoriza ejecuciones posteriores.

La feature y el binario deben tener nombres explícitos y no predeterminados.
La compilación normal, las pruebas normales y `legion-cli` no deben depender de
ellos. La fachada será el único símbolo público condicionado por la feature;
el proveedor concreto y los descriptores permanecerán privados.

| Archivo | Cambio preparado, aún desactivado |
| --- | --- |
| `crates/legion-linux/Cargo.toml` | Feature `approved-real-platform-profile-read` no predeterminada y binario que la requiera. |
| `crates/legion-linux/src/lib.rs` | Cambiar deliberadamente la barrera en un módulo privado, añadir fachada condicionada y error de inicialización estructurado. |
| `crates/legion-linux/src/bin/legion-linux-platform-profile-real-read.rs` | Binario de un solo propósito, sin rutas ni configuración, con confirmación literal. |
| `docs/linux-platform-profile-production-design.md` | Protocolo de activación y resultado aprobado. |
| `AGENTS.md` | Puerta explícita de lectura real única, recomendada al final de este informe. |

No se modificarían `legion-core`, `legion-cli`, `legion-fixtures`, módulos del kernel ni dependencias externas.

## Diff de activación implementado, no habilitado

Este extracto resume la activación presente en el árbol de trabajo. La feature
no es predeterminada y no se ha compilado ni ejecutado con la feature en esta
fase.

```diff
diff --git a/crates/legion-linux/Cargo.toml b/crates/legion-linux/Cargo.toml
@@
+[features]
+approved-real-platform-profile-read = []
+
+[[bin]]
+name = "legion-linux-platform-profile-real-read"
+path = "src/bin/legion-linux-platform-profile-real-read.rs"
+required-features = ["approved-real-platform-profile-read"]

diff --git a/crates/legion-linux/src/lib.rs b/crates/legion-linux/src/lib.rs
@@
-enum ApprovedRealReadActivation {}
+enum ApprovedRealReadActivation {
+    #[cfg(feature = "approved-real-platform-profile-read")]
+    ApprovedOneShot,
+}
+
+#[cfg(feature = "approved-real-platform-profile-read")]
+#[doc(hidden)]
+pub fn approved_one_shot_platform_profile_read(
+    confirmation: &str,
+) -> Result<LinuxPlatformProfileReading, ExperimentalRealReadError> {
+    require_literal(confirmation, "I-UNDERSTAND-READ-ONLY-SYSFS")?;
+    ONE_SHOT_CONSUMED.compare_exchange(false, true, ...)
+        .map_err(|_| ExperimentalRealReadError::AlreadyAttempted)?;
+    SysfsPlatformProfileProvider::new(ApprovedRealReadActivation::ApprovedOneShot)?.read()
+}

diff --git a/crates/legion-linux/src/bin/legion-linux-platform-profile-real-read.rs b/crates/legion-linux/src/bin/legion-linux-platform-profile-real-read.rs
+// Accept only --confirm I-UNDERSTAND-READ-ONLY-SYSFS and --format text|json.
+// Do not accept root, path, scenario, environment or fallback options.
+// Call the one-shot facade once, serialize an explicit result or error, then exit.
```

`ONE_SHOT_CONSUMED` sería un `AtomicBool` privado y condicionado por la misma feature. La fachada debe preservar la privacidad del proveedor. Las fallas al abrir `/sys`, `firmware` o `acpi` no equivalen a `Missing { entry }`; hace falta un error como `ProviderInitialization { stage, errno }` que no publique una lectura.

## Punto de activación, rutas y confinamiento

El punto exacto que volvería invocable al proveedor sería la construcción de `ApprovedRealReadActivation::ApprovedOneShot` dentro de la fachada condicionada por feature. Debe aparecer en un diff fácil de localizar y revisarse antes de compilarlo.

La futura secuencia de apertura sería:

1. Abrir `/sys` literal con `openat2(CWD, "/sys", ...)`, `O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW` y `RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS`. No usar `RESOLVE_BENEATH`: una ruta absoluta no lo admite.
2. Confirmar con `fstat` que es directorio; abrir solo `firmware` y `acpi` relativos al descriptor anterior, con `O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW` y `RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS`.
3. Abrir solo `platform_profile_choices`, `platform_profile` y nuevamente `platform_profile_choices`, relativos a `acpi`, con `O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK` y las tres restricciones de resolución.
4. Exigir archivo regular según la política actual, leer hasta 4096 bytes más un byte de detección, continuar lecturas cortas y completar solo en EOF.
5. Comparar las listas, validar con `PlatformProfileAdapter`, publicar `LinuxReadOnly` solo tras éxito completo y cerrar por RAII.

`O_NOFOLLOW` cubre el componente final; `RESOLVE_*` cubre la resolución relativa. Un `ELOOP` debe cancelar por enlace prohibido. La topología real no se ha inspeccionado: un enlace legítimo de sysfs sigue siendo una incógnita y debe fallar de modo cerrado, no activar un fallback.

## Comando futuro propuesto, no autorizado ni ejecutado

Tras un diff ejecutable revisado y autorización explícita para feature, binario, token, banderas, modelo, BIOS y kernel, el comando de prueba única sería:

```bash
/home/x4vier19/.cargo/bin/cargo run --locked -p legion-linux \
  --features approved-real-platform-profile-read \
  --bin legion-linux-platform-profile-real-read -- \
  --confirm I-UNDERSTAND-READ-ONLY-SYSFS --format json
```

No admitiría `--root`, `--path`, `--sysfs`, `--real`, perfiles, WMI ni otros atributos. Abriría exclusivamente las dos rutas objetivo mediante la secuencia anterior. No requiere `sudo`: `EACCES` o `EPERM` deben ser errores explícitos, sin elevación, cambio de permisos ni reintentos.

El binario futuro debe analizar sus argumentos una vez, exigir el token literal, llamar a la fachada una vez y terminar. No tendrá modo de monitoreo, temporizador, servicio, activación al iniciar sesión o al iniciar el sistema, bucle de lectura, señal para repetir ni reintento automático. Una nueva ejecución del proceso sería otra tentativa que requeriría autorización humana nueva; el guardado `AtomicBool` solo limita las llamadas dentro de una ejecución concreta.

## Resultados, errores y coherencia

| Condición | Resultado permitido |
| --- | --- |
| Tres lecturas completas, listas iguales y parser correcto. | Una observación de confianza inicial baja con origen y artefacto `LinuxReadOnly`. |
| `ENOSYS`, `ENOENT`, `EACCES`, `EPERM`, `EINVAL`, `E2BIG`, `ELOOP`, `EXDEV`, `EAGAIN`, `ENOTDIR` o tipo inesperado. | Error de inicialización/origen con etapa y `errno`, sin lectura ni fallback. |
| UTF-8 inválido, vacío, control inesperado, duplicado o perfil activo ausente. | Error de interpretación, sin normalización ni lectura publicada. |
| Interrupción más allá del límite, error tras bytes o más de 4096 bytes. | Error parcial/tamaño y descarte del búfer. |
| `choices_before != choices_after`. | `ConcurrentChange`, sin resultado ni reintento silencioso. |

`choices → active → choices` no es atómico ni detecta A→B→A o cambios después de la segunda lista. La lectura no valida control de energía. Los errores reales no usan `LinuxReadOnly`; solo un resultado completo puede usar esa procedencia. Fixtures y errores aislados permanecen en `StaticSimulation`.

## Resultado de la primera ejecución real autorizada

La única invocación autorizada usó el binario
`legion-linux-platform-profile-real-read` con la feature
`approved-real-platform-profile-read`, el literal
`I-UNDERSTAND-READ-ONLY-SYSFS` y formato JSON. No usó `sudo`, no ejecutó
pruebas, servicios ni otros binarios, y no produjo operaciones de escritura.

El proceso devolvió código `0`. Las tres lecturas lógicas fueron completas y
coherentes: el perfil activo fue `performance`; ambas lecturas de opciones
fueron `low-power balanced performance max-power custom`. El parser publicó
`state: available`, `artifact: linux-read-only`, origen `LinuxReadOnly`,
fuente `legion-linux::readonly:sysfs-platform-profile` y confianza `low`.

Esto valida solo esa consulta concreta en el equipo, BIOS y entorno
documentados. No valida que sysfs sea una instantánea atómica, que el acceso
sea pasivo para toda condición del controlador, ni futuras ejecuciones. No
valida controles, escritura, cambio de perfil, ventiladores, potencia,
firmware, WMI, ACPI o EC. La feature sigue desactivada por defecto, el
proveedor continúa separado de `legion-cli` y una nueva lectura exige una
autorización explícita independiente.

## Riesgos, cancelación y reversión

- Abrir sysfs no demuestra pasividad del controlador. Las banderas restringen rutas, no efectos.
- `O_NONBLOCK` puede devolver `EAGAIN`; no es EOF. `fstat`, tamaño y EOF pueden diferir en atributos virtuales.
- `RESOLVE_NO_SYMLINKS` puede rechazar una topología válida; no se relaja automáticamente.

**Éxito mínimo:** tres lecturas antes del límite, listas iguales, parser correcto, una salida `LinuxReadOnly` y ninguna acción adicional.

**Error seguro:** código distinto de cero, etapa y error identificables, sin dato parcial, fallback ni segunda consulta.

**Cancelar antes:** si cambia modelo/BIOS/kernel; si el diff añade otra ruta, privilegio, feature predeterminada, variable de entorno, CLI general, fallback o escritura; o si falla una prueba aislada. Durante la única invocación no hay bucles ni reintentos; ante bloqueo o resultado inesperado, `Ctrl-C` y ningún comando adicional.

No se necesita reversión de hardware: no hay escrituras, perfiles, módulos ni firmware. Los descriptores se cierran por RAII al salir. La reversión de software debe ser explícita: eliminar el `[[bin]]`, la feature, la fachada y `ONE_SHOT_CONSUMED`, y restaurar `enum ApprovedRealReadActivation {}` sin variantes. Después se verifica que el binario real no se compila sin feature. Esta reversión requiere un commit revisado; no se sustituye por limpieza destructiva.

## Pruebas aisladas obligatorias

1. `cargo test --workspace`, `cargo check --workspace --all-targets`, `cargo fmt --all -- --check` y `git diff --check` correctos.
2. Compilación con y sin feature: el binario real no existe sin feature y `legion-cli` no depende de `legion-linux`.
3. Fixtures para `ENOSYS`, `ENOENT`, `EACCES`, `EPERM`, `ELOOP`, `EXDEV`, `EAGAIN`, `EINVAL`, `E2BIG`, `EINTR`, directorio o tipo no regular; sin fallback ni resultado válido.
4. Fixtures para EOF vacío, 4096 bytes, byte 4097, UTF-8 inválido, lectura parcial, fallo tras bytes y opciones iguales o distintas.
5. Prueba de que fixture o error no puede serializarse como `LinuxReadOnly` y de que el token no abre otra ruta.

## AGENTS.md: cambio recomendado, no aplicado

Las reglas ya prohíben escrituras y exigen autorización para acciones activas. Para esta lectura de sysfs, que no invoca un método WMI/ACPI/EC explícito pero mantiene riesgo de controlador, convendría una puerta de **lectura real única de sysfs**: rutas y banderas exactas, modelo/BIOS/kernel registrados, ausencia de `sudo`, revisión del diff, una sola invocación, captura de resultado y prohibición de reintentos automáticos. `AGENTS.md` no cambia en esta fase.
