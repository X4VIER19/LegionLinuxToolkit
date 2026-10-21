# Interfaces conocidas y estado de confianza

| Interfaz | Uso previsto | Estado en 83EX / NUCN15WW | Nivel |
| --- | --- | --- | --- |
| `platform_profile` | Leer perfil activo y opciones | Observada | Confirmada, lectura pasiva |
| `lenovo_wmi_other` firmware attributes | Leer límites y valores por perfil | Observada | Confirmada, lectura pasiva |
| hwmon `acpi_fan` / `yogafan` | RPM | 0 RPM observado | No concluyente |
| WMI `92549549-...` | Tabla de ventilador | Presente, sin driver | Descubierta |
| `Fan_Get_Table` | Leer tablas WMI | Funcional en Windows | No validada en Linux; requiere autorización |
| `Fan_Set_Table` | Escribir tablas WMI | No probada | Prohibida |

## Referencia al kernel

`lenovo-wmi-gamezone` ofrece perfiles de plataforma y el modo `custom`; según
la documentación del kernel, los cambios de Other Mode requieren que el perfil
custom se seleccione manualmente para tener efecto. `lenovo-wmi-other` expone
límites de potencia a través de firmware attributes y puede publicar ventilador
en hwmon cuando el firmware anuncia soporte. La falta de esa capacidad en este
equipo es coherente con el mensaje de kernel observado, pero no explica aún las
lecturas 0 RPM de otros dispositivos.

## Distinción obligatoria

- **Temperatura actual**: una medición de sensor, con unidad y marca de tiempo.
- **Límite térmico**: una política configurable de firmware.
- **`current_value` de límite de potencia**: el límite configurado vigente; no
  representa consumo eléctrico real.
- **RPM reportado**: telemetría de una fuente concreta; cero no implica por sí
  solo ventilador detenido.
- **Tabla de curva**: configuración de firmware cuyo significado debe validarse
  para el modelo y BIOS exactos.

Los nombres, identificadores y formatos de métodos WMI no se consideran
compatibles entre modelos sin validación específica.

## Hechos, hipótesis y capacidades no validadas

**Hechos comprobados:** perfiles y atributos sysfs enumerados en Linux; el
dispositivo WMI listado; el mensaje del kernel sobre soporte de ventilador; y
las respuestas de `Fan_Get_Table` observadas en Windows.

**Hipótesis:** que el método, sus argumentos, buffers o tablas de Windows
tengan la misma semántica bajo Linux; y que las lecturas 0 RPM describan el
estado físico de los ventiladores.

**Capacidades no validadas:** lectura WMI desde Linux, telemetría RPM fiable y
cualquier modificación de perfiles, límites o curvas. Ninguna se habilita por
la presencia de un GUID o por resultados obtenidos en otro sistema operativo.
