# Esqueleto experimental del controlador WMI de ventiladores Lenovo

Este directorio contiene código de preparación aislado para una futura lectura
de `Fan_Get_Table` en la Lenovo Legion Slim 5 16ARP9 (83EX), BIOS `NUCN15WW`.
No forma parte del espacio de trabajo Rust ni se incluye desde ningún árbol de
compilación del kernel. La Fase 6O validó que puede compilarse de forma aislada
contra las cabeceras de Nobara, pero no debe cargarse ni registrarse en esta
fase.

## Estado de seguridad

El archivo C **no registra** un `wmi_driver`, no emite un alias WMI para carga
automática y no declara `probe`, eliminación ni apagado de WMI. Sus funciones
de inicialización y salida gestionan exclusivamente el ciclo de vida del
`miscdevice` experimental. La única función de lectura devuelve
`-EOPNOTSUPP` antes de poder invocar una API WMI. Por tanto, este módulo no
evalúa métodos ACPI/WMI, ni al cargarse ni por ningún otro camino ejecutable.

No existe API de usuario, atributo sysfs, interfaz de escritura, control de RPM
ni soporte para `Fan_Set_Table`.

## Contrato propuesto con espacio de usuario

`legion_fan_read_contract.h` define la versión 1 de un posible contrato de
**solo lectura**, compartido conceptualmente con el adaptador Rust. Describe
una respuesta de versión, tamaño de cabecera, clasificación de estado, tamaño
de carga y 88 bytes opacos de `FAT2`. Los estados distinguen: no compatible,
fallo de transporte, respuesta ACPI inválida y ABI inválido. Ninguno es una
lectura válida y ninguno contiene una interpretación física de los valores.

El encabezado no crea sysfs, procfs, debugfs, dispositivos de caracteres ni
otra API ejecutable. Si una fase futura autorizase una interfaz concreta, esta
deberá conservar el versionado, el límite exacto y la separación de errores,
sin añadir operaciones de escritura.

La Fase 6R prepara el ciclo de vida de un `miscdevice` binario llamado
`legion-fan-read-experimental`. Tiene únicamente una operación `read`; no tiene
escrituras ni `ioctl`. Si alguien lo cargara en una fase futura autorizada, el
nodo se registraría y cada lectura devolvería exclusivamente `-EOPNOTSUPP`.
No entrega datos FAT2, no controla ventiladores y no accede a WMI. Las
respuestas sintéticas solo existen en fixtures Rust y se etiquetan como
simulación. Este repositorio no instala el módulo ni habilita carga automática.

## Compilación aislada validada

La compilación se validó una vez, en una copia temporal del directorio, contra
las cabeceras de Nobara `7.2.11-200.nobara.fc44.x86_64`. Kbuild generó el
módulo sin errores de compilación. El árbol del repositorio no recibió binarios
ni artefactos de Kbuild, y el módulo no se cargó, registró ni ejecutó.

Los avisos de BTF derivados de que el entorno de compilación no dispone de un
`vmlinux` compatible o de la misma versión de `pahole` no modifican el código
ni habilitan el módulo. Una futura compilación debe continuar siendo aislada y
no autoriza carga ni prueba contra el firmware.

## Contrato confirmado que se conserva

La evidencia estática de firmware para este equipo confirma:

- GUID WMI: `92549549-4BDE-4F06-AC04-CE8BF898DBAA`.
- Instancia observada: `0`.
- `Fan_Get_Table`: método `5`.
- Solicitud AML: exactamente dos bytes, `[FanID, SensorID]`.
- Respuesta `FAT2`: 88 bytes, con dos contadores `u32` y dos secuencias de diez
  palabras `u32` en el orden representado por el esqueleto.

El módulo conserva esas palabras como datos opacos. No identifica temperaturas,
RPM ni otras magnitudes físicas, y no presupone que el contrato sea aplicable a
otro modelo o BIOS.

## Contrato futuro, solo lectura

Una fase posterior, revisada y autorizada de forma independiente, podría crear
un controlador WMI registrado que expusiera **una sola instantánea opaca de 88
bytes**, junto con estado de transporte y procedencia. La integración con
LegionLinuxToolkit tendría que consumir únicamente esa instantánea de lectura;
no se permitirá ninguna operación de escritura ni control de ventiladores.

Antes de esa fase harían falta, como mínimo: confirmación de la API WMI del
kernel objetivo, revisión del ciclo de vida del controlador, una barrera de
activación explícita, límites de búfer, registro de la operación, pruebas
aisladas y autorización específica para vincular y realizar una única consulta
real. La futura llamada debe usar únicamente el método `5`; este repositorio no
contiene una implementación de dicha llamada.

## Licencia y procedencia

El Rust del proyecto conserva su licencia dual MIT o Apache-2.0. Este código de
módulo potencial usa interfaces WMI del kernel y se publica separadamente como
`GPL-2.0-only`. Es código original de LegionLinuxToolkit; no copia código de
LenovoLegionLinux ni de sus decodificadores GPL.
