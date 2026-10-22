# Diseño de integración de producción para `platform_profile` (Fase 5B.5B-1)

## Alcance y límite de esta fase

Este documento diseña una posible evolución del proveedor de pruebas `openat2`
de `legion-linux` para consultar exclusivamente dos atributos Linux:

- `/sys/firmware/acpi/platform_profile`;
- `/sys/firmware/acpi/platform_profile_choices`.

La Fase 5B.5B-2 incorpora una implementación de forma compilable, pero no
activable: el proveedor concreto es privado y su constructor exige una
capacidad privada que el crate no crea. Por tanto, esta fase no abre ninguna
ruta del host, no invoca firmware y no autoriza a la CLI ni a otra capa a leer
sysfs. Toda activación futura requerirá una fase, revisión de código y
autorización independientes que añada deliberadamente esa capacidad.

Las observaciones manuales anteriores del 83EX con BIOS `NUCN15WW` indican que
esas rutas devolvieron respectivamente `performance` y `low-power balanced
performance max-power custom`. Son evidencia histórica de presencia y formato
en Nobara, no una garantía de que la ruta, su topología, sus permisos, su
pasividad o sus valores permanezcan iguales.

## Clases de afirmación

| Afirmación | Clase | Límite |
| --- | --- | --- |
| `openat2` resuelve una ruta relativa respecto de `dirfd`; `RESOLVE_BENEATH` rechaza escapes y rutas absolutas durante esa resolución. | Propiedad documentada del kernel. | No abre ni valida por sí sola el directorio raíz inicial. |
| `RESOLVE_NO_SYMLINKS` prohíbe resolver enlaces en todos los componentes; `O_NOFOLLOW` solo afecta el componente final. | Propiedad documentada del kernel. | Puede rechazar una topología sysfs legítima. |
| Un descriptor abierto conserva su referencia aunque el nombre de ruta se elimine o apunte después a otro objeto. | Propiedad documentada de la descripción de archivo abierta. | No vuelve coherentes ni inmutables los contenidos que el controlador genere. |
| sysfs expone detalles de implementación que pueden variar entre kernels; las aplicaciones deben evitar suposiciones de topología. | Recomendación documentada del kernel. | No determina si estas dos rutas concretas contienen enlaces. |
| El 83EX mostró ambos atributos durante una consulta manual previa. | Observación proporcionada del equipo. | No prueba que abrirlos sea pasivo ni que su contenido sea una instantánea atómica. |
| Usar descriptores anclados, nombres literales internos y fallo cerrado. | Decisión de diseño propuesta. | Requiere implementación y validación futuras. |

## Revisión estática del proveedor aislado actual

El código de Fase 5B.5B-2 en `crates/legion-linux/src/lib.rs` mantiene
`ConfinedOpenat2PlatformProfileSource` privado. Las pruebas le entregan solo
un descriptor de directorio temporal ya abierto; la variante de producción
solo puede recibir el descriptor de `/sys/firmware/acpi` desde
`SysfsPlatformProfileProvider`, cuyo constructor está bloqueado por
`ApprovedRealReadActivation`, un tipo privado sin variantes ni valor
construible. Además, `LinuxPlatformProfileOrigin` es no exhaustivo, de modo que
un consumidor externo tampoco puede fabricar la variante `LinuxReadOnly`.
Ninguno acepta ruta, variable de entorno,
argumento CLI ni proveedor externo. La implementación compilada no equivale a
una autorización para abrir sysfs:

| Parte actual | Valor para producción | Cambio o comprobación necesaria |
| --- | --- | --- |
| Nombres literales de `PlatformProfileEntry`. | Mantenerlos como única lista permitida. | Deben permanecer privados y no derivarse de entrada externa. |
| Límite interno de 4096 bytes y lectura de un byte adicional. | Reutilizable como política defensiva. | Validar que el atributo virtual permita EOF y que 4096 siga siendo razonable. |
| Conversión de `ENOENT`, permisos, `ENOSYS`, `ELOOP`, `EXDEV` y `EAGAIN` en errores. | Reutilizable como separación de errores de origen. | Conservar `errno` diagnóstico de forma estructurada y no degradar a datos. |
| Bucle de lectura: continuar ante lectura corta, completar solo en EOF. | Reutilizable como algoritmo. | Tratar `EAGAIN`, `EINTR` y errores después de bytes sin aceptar el búfer parcial. |
| `fstat` y exigencia de archivo regular. | Defensa adoptada por el proveedor bloqueado. | La activación real exige comprobar previamente que ambos atributos sysfs satisfacen esa expectativa; si no, debe permanecer desactivado. |
| `StaticSimulation` y etiqueta de raíz de pruebas. | Debe quedarse en fixtures. | Un proveedor de producción no podrá reutilizar su constructor ni construir la procedencia real antes del éxito total. |

