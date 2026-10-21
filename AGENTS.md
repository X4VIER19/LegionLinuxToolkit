# LegionLinuxToolkit: reglas para colaboradores y agentes

## Alcance actual

El único equipo objetivo inicial es una **Lenovo Legion Slim 5 16ARP9 (83EX)**
con BIOS `NUCN15WW`. El objetivo de esta fase es observar, documentar y diseñar;
no cambiar el estado del equipo.

El equipo dispone de Windows 11 y Nobara Linux en discos independientes. Windows
11, con Lenovo Legion Toolkit (LLT) funcional, es un entorno de referencia
técnica para comparar observaciones de firmware; no es una implementación ni
una garantía de compatibilidad para Linux.

## Regla de seguridad principal

Trata toda interfaz de firmware, EC, ACPI, WMI y sysfs de este proyecto como
potencialmente peligrosa. Un identificador WMI compartido no demuestra que un
método sea compatible entre equipos ni versiones de BIOS.

## Categorías de operaciones

### Operaciones pasivas de lectura

Una operación pasiva solo lee archivos o metadatos locales cuyo acceso conocido
no ejecute código de firmware. No incluye la invocación de un método aunque el
método se denomine «Get», «Read» o equivalente.

### Invocaciones activas de firmware

Una invocación activa llama un método real ACPI, WMI o EC. Puede producir efectos
o depender de estado aunque se espere una respuesta de lectura; por ello siempre
requiere autorización explícita previa.

### Operaciones de escritura

Una escritura cambia cualquier configuración o estado de sysfs de control,
firmware, WMI, ACPI, EC, perfil de energía o ventilador. Requiere autorización
explícita y, durante la fase experimental inicial, permanece prohibida cuando
afecte los elementos enumerados más abajo.

### Permitido sin autorización adicional

- Leer código fuente y documentación.
- Consultar metadatos locales y archivos sysfs ordinarios de solo lectura,
  cuando se conozca que su acceso no invoca métodos activos del firmware.
- Crear y editar archivos dentro del repositorio.
- Analizar datos simulados.

### Requiere autorización explícita previa

- Invocar cualquier método ACPI, WMI o EC real, incluso uno denominado de
  lectura.
- Compilar o ejecutar pruebas que interactúen con hardware real.
- Instalar dependencias.
- Cambiar perfiles de energía.
- Cargar, descargar, vincular o desvincular módulos del kernel.
- Modificar archivos o configuraciones fuera del repositorio.
- Crear commits o realizar `push`.
- Cambiar temporalmente un perfil o una curva mediante la interfaz de LLT con
  el fin de una investigación comparativa.

### Prohibido durante la fase experimental inicial

- Invocar `Fan_Set_Table`.
- Escribir curvas o velocidades de ventilación.
- Escribir registros del EC, WMI, firmware o cualquier interfaz de control de
  sysfs.
- Cargar, descargar, vincular o desvincular módulos del kernel sin autorización
  explícita.
- Cambiar perfiles de energía sin autorización explícita.
- Forzar capacidades que el kernel declare incompatibles.
- Desactivar protecciones térmicas.
- Ejecutar pruebas de estrés térmico sin un protocolo separado y aprobado.

No se permite cargar, descargar, vincular ni desvincular módulos experimentales
durante la fase experimental inicial. Una autorización general para desarrollar
el proyecto no levanta esta prohibición. Cualquier excepción futura exige una
transición de fase explícita y una aprobación específica para el módulo y la
acción concretos.

## Puerta obligatoria para operaciones activas futuras

Antes de cualquier acción que pueda cambiar estado, el colaborador debe:

1. Identificar exactamente la interfaz, el método, los argumentos y el modelo
   de BIOS afectados.
2. Explicar el riesgo y el resultado esperado al propietario.
3. Solicitar y recibir autorización explícita para esa acción concreta.
4. Confirmar de forma específica la operación con el usuario.
5. Registrar el resultado, incluyendo método, parámetros, fecha, identidad del
   equipo y estado observado.
6. Tener listo un procedimiento de interrupción y recuperación.

La autorización no elimina las prohibiciones de la fase experimental inicial.

## Uso del entorno Windows de referencia

- Priorizar PowerShell, WMI y herramientas de diagnóstico de solo lectura.
- Antes de proponer una comprobación adicional, indicar los comandos exactos,
  su propósito, los datos esperados y si invocan firmware.
- No repetir una investigación ya documentada salvo que exista una razón técnica
  concreta, registrada junto con la nueva comprobación.
- Registrar cada resultado como observación de Windows, separada de toda
  inferencia sobre Linux.
- No modificar mediante scripts registros EC, firmware ni estructuras WMI.
- No cambiar perfiles ni curvas desde scripts experimentales.
- No usar APIs o herramientas Windows en Linux como sustituto de un controlador
  compatible.
- Si una comparación exige cambiar temporalmente un perfil desde LLT, explicar
  antes el objetivo, el cambio y el efecto esperado, y obtener aprobación.

`Fan_Set_Table` sigue prohibido incluso en Windows. Una operación que funcione
en Windows no autoriza ni valida la misma operación en Linux.

## Detección pasiva obligatoria

Ningún callback de un controlador futuro —incluidos `probe()`, `init()`,
`remove()` o `shutdown()`— puede invocar métodos WMI, ACPI o EC sin un diseño
específico, una revisión de seguridad y una aprobación explícita previos. La
detección de hardware debe ser completamente pasiva: enumerar presencia y
metadatos no es permiso para ejecutar un método de firmware.

## Principios de implementación

- Lectura primero: las capacidades de mutación deben quedar deshabilitadas por
  defecto y físicamente separadas de los adaptadores de lectura.
- Preferir las interfaces estándar del kernel cuando existan.
- No desarrollar un controlador propio mientras no se haya documentado que los
  controladores e interfaces existentes del kernel son insuficientes para la
  capacidad concreta.
- Declarar una capacidad como soportada solamente después de validarla en el
  modelo, BIOS y entorno documentados.
- Validar una capacidad por **modelo, identificador DMI, versión de BIOS,
  interfaz WMI, operación específica y, cuando corresponda, versión de
  kernel**. No extrapolar por familia o GUID.
- No copiar código de terceros sin revisar primero su licencia y atribución.
- No crear commits ni hacer `push` sin autorización explícita.
