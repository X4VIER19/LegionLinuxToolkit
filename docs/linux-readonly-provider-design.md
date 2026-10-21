# Diseño del proveedor Linux real de solo lectura (Fase 5B.4)

## Alcance y estado

Este documento investiga un proveedor futuro para leer únicamente:

- `/sys/firmware/acpi/platform_profile`;
- `/sys/firmware/acpi/platform_profile_choices`.

No se implementa ningún proveedor real en esta fase. No se abrió ninguna ruta
del host, no se invocó firmware y no se ejecutaron experimentos. Las
observaciones manuales ya registradas para el 83EX/`NUCN15WW` fueron
`performance` y `low-power balanced performance max-power custom`; son
evidencia de presencia y contenido en ese momento, no una autorización para
que LegionLinuxToolkit las consulte automáticamente.

## Clase de afirmación

| Elemento | Clase | Límite |
| --- | --- | --- |
| `openat2()` existe desde Linux 5.6 y sus banderas `RESOLVE_*` restringen la resolución de rutas. | Garantía documentada del kernel. | No prueba que la llamada o el ABI estén disponibles en la compilación/distribución concreta. |
| `O_NOFOLLOW` solo afecta al componente final; `RESOLVE_NO_SYMLINKS` cubre todos los componentes. | Garantía documentada de `open(2)`/`openat2(2)`. | Rechazar enlaces puede impedir rutas legítimas de sysfs. |
| sysfs puede usar enlaces legítimos, sobre todo en árboles de clase, bus y dispositivo. | Regla documentada de sysfs. | No confirma si los dos atributos objetivo ni sus ancestros son enlaces en este equipo. |
| Las dos rutas objetivo respondieron a una lectura manual anterior en Nobara. | Observación del 83EX. | No es una prueba automática, de estabilidad ni de pasividad futura. |
| El proveedor debe usar rutas constantes, un descriptor de directorio y `openat2` cuando esté disponible. | Decisión de diseño propuesta. | Requiere revisión e implementación posterior. |

## Alternativas de apertura

| Alternativa | Ventajas | Garantías y límites | Decisión |
| --- | --- | --- | --- |
| Abrir cada ruta absoluta fija con `O_RDONLY | O_CLOEXEC | O_NOFOLLOW`. | Pequeña y sin entrada de ruta del usuario. | `O_RDONLY` no solicita escritura; `O_CLOEXEC` evita filtrar el descriptor a un proceso ejecutado; `O_NOFOLLOW` solo rechaza un enlace en el componente final. No confina los componentes anteriores ni ofrece una defensa general ante sustituciones. | Insuficiente como diseño de seguridad principal. |
| Abrir una raíz fija y usar `openat()` con nombres constantes. | El segundo paso no procesa rutas arbitrarias ni `..`. | Conserva el descriptor de la raíz, pero `openat()` y `O_NOFOLLOW` no cubren enlaces en todos los componentes y no aportan las reglas de resolución de `openat2`. | Posible compatibilidad de último recurso, pero no se activará sin una revisión específica. |
| Abrir una raíz fija y usar `openat2()` para cada atributo con una política de resolución explícita. | Permite expresar confinamiento respecto de un `dirfd`, rechazar escapes y distinguir errores de resolución. | Requiere Linux 5.6+, una interfaz Rust revisada y decidir la política de enlaces después de verificar la ruta concreta. No vuelve atómica una lectura de dos atributos. | Recomendación técnica para el primer proveedor real. |

La versión de kernel Nobara previamente registrada (`7.2.0-202.nobara.fc44`) es
posterior a 5.6, por lo que es razonable investigar `openat2`; aun así, no se
ha comprobado en ejecución la disponibilidad del syscall, las cabeceras ni las
opciones de empaquetado de ese entorno.

## Banderas y confinamiento

La propuesta de apertura es siempre de lectura, sin `O_CREAT`, `O_TRUNC`,
`O_WRONLY` ni `O_RDWR`:

| Mecanismo | Propósito | Garantía relevante | Política propuesta |
| --- | --- | --- | --- |
| `O_RDONLY` | Solicitar un descriptor legible. | El descriptor no se abre para escritura. | Obligatorio. |
| `O_CLOEXEC` | No heredar el descriptor tras un `exec`. | Evita la carrera de aplicar `FD_CLOEXEC` después de abrir. | Obligatorio. |
| `O_NOFOLLOW` | Rechazar enlace en el componente final. | No protege componentes anteriores. | Mantener como defensa final cuando la API lo soporte. |
| `RESOLVE_BENEATH` | Impedir que una resolución relativa salga del `dirfd`. | Rechaza rutas absolutas, `..` y enlaces que escapen; puede devolver `EXDEV` o `EAGAIN` si no puede garantizarlo. | Recomendado para los nombres constantes de atributo. |
| `RESOLVE_NO_MAGICLINKS` | Bloquear magic-links de estilo `/proc`. | Evita una categoría de enlaces especiales. | Recomendado como defensa explícita. |
| `RESOLVE_NO_SYMLINKS` | Rechazar cualquier enlace en toda la resolución. | Más fuerte que `O_NOFOLLOW`, pero puede fallar en rutas legítimas que usen enlaces. | No habilitar hasta verificar los dos caminos concretos. |
| `RESOLVE_IN_ROOT` | Tratar el `dirfd` como raíz durante la resolución. | Contiene `/` y `..` dentro de esa raíz; no usarlo junto con una política distinta sin revisión de compatibilidad. | Alternativa a evaluar, no combinar ni activar por defecto en esta fase. |
| `RESOLVE_NO_XDEV` | Bloquear cruces de puntos de montaje y bind mounts. | Puede romper rutas legítimas o distribuciones con montajes distintos. | No usar sin evidencia de que la ruta objetivo no los requiere. |

