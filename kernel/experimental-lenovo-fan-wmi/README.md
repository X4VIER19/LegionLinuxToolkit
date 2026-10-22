# Esqueleto experimental del controlador WMI de ventiladores Lenovo

Este directorio contiene código de preparación aislado para una futura lectura
de `Fan_Get_Table` en la Lenovo Legion Slim 5 16ARP9 (83EX), BIOS `NUCN15WW`.
No forma parte del espacio de trabajo Rust, no se incluye desde ningún árbol de
compilación del kernel y no debe compilarse ni cargarse en esta fase.

## Estado de seguridad

El archivo C **no registra** un `wmi_driver`: no declara rutas de
inicialización, `probe`, eliminación o apagado. La única función de lectura es
un marcador deshabilitado que devuelve `-EOPNOTSUPP` antes de poder invocar una
API WMI. Por tanto, este esqueleto no evalúa métodos ACPI/WMI, ni al cargarse ni
por ningún otro camino ejecutable.

No existe API de usuario, atributo sysfs, interfaz de escritura, control de RPM
ni soporte para `Fan_Set_Table`.

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