El proveedor aislado protege contra sustitución de la raíz mediante el
descriptor y contra enlaces de atributos en sus fixtures. No demuestra que la
misma política sea compatible con `/sys/firmware/acpi`, ni que `fstat`, el
tamaño o EOF describan de forma completa la semántica de un atributo sysfs.

## Arquitectura recomendada

La primera integración real aprobada debería vivir en `legion-linux` como un
proveedor concreto privado, por ejemplo `SysfsPlatformProfileProvider`. No se
exportará a `legion-cli`, `legion-core` ni a consumidores externos.

```text
política de activación aprobada
             |
             v
proveedor privado Linux ── abre raíz fija por descriptores ──> dirfd anclado
             |                                                    |
             |                                        dos nombres internos
             v                                                    v
errores de origen <── openat2 + lectura acotada <── atributos sysfs
             |
             v
PlatformProfileAdapter (validación de contenido)
             |
             v
origen LinuxReadOnly, solo tras éxito completo
```

`legion-core` conserva modelos y validación, sin rutas, descriptores ni
dependencia de Linux. La CLI continúa usando fixtures. Aunque
`SysfsPlatformProfileProvider` se compila en `legion-linux`, no hay una
capacidad construible, exportación, argumento ni variable de entorno que pueda
instanciarlo o ejecutar su secuencia.

### Barrera de activación auditable

`ApprovedRealReadActivation` es un `enum` privado sin variantes. No hay en el
crate una función, feature, variable de entorno ni constructor que pueda crear
un valor de ese tipo; sin código `unsafe`, no puede existir un argumento para
`SysfsPlatformProfileProvider::new`. Por ello, compilar la implementación no
la activa.

Una fase futura tendría que modificar explícitamente ese tipo o añadir una
función interna que produzca el valor de activación. Ese cambio debe quedar en
un diff revisable junto con: la autorización para las dos rutas, una prueba de
que sigue sin haber entrada de ruta o CLI, la política de procedencia y las
pruebas aisladas actualizadas. No se permite introducir un feature oculto,
fallback, variable de entorno o constructor público como sustituto de esa
revisión.

El proveedor conserva internamente un candidato a origen `LinuxReadOnly`, pero
solo publica `LinuxPlatformProfileReading` después de completar las tres
lecturas, comprobar su coherencia y superar `PlatformProfileAdapter`. Si falla
cualquier paso, devuelve un error sin publicar una lectura ni procedencia real.

### Apertura directa frente a árbol anclado

| Alternativa | Ventaja | Riesgo o límite | Recomendación |
| --- | --- | --- | --- |
| Abrir las dos rutas absolutas fijas por separado. | Poco código y no incorpora entrada de usuario. | Cada apertura vuelve a resolver el prefijo; no conserva una raíz anclada y `O_NOFOLLOW` no protege componentes intermedios. | No usar como diseño principal. |
| Abrir una vez `/sys/firmware/acpi` y resolver ambos atributos mediante su `dirfd`. | Los atributos se resuelven desde la misma referencia de directorio y con nombres fijos. | La apertura inicial de la raíz sigue necesitando una política de ruta fija y verificación. | Base del diseño recomendado. |
| Abrir `/sys`, después `firmware` y después `acpi`, cada directorio desde el descriptor anterior. | Reduce la resolución por cadena a un único punto inicial y permite comprobar cada componente como directorio. | Más código y no prueba que el contenido del atributo sea estable. | Recomendación más conservadora antes de la primera lectura real. |

Para la apertura inicial de `/sys`, una ruta absoluta literal no puede usarse
con `RESOLVE_BENEATH`: esa bandera rechaza rutas absolutas. La propuesta es
usar esa ruta fija solo para obtener el primer descriptor, con
`O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW` y
`RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS`. A partir de ahí, `firmware`,
`acpi` y los dos atributos se abren exclusivamente como nombres relativos con
`RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS`; la incorporación de
`RESOLVE_NO_SYMLINKS` a la ruta real queda pendiente de comprobar que no
rechace una topología necesaria.

