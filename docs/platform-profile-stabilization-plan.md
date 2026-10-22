# Plan de estabilización de `platform_profile` (Fase 5C.1)

## Propósito y estado de evidencia

Este plan describe una evolución posible de la consulta Linux de
`platform_profile` hacia una capacidad estable, mantenible y estrictamente de
solo lectura. No autoriza implementación, activación, nuevas lecturas ni
cambios en los perfiles.

La única evidencia real de LegionLinuxToolkit es una ejecución autorizada el
2026-10-21 en la Lenovo Legion Slim 5 16ARP9 (83EX), BIOS `NUCN15WW`, Nobara
con kernel `7.2.0-202.nobara.fc44.x86_64`. La secuencia
`choices → active → choices` fue completa y coherente, con `performance` como
perfil activo y `low-power balanced performance max-power custom` como
opciones. La observación se publicó como `LinuxReadOnly` con confianza baja.

| Afirmación | Estado | Límite |
| --- | --- | --- |
| El proveedor experimental pudo leer los dos atributos fijos una vez. | Validado en la ejecución descrita. | No valida ejecuciones posteriores, otras BIOS, kernels, distribuciones ni equipos. |
| `PlatformProfileAdapter` conserva texto original y valida perfiles, duplicados y controles. | Validado por pruebas aisladas y la lectura concreta. | No convierte un perfil conocido en permiso para cambiarlo. |
| `choices → active → choices` detecta una lista que cambió entre lecturas. | Validado como diseño y en fixtures. | No ofrece atomicidad ni detecta una carrera A→B→A. |
| El acceso sysfs es pasivo para cualquier estado del firmware. | Desconocido. | Las banderas de apertura no prueban ausencia de efectos en el controlador. |
| La consulta puede declararse estable para cualquier distribución Linux. | No validado. | Cada combinación de modelo, BIOS, kernel y distribución necesita evidencia propia. |

La estabilidad propuesta se limita a **observar** el perfil y sus opciones. No
incluye escribir `platform_profile`, controles de ventilador, potencia, WMI,
ACPI, EC, firmware ni telemetría adicional.

## Arquitectura objetivo

La única lógica de apertura y validación debe seguir residiendo en
`legion-linux`; `legion-core` permanece independiente de Linux, rutas y
descriptores.

### Estado de Fases 5C.2 y 5C.3

La Fase 5C.2 implementa la interfaz pública compartida
`PlatformProfileQuery::read_once()` en `legion-linux`. El lector existente de
fuentes inyectadas la implementa para simulaciones, conserva `read()` solo como
alias de compatibilidad y sigue pasando todo resultado por
`PlatformProfileAdapter`. El proveedor sysfs privado también la implementa
únicamente bajo la feature experimental, con su secuencia reforzada de tres
lecturas; no se ejecuta ni se conecta a `legion-cli` en esta fase.

Las nuevas pruebas usan fuentes simuladas para verificar una lectura válida,
ausencia, incoherencia y preservación de `StaticSimulation`. No introducen una
fuente Linux nueva, una ruta configurable ni una forma de construir
`LinuxReadOnly` desde fixtures.

La Fase 5C.3 conecta el comando `legion-cli platform-profile` a esa misma
interfaz, pero solo mediante `LinuxPlatformProfileReader` y un
`SimulatedPlatformProfileTree` construido con escenarios embebidos de
`legion-fixtures`. La CLI no recibe un proveedor, una ruta, una raíz ni una
selección de origen: convierte el enum cerrado del escenario en bytes en
memoria y llama una vez a `PlatformProfileQuery::read_once()`. Los datos
ausentes se preservan como ausencia, no como cadena vacía ni error de origen;
los errores de adquisición simulados siguen siendo distintos de los errores
del adaptador. Toda salida de esta ruta conserva `StaticSimulation`.

```text
CLI estable / GUI futura
        │ petición explícita de una sola consulta
        v
PlatformProfileQuery (interfaz de aplicación, sin rutas)
        │
        ├── StaticPlatformProfileQuery ──> fixtures / StaticSimulation
        │
        └── LinuxPlatformProfileQuery ──> proveedor privado openat2
                                                │
                                                └── dos nombres fijos
                                                    platform_profile_choices
                                                    platform_profile
        │
        v
PlatformProfileAdapter en legion-core
        │
        v
PlatformProfileObservation { origen, confianza, resultado o error }
```