`openat2` rechaza banderas desconocidas o conflictivas, a diferencia de
`openat`, y documenta `ELOOP` para `RESOLVE_NO_SYMLINKS`, `EXDEV` para escapes
detectados y `EAGAIN` cuando no puede garantizar algunas restricciones ante una
carrera. Esos resultados deben publicarse como errores de origen, no degradarse
a una ruta alternativa ni reintentarse sin límite.

## Política de enlaces y rutas

La raíz lógica prevista es el directorio fijo `/sys/firmware/acpi`, abierto una
vez por el proveedor real aprobado. Los únicos nombres que podrá recibir la
apertura relativa serán literales internos: `platform_profile` y
`platform_profile_choices`. No habrá raíz, subruta, atributo ni bandera
suministrados por CLI, archivo de configuración o variable de entorno.

La política propuesta tiene dos etapas:

1. En una comprobación futura expresamente autorizada, registrar si cada
   componente de las dos rutas es directorio real, enlace simbólico o punto de
   montaje, sin asumir que el patrón de sysfs de clases se aplica a esta rama de
   firmware.
2. Si los atributos y ancestros necesarios son objetos reales, usar
   `RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS` y evaluar
   `RESOLVE_NO_SYMLINKS` como endurecimiento adicional. Si un enlace legítimo
   es imprescindible, documentar su objetivo y mantener el confinamiento con
   `RESOLVE_BENEATH`; no sustituirlo por `canonicalize()` ni por concatenación
   de rutas.

Las reglas de sysfs recomiendan resolver enlaces de árboles de dispositivo a
su destino real y advierten contra acceder a atributos a través de un enlace a
otro dispositivo. Ese consejo impide aplicar una prohibición general sin
contexto: para estos dos atributos se necesita evidencia específica de la
rama `/sys/firmware/acpi` antes de decidir si el rechazo total de enlaces es
compatible.

## Lectura, EOF y consistencia

El proveedor futuro deberá imponer `MAX_PLATFORM_PROFILE_BYTES = 4096` como
constante interna. El tamaño esperado es muy inferior, pero el límite reduce
memoria y evita aceptar contenido inesperadamente grande.

Para cada descriptor, el algoritmo propuesto es:

1. Leer en un búfer de hasta límite + 1 bytes, en iteración hasta EOF.
2. Un retorno corto no es por sí mismo una lectura parcial: continuar hasta
   EOF. EOF sin bytes se conserva como texto vacío y el parser lo rechazará.
3. Si se supera el límite, descartar el contenido y devolver `TooLarge`.
4. Si hay `EINTR` antes de transferir bytes, permitir un número pequeño y
   documentado de reintentos; agotarlo es `ReadFailed`. Si hay error tras
   acumular bytes, descartar el búfer y devolver `PartialRead` o `ReadFailed`,
   nunca interpretar lo acumulado.
5. Decodificar UTF-8 solo al completar; bytes inválidos son `InvalidUtf8`.
6. Cerrar por RAII; no reintentar un cierre interrumpido, porque Linux puede
   haber liberado el descriptor aunque informe error.

Dos lecturas no forman una transacción sysfs. La opción conservadora para una
instantánea coherente es leer `choices`, luego `active`, y después `choices`
otra vez. Solo se publicará la instantánea si ambas listas de opciones son
idénticas y `PlatformProfileAdapter` confirma que el perfil activo pertenece a
la lista. Si cambian, el resultado será `ConcurrentChange`/`ReadFailed`; no se
mezclarán valores ni se habilitará una operación. Un único reintento completo,
si se aprueba, debe estar acotado y quedar registrado.

## Errores, validación y procedencia

La separación propuesta es:

