# LegionLinuxToolkit

Aplicación de código abierto, orientada a Linux, para observar y administrar
hardware de portátiles Lenovo Legion. El desarrollo comienza de forma
conservadora con una sola máquina y privilegia las interfaces que ya ofrece el
kernel.

## Equipo objetivo inicial

| Componente | Valor |
| --- | --- |
| Modelo | Lenovo Legion Slim 5 16ARP9 (83EX) |
| BIOS | NUCN15WW |
| Placa | LNVNB161216 |
| CPU | AMD Ryzen 7 7435HS |
| GPU | NVIDIA RTX 4060 Laptop |
| Entorno de desarrollo | Nobara, kernel 7.2.0-202.nobara.fc44.x86_64 |

La compatibilidad funcional inicial se limita al 83EX con BIOS `NUCN15WW` y el
entorno documentado. El diseño busca distribuirse en Nobara, CachyOS, Linux
Mint y otras distribuciones, pero aún no afirma compatibilidad con otros
equipos, revisiones de BIOS ni distribuciones.

## Entorno de referencia Windows

El equipo tiene dual boot: Windows 11 y Nobara Linux residen en discos
independientes. En Windows funciona Lenovo Legion Toolkit (LLT), incluyendo
perfiles de energía y curvas de ventilador en el perfil Personalizado. Este
entorno servirá para obtener observaciones comparativas seguras mediante WMI,
PowerShell y diagnósticos. No implica que una llamada, estructura o resultado
de Windows sea compatible con Linux.

Las comparaciones se limitarán inicialmente a consultas de solo lectura. Todo
cambio temporal desde LLT necesita autorización explícita; `Fan_Set_Table`,
escrituras de EC/firmware/WMI y scripts que cambien perfiles o curvas siguen
fuera de alcance. Cualquier comprobación nueva se propondrá antes con comandos,
propósito y riesgo; no se repetirá una investigación ya documentada sin una
razón técnica concreta.

## Estado: experimental y de solo lectura

La primera fase documental registra las interfaces observadas para inventario y
monitorización futura:

- perfiles expuestos por `/sys/firmware/acpi/platform_profile`;
- atributos de potencia y límites térmicos expuestos por
  `lenovo_wmi_other`;
- detección pasiva de las interfaces de ventilación disponibles.

El 2026-10-21, una única ejecución experimental autorizada leyó de forma
coherente el perfil `performance` y las opciones
`low-power balanced performance max-power custom` desde las dos interfaces
fijas de `platform_profile`. El resultado quedó marcado como
`linux-read-only` con confianza baja. Esto valida únicamente esa lectura en el
83EX/`NUCN15WW`/Nobara documentado: no valida controles, escritura, futuros
accesos, ventiladores, WMI, ACPI o EC. La feature experimental continúa
desactivada por defecto y no está conectada a `legion-cli`.

El soporte de ventiladores de este modelo no está demostrado: `acpi_fan` y
`yogafan` informaron 0 RPM en las pruebas, y el kernel registró que el informe
y ajuste de ventilador no están soportados. Las lecturas de 0 RPM **no** se
interpretan todavía como ausencia de giro.

La primera aplicación se desarrollará inicialmente solo contra árboles sysfs y
respuestas WMI simulados; sus pruebas automatizadas no podrán acceder al
firmware ni a rutas de hardware reales. No se modifican curvas, velocidades,
perfiles de energía ni parámetros de firmware. Tampoco se invoca
`Fan_Set_Table` ni se ejecutan métodos WMI al detectar el dispositivo. Consulta
[las reglas de seguridad](AGENTS.md) antes de contribuir.

## Arquitectura propuesta

El proyecto se organizará por capas y capacidades:

```text
CLI/GUI futura
    │
API de dominio inmutable ── inventario, perfiles, límites, ventiladores
    │
Adaptadores de solo lectura ── sysfs / hwmon / WMI (opt-in)
    │
Kernel y firmware
```

## Fases 1–3: Rust, compatibilidad y WMI simulado

El workspace contiene `legion-core` (modelos y parser puro), `legion-cli`
(salida legible y JSON) y `legion-fixtures` (snapshots embebidos). No usa
adaptadores del host ni lee rutas del host; las entradas del CLI son fixtures
incluidas y las pruebas rechazan nombres de rutas como `/sys`.

Cada adaptador devuelve datos tipados junto con procedencia, unidad, marca de
tiempo y nivel de confianza. Las operaciones de escritura serán un componente
independiente, ausente del primer prototipo y bloqueado por una política de
autorización explícita.

La Fase 3 incorpora en `legion-core` un contrato de transporte WMI independiente
de Linux y, en `legion-fixtures`, su única implementación: un transporte de
respuestas sintéticas en memoria. No presupone un ABI binario, identificador de
método ni interfaz WMI funcional desde espacio de usuario en Nobara. El CLI
puede mostrar únicamente esos escenarios simulados, nunca invocar firmware.

La propuesta detallada y el plan están en [docs/architecture.md](docs/architecture.md)
y [docs/development-plan.md](docs/development-plan.md). Los hechos observados
están separados de las hipótesis en [docs/known-interfaces.md](docs/known-interfaces.md).

## Documentación

- [Resultados Linux](docs/linux-observations.md)
- [Resultados Windows y WMI](docs/windows-wmi-observations.md)
- [Interfaces conocidas](docs/known-interfaces.md)
- [Registro de compatibilidad](docs/compatibility-registry.md)
- [Transporte WMI simulado](docs/wmi-transport-simulation.md)
- [Investigación pasiva de WMI](docs/wmi-passive-research.md)
- [Contrato técnico de `Fan_Get_Table`](docs/fan-get-table-contract-research.md)
- [Telemetría y perfiles simulados](docs/telemetry-simulation.md)
- [Investigación de telemetría Linux](docs/linux-telemetry-research.md)
- [Adaptador simulado de `platform_profile`](docs/platform-profile-simulation.md)
- [Adaptador Linux simulado de `platform_profile`](docs/linux-platform-profile-adapter.md)
- [Diseño y primer hito de lectura real de `platform_profile`](docs/linux-platform-profile-first-read-proposal.md)
- [Arquitectura modular](docs/architecture.md)
- [Plan incremental](docs/development-plan.md)
- [Fuentes y licencias](docs/research-sources.md)

## Licencias y referencias

LegionLinuxToolkit se distribuye bajo licencia dual
[MIT](LICENSE-MIT) o [Apache-2.0](LICENSE-APACHE), a elección de quien reciba
el software. Este repositorio no incorpora código de proyectos de terceros en
esta fase.
Las implementaciones externas se estudian como referencias de arquitectura y
compatibilidad; cualquier reutilización futura requiere comprobar su licencia,
atribución y compatibilidad antes de incluirla.