### Separación de origen y selección de fuente

La interfaz de aplicación propuesta representa una sola consulta:

```text
PlatformProfileQuery::read_once() -> Result<PlatformProfileObservation, PlatformProfileQueryError>
```

No recibe raíz, ruta, descriptor, atributo, variables de entorno ni una opción
de reintento. La selección se hará por un enum cerrado de nivel de aplicación:

| Selección | Implementación | Procedencia posible |
| --- | --- | --- |
| `StaticSimulation` | Fixture embebida o inyectada para pruebas. | Solo `StaticSimulation`. |
| `LinuxReadOnly` | Fachada Linux privada sobre el proveedor confinado. | Solo después de las tres lecturas completas, coherentes y validadas. |

El tipo resultado conservará por separado el valor bruto, la forma normalizada,
el estado (`Available`, `Unvalidated`, `Unavailable` o error), la procedencia y
la confianza. Una salida de error no contiene una observación válida ni se
etiqueta como `LinuxReadOnly`.

Los perfiles desconocidos que superen las reglas sintácticas se conservarán
como vocabulario desconocido y estado `Unvalidated`; se mostrarán al usuario,
pero no habilitarán controles, perfiles preseleccionados ni inferencias sobre
capacidades. Perfiles vacíos, duplicados, con controles no permitidos o cuyo
activo no aparezca en opciones seguirán siendo errores de interpretación.

### Reutilización por CLI y GUI

La CLI y una GUI futura usarán la misma interfaz de aplicación y el mismo
resultado tipado; ninguna volverá a abrir sysfs, analizar texto ni construir
provenencias por su cuenta. La GUI no iniciará consultas al arrancar, al volver
al primer plano ni mediante temporizadores: una acción de usuario deberá crear
una sola petición `read_once()`.

La CLI conservará sus comandos simulados actuales. Una fase posterior podría
añadir un subcomando de lectura Linux explícito y separado de los escenarios,
por ejemplo una selección cerrada `--source linux-read-only`. Tal subcomando
no aceptará `--root`, `--path`, `--sysfs`, configuraciones ni entorno y deberá
requerir una confirmación de usuario visible. Su existencia no se propone para
esta fase ni debe convertir la consulta en un comportamiento predeterminado.

## Política de acceso real

La implementación estable conservará exactamente estos literales internos:

- `platform_profile_choices`;
- `platform_profile`.

La raíz y los directorios intermedios permanecerán internos y fijos:
`/sys` → `firmware` → `acpi`. El proveedor continuará usando descriptores de
directorio y `openat2`; no habrá concatenación de rutas, `canonicalize()`,
`open()`/`openat()` de reserva, exploración de sysfs ni adaptador genérico de
archivos.

| Etapa | Banderas y comprobación | Fallo seguro |
| --- | --- | --- |
| Raíz `/sys` | `O_RDONLY`, `O_DIRECTORY`, `O_CLOEXEC`, `O_NOFOLLOW`, `RESOLVE_NO_MAGICLINKS`, `RESOLVE_NO_SYMLINKS`; comprobar directorio. | Error de inicialización; no fallback. |
| `firmware` y `acpi` | Apertura relativa por descriptor con las banderas anteriores y `RESOLVE_BENEATH`. | Error de confinamiento o tipo; no continuar. |
| Atributos | `O_RDONLY`, `O_CLOEXEC`, `O_NOFOLLOW`, `O_NONBLOCK`, `RESOLVE_BENEATH`, `RESOLVE_NO_MAGICLINKS`, `RESOLVE_NO_SYMLINKS`; comprobar tipo según la política validada. | Error de origen; descartar cualquier búfer. |
| Lectura | Máximo 4096 bytes más un byte de detección; acumular solo hasta EOF. | `EINTR` agotado, `EAGAIN`, error tras bytes o exceso: error, nunca dato parcial. |