No habrá búsqueda de sysfs, raíz configurable, `SYSFS_PATH`, concatenación de
rutas, `canonicalize()`, fallback a `open()`/`openat()` ni reintento que cambie
banderas de confinamiento.

## Banderas, tipos de objeto y confinamiento

| Mecanismo | Uso propuesto | Garantía documentada | Decisión pendiente |
| --- | --- | --- | --- |
| `O_RDONLY` | Obligatorio en directorios y atributos. | Solicita apertura de solo lectura. | No demuestra que la lectura sea libre de efectos del controlador. |
| `O_CLOEXEC` | Obligatorio. | Evita la carrera de marcar el descriptor close-on-exec después de abrir. | Ninguna. |
| `O_NOFOLLOW` | Defensa complementaria en cada apertura. | Rechaza un enlace solo en el componente final. | No sustituye las banderas `RESOLVE_*`. |
| `O_NONBLOCK` | Incluido en la implementación bloqueada para tratar `EAGAIN` como error de origen. | Para archivos regulares no cambia el bloqueo; para otros objetos puede provocar `EAGAIN` si la operación bloquearía. | La autorización de activación debe aceptar explícitamente esta política para estos atributos sysfs. |
| `RESOLVE_BENEATH` | Obligatorio en aperturas relativas al `dirfd` anclado. | Impide que la resolución salga de ese `dirfd`; puede indicar carrera o escape con `EAGAIN`/`EXDEV`. | No aplica a la primera ruta absoluta. |
| `RESOLVE_NO_MAGICLINKS` | Obligatorio. | Prohíbe magic-links durante la resolución. | Ninguna conocida para estos atributos, pero debe probarse con fixtures. |
| `RESOLVE_NO_SYMLINKS` | Aplicado en la implementación bloqueada como política de máxima restricción. | Prohíbe enlaces en todos los componentes y también implica `NO_MAGICLINKS`. | Confirmar si `/sys/firmware/acpi` y los atributos requeridos no dependen de enlaces legítimos antes de habilitarla. |

La documentación de `openat2` advierte que `RESOLVE_NO_SYMLINKS` puede causar
errores en sistemas que antes funcionaban cuando se modifican componentes de la
ruta. No se debe desactivar automáticamente por ese motivo: el resultado debe
ser una incapacidad explícita y una revisión de topología aprobada.

Después de abrir la raíz, `fstat` puede confirmar que el descriptor representa
un directorio antes de usarlo como ancla. Para los atributos, `fstat` aporta
diagnóstico de tipo, pero no es una prueba de que un nodo virtual tenga tamaño
convencional, que EOF represente una instantánea estable, ni que la lectura sea
pasiva. La política inicial debe registrar el tipo observado y fallar si no
coincide con la expectativa que se haya validado para esas rutas; no debe
inventar una equivalencia entre “archivo regular” y “atributo sysfs seguro”.

## Secuencia propuesta de apertura, lectura y cierre

Esta es una secuencia de diseño, no un procedimiento autorizado para ejecutar:

1. Comprobar que existe una política de activación interna creada tras la
   autorización específica del usuario para el modelo, BIOS, kernel, rutas y
   comportamiento de lectura.
2. Abrir la raíz fija mediante la política anterior y obtener un `OwnedFd`.
   Verificar con `fstat` que es directorio. Si no puede garantizarse la
   resolución, detenerse.
3. Abrir los directorios descendientes por `dirfd`, con nombres literales y
   `openat2`; no conservar cadenas de ruta como autoridad posterior.
4. Abrir un atributo solo con su nombre literal, banderas de lectura y la
   política `RESOLVE_*` aprobada. No usar banderas de creación, escritura o
   truncado.
5. Leer hasta 4096 bytes, solicitando solo un byte adicional para detectar
   exceso. Una lectura corta se acumula; solo `read() == 0` completa el valor.
6. Si el contenido completo no es UTF-8, o el parser detecta vacío, controles,
   duplicados o incoherencia, publicar un error, nunca una normalización.
7. Cerrar cada descriptor por RAII. No reintentar `close()` después de un error
   porque el descriptor podría haber sido liberado.
8. Construir `LinuxReadOnly` únicamente después de que todas las lecturas,
   comprobaciones de coherencia y validaciones hayan tenido éxito.

