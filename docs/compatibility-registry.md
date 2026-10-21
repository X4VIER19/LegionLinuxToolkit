# Registro de compatibilidad estático

La Fase 2 implementa un registro declarativo en memoria. No detecta el equipo
actual, no lee DMI, sysfs ni otras rutas del host y no habilita controles de
hardware.

## Claves de identidad

Una coincidencia requiere, sin normalización ambigua, fabricante, modelo, tipo
de máquina y versión de BIOS. Los valores ausentes, vacíos o desconocidos no
son comodines: una consulta con alguno de los cuatro campos base vacío queda
incompleta. El identificador DMI está modelado, pero todavía no se ha
recopilado para el perfil de referencia y por eso se conserva como ausente, no
como un valor supuesto. Una consulta que aporte un DMI no documentado tampoco
coincide con ese perfil. Una consulta incompleta, sin coincidencia exacta o con
más de un perfil coincidente no habilita nada.

## Perfil inicial

El único perfil estático inicial representa Lenovo Legion Slim 5 16ARP9,
tipo `83EX`, BIOS `NUCN15WW`. Sus capacidades incluyen procedencia y estado:

- perfiles de plataforma y atributos de potencia: observados en Linux mediante
  las interfaces documentadas; no se clasifican aún como validados;
- RPM de ventilador: no validado; cero RPM sigue siendo no concluyente;
- `Fan_Get_Table`: observado en Windows, no validado en Linux;
- `Fan_Set_Table`: estado de compatibilidad desconocido y prohibido por la
  política de seguridad;
- reporte de ventilador de `lenovo_wmi_other`: no compatible para esa interfaz,
  según el mensaje del kernel documentado.

Los estados describen evidencia, no permisos. La evidencia registra plataforma,
fuente, tipo (`documentation`, `observation` o `validation`) y estado. Cada
`CapabilityRecord` se construye como descriptivo con `controls_enabled=false`;
el campo no es modificable desde fixtures, CLI ni una coincidencia del registro.

## Criterio para `validated`

Una capacidad solo podrá ser `validated` tras una comprobación repetible para la
combinación exacta de modelo, BIOS, interfaz y operación, con evidencia
registrada del resultado esperado y sin contradecir la política de seguridad.
La mera presencia de una ruta, módulo o GUID, o una observación en Windows, no
cumple este criterio. `validated` tampoco habilita controles de hardware.