`ENOSYS`, `EINVAL` o `E2BIG` señalan que `openat2` o su política no son
utilizables en ese entorno. `ELOOP`, `EXDEV` y `EAGAIN` durante la resolución
se tratarán como confinamiento no garantizado. `ENOENT`, `EACCES` y `EPERM`
son errores de disponibilidad o permisos, no evidencia de incompatibilidad
permanente. Ningún caso activa elevación de privilegios, relajación de banderas
ni un segundo método de apertura.

## Coherencia, repetición y ciclo de vida

Cada solicitud explícita realizará como máximo una consulta lógica:

1. leer opciones;
2. leer perfil activo;
3. leer opciones de nuevo;
4. comparar ambas listas y validar que el activo pertenece a la lista;
5. publicar una observación o un error y finalizar.

La API no incluirá `watch`, `refresh_loop`, tareas en segundo plano, servicios,
procesos residentes ni reintentos automáticos. Una segunda actualización en la
CLI o GUI será otra acción de usuario y debe pasar por la política de
autorización vigente; no se programará ni se desencadenará por eventos de
sistema.

Si las listas difieren, el resultado será `ConcurrentChange`, sin combinar
valores ni intentar otra vez. Aunque coincidan, la presentación debe advertir
que la comprobación no es atómica y no demuestra que el perfil permanezca
igual después de cerrar los descriptores.

## Compatibilidad y validación por entorno

El registro de compatibilidad debe asociar cada observación con, como mínimo:

- modelo exacto y, cuando esté disponible sin inventarlo, identidad DMI;
- BIOS;
- distribución, versión de kernel y arquitectura;
- interfaz y operación: lectura de los dos atributos de `platform_profile`;
- política `openat2` y versión de la implementación;
- resultado, clase de error si existe y fecha de la evidencia.

Una coincidencia incompleta, una BIOS desconocida o un kernel/distribución sin
evidencia no habilitarán de forma silenciosa la fuente Linux. La UX podrá
mostrarla como no validada y permitir solo el camino de revisión/autorización
que se defina posteriormente; nunca deberá sustituirla por datos simulados sin
indicarlo.

La primera ejecución real constituye evidencia para el 83EX/`NUCN15WW`/Nobara
documentado, pero su confianza continúa baja porque es una única muestra. Para
elevarla no basta repetir automáticamente: harán falta ejecuciones manuales
independientes, autorizadas y registradas en estados de perfil conocidos, así
como resultados de fallo cerrado en entornos no compatibles.

## Criterios de aceptación antes de declararla estable

1. Mantener todas las pruebas simuladas de confinamiento, UTF-8, EOF, tamaño,
   enlaces, sustitución, permisos, `openat2` no disponible y concurrencia.
2. Añadir pruebas de interfaz para comprobar que la CLI y GUI futura solo usan
   `PlatformProfileQuery`, no rutas ni proveedores concretos.
3. Probar que `StaticSimulation` no puede construir ni serializar
   `LinuxReadOnly`, y que los errores nunca contienen una observación válida.
4. Compilar explícitamente con y sin la feature temporal durante transición;
   comprobar que la compilación predeterminada no contiene una ruta de lectura
   real ni el binario experimental.
5. Definir un protocolo de ejecuciones manuales autorizadas: identidad y
   versiones registradas, comando exacto, salida completa, sin `sudo`, sin
   cambios de perfil y sin reintentos automáticos.
6. Reunir evidencia en al menos los entornos declarados compatibles o marcar
   claramente los demás como no validados. No declarar soporte de CachyOS,
   Linux Mint u otros sistemas por diseño teórico.
7. Revisar por separado que la política de tipo de objeto y enlaces continúa
   siendo aceptada por las rutas concretas de sysfs; un error debe conservar el
   modo cerrado.
8. Obtener una revisión de seguridad y autorización específica antes de cada
   nueva lectura real durante la transición. La estabilidad de software no
   amplía los permisos de hardware.

## Transición de la herramienta experimental