| Capa | Ejemplos | Resultado |
| --- | --- | --- |
| Origen Linux | no existe, permiso denegado, `ELOOP`, `EXDEV`, `EAGAIN`, interrupción agotada, UTF-8, límite, cambio concurrente. | Nuevo error de origen estructurado con el nombre lógico, clase de error y `errno` diagnóstico opcional. |
| Interpretación | vacío, caracteres de control, duplicados, perfil activo no anunciado. | `PlatformProfileError` de `legion-core`. |
| Validación de soporte | Modelo/BIOS/kernel no coinciden o no hay evidencia suficiente. | Estado `Unvalidated` o rechazo por registro de compatibilidad; nunca una lectura se convierte por sí sola en control habilitado. |

El proveedor real no debe aceptar una procedencia proporcionada por el
llamador. Al aprobarse, `legion-linux` construirá internamente
`LinuxPlatformProfileOrigin::LinuxReadOnly` (renombrando el marcador actual
`LinuxReadOnlyReserved`) y `TelemetryArtifact::LinuxReadOnly`; el proveedor
virtual seguirá produciendo exclusivamente `StaticSimulation`. Una prueba debe
demostrar que ningún constructor de fixture puede obtener la etiqueta real.

## Evolución propuesta de la interfaz sellada

Actualmente `PlatformProfileByteSource` está sellada y el único implementador
es `SimulatedPlatformProfileTree`. Al autorizar un proveedor real:

1. Mantener el trait sellado y añadir un implementador privado
   `SysfsPlatformProfileProvider` dentro de `legion-linux`.
2. Sustituir el genérico público del lector por una fuente interna enumerada o
   constructores explícitos: uno de simulación y otro de acceso real aprobado.
3. El constructor real no recibirá rutas: contendrá los dos nombres constantes,
   la raíz interna fija y las banderas revisadas.
4. Exigir una capacidad de política creada por la aplicación tras confirmación
   explícita del usuario; no usar variables de entorno ni fallback para crearla.
5. Conservar por separado los diagnósticos del syscall, la procedencia real y
   la salida tipada de `PlatformProfileAdapter`.

Esto conserva las pruebas de fixtures y evita que un consumidor externo
implemente por accidente un lector de host.

## Pruebas necesarias antes de activar una lectura real

| Prueba | Fixtures aisladas | Sysfs real con autorización independiente |
| --- | --- | --- |
| Límites, UTF-8, EOF, interrupciones, errores y contenido malformado. | Sí. | No. |
| Nombres fijos, rechazo de traversal, enlaces, sustitución y procedencia. | Sí, con árbol de prueba controlado y pruebas de carrera. | No inicialmente. |
| Semántica de `openat2` y mapeo de errores por kernel soportado. | Sí, con una raíz temporal y compatibilidad condicionada. | No. |
| Existencia, permisos, tipo y resolución concreta de las dos rutas 83EX. | No. | Sí; consulta pasiva explícitamente aprobada. |
| Confirmar que la lectura no produce cambios de perfil y que los valores se mantienen coherentes. | No. | Sí; protocolo previo, registro y supervisión aprobados. |

## Riesgos y decisiones pendientes

- Falta comprobar el comportamiento de enlaces y montajes de las dos rutas
  concretas; `RESOLVE_NO_SYMLINKS` podría ser correcto o podría romper una ruta
  legítima.
- Falta decidir si la dependencia necesaria para una interfaz Rust segura de
  `openat2` puede incorporarse bajo la licencia dual y las restricciones de
  dependencias. No se añade ninguna en esta fase.
- `openat2` no ofrece una transacción entre los dos atributos; la política de
  doble lectura reduce mezclas, no prueba que el firmware no cambie de nuevo
  inmediatamente después.
- Una lectura sysfs puede ejecutar código del controlador. Antes de usarla se
  debe confirmar que estas dos rutas concretas son pasivas según las reglas del
  proyecto y obtener autorización para los comandos exactos.

## Recomendación

No implementar ni activar aún una lectura real. Para la siguiente fase, usar
primero fixtures de un proveedor `openat2` aislado y una revisión de API Rust.
Después, pedir autorización separada para una comprobación pasiva mínima de las
dos rutas del 83EX, con nombres, banderas, límite, manejo de errores y política
de enlaces ya aprobados. Si la ruta exige enlaces no documentados o la
confinación no puede demostrarse, mantener el proveedor virtual hasta rediseñar
la capa de apertura.

## Fuentes públicas consultadas

- [openat2(2)](https://man7.org/linux/man-pages/man2/openat2.2.html)
- [open(2)](https://man7.org/linux/man-pages/man2/open.2.html)
- [Resolución de rutas Linux](https://man7.org/linux/man-pages/man7/path_resolution.7.html)
- [read(3p)](https://man7.org/linux/man-pages/man3/read.3p.html)
- [close(2)](https://man7.org/linux/man-pages/man2/close.2.html)
- [Reglas de acceso a sysfs](https://docs.kernel.org/admin-guide/sysfs-rules.html)
- [UAPI `openat2.h` del kernel Linux](https://github.com/torvalds/linux/blob/master/include/uapi/linux/openat2.h)