No existe una garantía de que leer sysfs no llame código de un controlador. La
autorización de cada lectura real debe confirmar que estas rutas son pasivas
para el contexto concreto, o aceptar explícitamente ese riesgo documentado.

## Política de errores y datos incompletos

| Condición | Clasificación | Publicación |
| --- | --- | --- |
| `ENOSYS` | `Openat2Unavailable`. | Fallo cerrado; no fallback. |
| `ENOENT` | Ausente. | Error de origen; no inferir incompatibilidad permanente. |
| `EACCES` / `EPERM` | Permiso insuficiente. | Error de origen; no pedir `sudo` ni cambiar permisos. |
| `ELOOP`, `EXDEV`, `EAGAIN` durante resolución con restricciones. | Confinamiento no garantizado o topología prohibida. | Error de seguridad; no relajar banderas ni aceptar el atributo. |
| `EINVAL`, `E2BIG` u otros errores de ABI/banderas. | Protocolo o entorno no compatible. | Error de origen; revisar versión de kernel, Rustix y política. |
| `EINTR` antes de transferir bytes. | Interrupción. | Como máximo un reintento documentado para la misma lectura; después, error. |
| Error después de acumular bytes; `EAGAIN` al usar no bloqueo. | Lectura incompleta. | Descartar el búfer y devolver `PartialRead`/`ReadFailed`. |
| EOF sin bytes. | Contenido completo pero vacío. | Pasar al parser, que debe rechazarlo como vacío. |
| Más de 4096 bytes, UTF-8 inválido o formato inválido. | Límite o interpretación. | No publicar una lectura válida. |

El proveedor real no debe transformar `ENOENT`, permiso, errores de
confinamiento ni lectura parcial en un estado `Available`. La distinción entre
error de origen y `PlatformProfileError` de interpretación debe preservarse en
la API y en diagnósticos.

## Coherencia de `platform_profile` y `platform_profile_choices`

Los dos atributos no forman una transacción. Una secuencia conservadora es:

1. leer `platform_profile_choices` como `choices_before`;
2. leer `platform_profile` como `active`;
3. leer de nuevo `platform_profile_choices` como `choices_after`;
4. publicar solo si `choices_before == choices_after` y
   `PlatformProfileAdapter` confirma que `active` aparece en esa lista.

La igualdad de las dos listas detecta algunos cambios entre lecturas, pero no
prueba una instantánea atómica: un cambio A→B→A podría no detectarse y el
controlador podría cambiar el perfil inmediatamente después. Si cualquiera de
las lecturas falla, difiere o no valida, el resultado será `ConcurrentChange`,
`ReadFailed` o un error de interpretación; nunca se combinarán valores ni se
habilitarán controles. La política inicial recomendada es no reintentar la
secuencia automáticamente; cualquier reintento futuro tendrá que ser acotado,
observable y aprobado.

## Procedencia y transición de la interfaz sellada

La procedencia real no debe ser una elección del llamador. Cuando se autorice
la implementación:

1. Mantener `PlatformProfileByteSource` sellado.
2. Mantener `SimulatedPlatformProfileTree` como único proveedor que puede
   construir `StaticSimulation`.
3. Añadir un proveedor concreto privado que solo pueda construirse con la
   política interna de activación aprobada y que no reciba rutas.
4. Reemplazar el origen de tipo genérico por una fuente interna enumerada, o
   por constructores separados que produzcan un tipo de resultado cerrado.
5. `LinuxReadOnly` ya existe, pero solo el proveedor privado puede producirlo;
   construir `TelemetryArtifact::LinuxReadOnly` después del éxito completo.
6. Añadir una prueba negativa que demuestre que los fixtures, una etiqueta de
   raíz y datos arbitrarios no pueden construir ni serializarse como origen
   real.

La procedencia `LinuxReadOnly` significa únicamente que las dos aperturas y
lecturas aprobadas finalizaron correctamente. No significa que la capacidad de
control esté validada, que el perfil cambie el rendimiento observado ni que se
haya validado cualquier operación de escritura.

## Modelo de amenazas y límites