No debe duplicarse el lector `openat2`. La extracción futura ha de mover la
secuencia confinada y `PlatformProfileAdapter` a una implementación privada
compartida; la herramienta experimental y la futura fachada estable solo
serán dos políticas de invocación de esa misma lógica.

Mientras se reúna evidencia, conviene conservar el binario experimental como
herramienta de diagnóstico estrictamente limitada, con feature no
predeterminada, confirmación literal y `AtomicBool` por proceso. No se le debe
añadir una opción de monitorización ni caminos de escritura.

Solo tras cumplir los criterios de aceptación, una fase aprobada podrá decidir
entre retirarlo o mantenerlo como herramienta de ingeniería. Si se retira, el
diff debe eliminar feature, binario, fachada y estado atómico juntos, y dejar
pruebas de que no quedan rutas de activación alternativas. La herramienta no
debe coexistir con una segunda implementación de apertura.

## Propuesta de cambio futuro para `AGENTS.md` (no aplicada)

La regla general de lecturas pasivas es demasiado amplia para considerar esta
capacidad estable por sí sola. Una futura modificación debería añadir una
sección de permiso estrecho, con estas condiciones acumulativas:

1. Permite solo una lectura explícitamente solicitada de
   `/sys/firmware/acpi/platform_profile` y
   `/sys/firmware/acpi/platform_profile_choices`, incluida la segunda lectura
   de opciones para coherencia.
2. Requiere modelo, BIOS, kernel y distribución identificados en el registro;
   si falta alguno, la operación queda no validada y requiere revisión.
3. Exige proveedor de solo lectura confinado, nombres internos fijos,
   `openat2` sin fallback, límite de 4096 bytes y sin `sudo`.
4. Prohíbe rutas configurables, ejecución automática, servicios,
   monitorización, reintentos automáticos y cualquier escritura.
5. Obliga a registrar comando, procedencia, resultado o `errno`, fecha y
   cualquier error; un resultado fallido no se reutiliza como datos válidos.
6. Declara expresamente que este permiso no cubre otros nodos sysfs, WMI,
   ACPI, EC, ventiladores, potencia, firmware ni cambios de perfil.

Hasta que ese cambio sea aprobado, la autorización de nuevas lecturas reales
debe seguir siendo puntual y explícita, como ocurrió con el primer hito.

## Secuencia de implementación gradual propuesta

| Fase | Cambio permitido | Barrera de seguridad |
| --- | --- | --- |
| 5C.1 (actual) | Diseño y registro de evidencia. | Sin cambios funcionales ni lecturas. |
| 5C.2 (actual) | Interfaz `read_once()` y pruebas exclusivamente simuladas. | Sin proveedor real nuevo ni dependencia de `legion-cli` sobre sysfs. |
| 5C.3 (actual) | Integrar `legion-cli platform-profile` con la consulta compartida y escenarios embebidos. | Solo `StaticSimulation`; la CLI no puede construir el proveedor sysfs ni seleccionar rutas. |
| 5C.4 | Proponer integración explícita de CLI/GUI y cambio estrecho de `AGENTS.md`. | Revisión de diff, autorización independiente y sin escritura. |
| 5C.5 (actual) | Preparar un binario CLI experimental condicionado por feature y una puerta de compatibilidad estática cerrada. | Sin ejecución real; la puerta no detecta el host ni verifica DMI. |
| 5C.5B | Validación manual autorizada por entorno y decisión de estabilidad. | Cada lectura real se aprueba por separado hasta que se cambien las reglas. |
| 5C.6 | Declaración de soporte o conservación como experimental. | Solo tras satisfacer criterios de aceptación y revisión de seguridad. |

## Riesgos que permanecen

- sysfs puede cambiar semántica, permisos, tipo de objeto o topología entre
  kernels y distribuciones.
- La ruta puede fallar aunque exista; el fallo debe ser informativo y cerrado.
- Una lectura correcta no permite inferir efectos de control, consumo, estado
  térmico, RPM ni seguridad de métodos de firmware.
- La coherencia observada no equivale a una instantánea atómica.
- Una UI atractiva puede ocultar la distinción entre simulación, lectura real,
  dato no validado y error; el modelo de procedencia debe conservarse en todas
  las presentaciones.
