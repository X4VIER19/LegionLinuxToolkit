# Fase 4A.1: contrato técnico de `Fan_Get_Table`

**Alcance:** análisis estático y documentación pública. No se invocó WMI, ACPI
ni EC; no se leyó ninguna interfaz del equipo, no se cargaron módulos y no se
propone un comando experimental. El único objetivo de las conclusiones sobre
el equipo es la Lenovo Legion Slim 5 16ARP9 (`83EX`), BIOS `NUCN15WW`.

## Resultado

No hay evidencia suficiente para diseñar ni autorizar una llamada real segura
a `Fan_Get_Table` en Linux. Se conoce una parte de la enumeración WMI y una
observación funcional en Windows, pero todavía falta el contrato específico del
método para este modelo: identificador, representación de entrada, contrato de
salida y semántica. Los identificadores y estructuras de otros Legion se tratan
solamente como hipótesis de investigación.

## Matriz de hechos, hipótesis y datos desconocidos

| Conclusión | Clasificación | Fuente y evidencia concreta | Confianza y aplicabilidad | Limitación |
| --- | --- | --- | --- | --- |
| El bloque WMI del equipo tiene GUID `92549549-4BDE-4F06-AC04-CE8BF898DBAA`, tipo `method`, una instancia y `object_id=AB`. | Hecho observado | Inventario Linux previo documentado en [observaciones Windows/WMI](windows-wmi-observations.md). | Alta para la enumeración de este 83EX en el entorno observado. | No revela el ABI, los métodos permitidos ni sus efectos. |
| `WMAB` es el nombre ACPI interno candidato del bloque. | Derivación documentada | El núcleo compone el nombre de un bloque de método como `W` + `M` + los dos bytes de `object_id` en [`core.c`](https://github.com/torvalds/linux/blob/master/drivers/platform/wmi/core.c). Con `AB`, el resultado es `WMAB`. | Alta para la regla genérica de composición; media para el nombre candidato de este bloque. | No establece que `WMAB` implemente `Fan_Get_Table`, ni el ID, argumentos o respuesta de esa operación. |
| `Fan_Get_Table` existe y funcionó en Windows con `FanID=1`, `SensorID=4`, ambos `UInt8`. | Hecho observado | Registro previo de Windows en [observaciones Windows/WMI](windows-wmi-observations.md): se obtuvo una respuesta de dos tablas de diez valores. | Alta para esa llamada Windows y esos parámetros observados. | No se capturó un buffer WMI/ACPI Linux, ni se validó su significado, persistencia o equivalencia entre plataformas. |
| El identificador numérico `5` procede de LenovoLegionLinux. | Hecho de procedencia, no del 83EX | Su archivo [`legion-laptop.c`](https://github.com/johnfanv2/LenovoLegionLinux/blob/main/kernel_module/legion-laptop.c) declara `WMI_METHOD_ID_FAN_GET_TABLE` como `5` y lo pasa a su wrapper de `wmi_evaluate_method()`. | Alta sobre el origen de la hipótesis en ese código externo. | No es documentación del firmware del 83EX; la búsqueda dirigida no halló una inclusión positiva de `83EX`/`NUCN15WW` en su lista pública de modelos. La ausencia no prueba incompatibilidad. |
| El ID `5` es válido para `Fan_Get_Table` en 83EX/NUCN15WW. | Hipótesis no validada | No existe BMOF, documentación de Lenovo ni observación Linux del 83EX que lo confirme. | Baja. No se usa como constante del proyecto ni como parámetro propuesto. | LenovoLegionLinux contiene variantes por modelo/BIOS y otras familias Legion muestran GUID o formatos diferentes; compartir nombre o familia no permite extrapolar. |
| Los `UInt8` de Windows definen dos bytes consecutivos de entrada en Linux. | Desconocido | La documentación Windows define invocación CIM con parámetros tipados y con nombre; la API WMI del kernel entrega un identificador numérico y buffers específicos del dispositivo. | Ninguna. | El orden, empaquetado, alineación, cabecera y tipo ACPI del buffer siguen sin evidencia para el 83EX. |
| Una salida con longitud suficiente valida el contenido o el protocolo. | Falso | La [guía de controladores WMI](https://www.kernel.org/doc/html/latest/wmi/driver-development-guide.html) asigna al controlador el análisis y validación del buffer; la API solo permite exigir un tamaño mínimo y alinea buffers. | Alta, como regla genérica del kernel. | Los límites de longitud no verifican IDs, campos, unidades ni semántica de las tablas. |
| Existe una interfaz WMI de espacio de usuario en Nobara que permita llamar a este método. | Desconocido | La interfaz pública del kernel describe un `wmi_driver`/`wmi_device` y `wmidev_invoke_method()` para controladores; no se identificó una ruta de usuario validada para este equipo. | Ninguna. | Los nodos sysfs observados enumeran el dispositivo; no prueban que admitan invocación de métodos. |

## Windows y Linux: contratos distintos

En Windows, `Fan_Get_Table` se observó mediante una clase WMI/CIM con nombre y
parámetros `UInt8`. La especificación de WMI de Microsoft define las entradas y
salidas de un método como objetos de parámetros separados; las herramientas
CIM resuelven los nombres y tipos a partir del esquema de la clase
([MS-WMIO](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmio/30de0d74-8e47-4a83-a06e-180991079c38),
[Invoke-CimMethod](https://learn.microsoft.com/en-us/powershell/module/cimcmdlets/invoke-cimmethod)).

En Linux, la interfaz moderna documentada es un controlador enlazado a
`wmi_device` que llama `wmidev_invoke_method()` con instancia, ID numérico y
buffers de bytes. El propio controlador debe interpretar el Binary MOF (BMOF),
analizar la respuesta y validar sus campos ([API WMI del
kernel](https://www.kernel.org/doc/html/latest/driver-api/wmi.html), [guía de
desarrollo](https://www.kernel.org/doc/html/latest/wmi/driver-development-guide.html)).
Por tanto, una llamada de PowerShell exitosa no determina el ID ni el
empaquetado que usaría una implementación Linux.

`wmidev_evaluate_method()` es una API histórica del kernel, documentada como
obsoleta. Internamente termina evaluando el objeto ACPI compuesto a partir del
bloque WMI; sigue siendo una invocación activa de firmware. No es una API de
usuario, ni una vía autorizada para este proyecto.

## Parámetros y representación binaria

| Elemento | Conocido | Desconocido |
| --- | --- | --- |
| `FanID` | Nombre y valor observado `1`; tipo CIM/Windows `UInt8`. | Si se codifica como un campo de un buffer Linux, su offset, orden y reglas de rango. |
| `SensorID` | Nombre y valor observado `4`; tipo CIM/Windows `UInt8`. | Si se codifica junto a `FanID`, con relleno, cabecera u otra representación ACPI. |
| Entrada Linux | Debe tener una forma definida por el dispositivo antes de llamar a una API del kernel. | Tamaño exacto, endianidad, alineación semántica y si los parámetros Windows se trasladan directamente. |
| Salida | Windows devolvió dos tablas de diez valores en la observación registrada. | Tipo ACPI/WMI, longitud exacta, estructura, unidades, significado de los índices y si describe la curva activa. |

La documentación del kernel identifica el BMOF como la metadata que describe
los IDs y layouts WMI. El controlador `wmi-bmof` obtiene esos datos mediante
una consulta de bloque WMI; realizarla sería una invocación activa y puede
requerir un módulo. Por las reglas de este repositorio no se realizó ni se
propone como acción de la fase actual.

## Información imprescindible antes de una prueba real futura

Antes de diseñar una operación de lectura, haría falta evidencia específica del
83EX/`NUCN15WW` para todos los puntos siguientes:

1. La identidad completa del bloque y la presencia de la instancia en el kernel
   objetivo, sin usar campos DMI ausentes como comodines.
2. El BMOF u otra documentación primaria del fabricante que asocie
   inequívocamente `Fan_Get_Table` con su ID de método para este BIOS.
3. La estructura de entrada: tipo ACPI, orden de campos, longitudes,
   alineación, rangos válidos y significado de `FanID`/`SensorID`.
4. La estructura de salida: tipo, longitud mínima y máxima, versión, todos los
   campos, unidades y condiciones de error.
5. Evidencia de que la consulta no cambia perfil, curva, estado térmico ni
   persistencia; el nombre «Get» no es suficiente.
6. Una vía Linux revisada que se limite a una acción manual explícita y valide
   modelo, DMI, BIOS, interfaz, operación y versión de kernel. Debe fallar de
   forma segura ante cualquier discrepancia.
7. Un protocolo posterior aprobado: límites de buffer, registro de parámetros
   y resultado, supervisión, criterio de interrupción y recuperación. La
   autorización tendría que ser nueva y referirse al método exacto.

## Alternativas sin controlador experimental

| Alternativa | Evidencia disponible | Estado para 83EX/NUCN15WW |
| --- | --- | --- |
| HWMON de `lenovo_wmi_other` | El código del kernel puede exponer `fanX_input` cuando Capability Data declara canales y límites RPM válidos. | No disponible como telemetría validada: el kernel observado informó que el reporte/ajuste de ventilador no es compatible. No se deben relajar sus controles. |
| `acpi_fan` y `yogafan` | Las observaciones anteriores devolvieron `0 RPM`. | No concluyente: cero es un valor numérico, no una prueba de ventilador detenido ni de sensor fiable. |
| Curvas mediante interfaces existentes | `lenovo_wmi_gamezone` gestiona perfiles, pero no documenta una lectura de la curva de este bloque para el 83EX. | Ninguna ruta pasiva y validada para leer curvas. Cambiar perfiles o curvas está prohibido. |
| Módulos externos, EC o ACPI directo | Proyectos externos contienen vías específicas de modelo. | No son alternativa: no están validados para 83EX y entrarían en las prohibiciones de módulos e invocaciones activas. |

## Recomendación

Mantener la Fase 4B bloqueada para pruebas reales. La evidencia permite
documentar y simular una abstracción, pero no fijar un ID, construir un buffer
ni interpretar una respuesta del 83EX. El siguiente trabajo seguro continúa
siendo documental: localizar documentación primaria o metadata específica del
modelo sin ejecutar métodos, y revisar la versión exacta de las fuentes del
kernel de Nobara. Cualquier propuesta de experimento debe presentarse aparte;
este informe no contiene ni autoriza un comando ejecutable.

## Fuentes públicas y alcance de la revisión

- [API WMI del kernel Linux](https://www.kernel.org/doc/html/latest/driver-api/wmi.html)
- [Guía de desarrollo de controladores WMI](https://www.kernel.org/doc/html/latest/wmi/driver-development-guide.html)
- [WMI Binary MOF](https://docs.kernel.org/wmi/devices/wmi-bmof.html)
- [Implementación ACPI-WMI `core.c`](https://github.com/torvalds/linux/blob/master/drivers/platform/wmi/core.c)
- [LenovoLegionLinux: `legion-laptop.c`](https://github.com/johnfanv2/LenovoLegionLinux/blob/main/kernel_module/legion-laptop.c)
- [LenovoLegionLinux: lista de funciones y pruebas](https://github.com/johnfanv2/LenovoLegionLinux/blob/main/doc/FEATURES_AND_TESTING.md)
- [Especificación Microsoft MS-WMIO](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmio/30de0d74-8e47-4a83-a06e-180991079c38)
- [Microsoft `Invoke-CimMethod`](https://learn.microsoft.com/en-us/powershell/module/cimcmdlets/invoke-cimmethod)

Se inspeccionaron los fragmentos concretos indicados, no los repositorios
externos completos. No se ha copiado código. El `SPDX` de
`legion-laptop.c` declara `GPL-2.0-or-later`; esa comprobación de un archivo no
equivale a una auditoría de la licencia global del proyecto externo.