| Amenaza o condición | Mitigación propuesta | Límite residual |
| --- | --- | --- |
| Entrada de ruta desde usuario, CLI o entorno. | No exponer rutas; usar nombres y raíz literales internos. | La ruta raíz fija debe revisarse antes de activar. |
| Enlace o escape desde el atributo. | `dirfd`, `RESOLVE_BENEATH`, `NO_MAGICLINKS`, y política de enlaces explícita. | No decidir `NO_SYMLINKS` sin conocer la topología real. |
| Sustitución de nombre después de abrir. | Conservar `OwnedFd`; leer mediante descriptor. | El contenido virtual puede cambiar por el controlador. |
| Descriptor filtrado a un proceso hijo. | `O_CLOEXEC` en cada apertura. | No sustituye una política general de ejecución de procesos; el proyecto no ejecuta procesos externos. |
| Objeto inesperado que bloquee o no se comporte como archivo. | Verificar tipo aprobado; evaluar `O_NONBLOCK`; límite y EOF. | sysfs no garantiza la semántica de un archivo convencional. |
| Cambio concurrente entre atributos. | Lectura `choices` antes/después y validación cruzada. | No hay atomicidad ni detección de ABA. |
| Kernel sin `openat2` o con ABI incompatible. | `ENOSYS`/`EINVAL`/`E2BIG` como fallo cerrado. | No existe alternativa automática. |
| Lectura que active comportamiento del controlador. | Revisión específica de pasividad y autorización por ruta. | No se puede inferir desde las banderas de apertura. |

## Pruebas y criterios de aceptación antes de activar lectura real

### Fixtures aisladas obligatorias

- Árbol de directorios por descriptores: rutas válidas, cada componente como
  enlace, objetos no directorio, atributos no regulares y sustitución de raíz o
  atributo después de adquirir el descriptor.
- Mapeo determinista de `ENOSYS`, `ENOENT`, `EACCES`, `EPERM`, `ELOOP`,
  `EXDEV`, `EAGAIN`, `EINVAL`, `E2BIG` y `EINTR` sin fallback.
- EOF vacío, contenido exactamente de 4096 bytes, byte 4097, UTF-8 inválido,
  lecturas cortas, error antes/después de bytes y `EAGAIN` con no bloqueo.
- Secuencias de `choices_before`/`active`/`choices_after` iguales, distintas e
  inválidas; demostrar que las inconsistencias no producen salida disponible.
- Pruebas de tipos para impedir que fixtures, entradas arbitrarias o la CLI
  fabriquen `LinuxReadOnly`.

### Condiciones previas a cualquier lectura real

1. Revisión de seguridad aprobada del código de producción, incluida la
   apertura inicial de raíz, el tipo de objeto esperado y la política de
   enlaces.
2. Confirmación documentada de compatibilidad por modelo 83EX, BIOS `NUCN15WW`,
   kernel, interfaz y operación concreta; ausencia de evidencia será
   `Unvalidated`, no compatibilidad implícita.
3. Autorización explícita para las dos rutas exactas, banderas, límite,
   procedimiento de registro y tratamiento de errores.
4. Protocolo de observación que registre identidad, versión de kernel, fecha,
   rutas, resultado, `errno`, contenido redactado si fuera necesario y que no
   cambie perfil alguno.
5. Resultado satisfactorio de una comprobación pasiva independiente sobre
   existencia, permisos, resolución de enlaces y tipo de los objetos. Si falla
   una condición de confinamiento, conservar el proveedor de fixtures.

## Riesgos pendientes y recomendación

Falta evidencia directa sobre enlaces, puntos de montaje, tipo de objeto y
semántica de EOF de las dos rutas en el 83EX. También falta demostrar que sus
lecturas no activan comportamiento con efectos en el controlador y validar el
mecanismo de autorización que construiría el proveedor real.

Por ello, no se debe crear la capacidad de activación ni llamar al constructor
privado todavía. La próxima fase razonable es una revisión de esta interfaz
bloqueada y de las pruebas aisladas adicionales; cualquier consulta real deberá
ser una etapa distinta con autorización expresa, sin fallback y con un
protocolo de observación revisado.

## Fuentes públicas

- [openat2(2)](https://man7.org/linux/man-pages/man2/openat2.2.html)
- [open(2)](https://man7.org/linux/man-pages/man2/open.2.html)
- [read(2)](https://man7.org/linux/man-pages/man2/read.2.html)
- [Reglas de acceso a sysfs](https://docs.kernel.org/admin-guide/sysfs-rules.html)
- [ABI `platform_profile`](https://docs.kernel.org/userspace-api/sysfs-platform_profile.html)
- [API `rustix::fs::openat2` 1.1.5](https://docs.rs/rustix/1.1.5/rustix/fs/fn.openat2.html)
