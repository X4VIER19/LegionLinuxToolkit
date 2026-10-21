# Adaptador simulado de `platform_profile` (Fase 5B.2)

Esta etapa implementa exclusivamente un parser en memoria para el formato de
texto documentado de `platform_profile`. No abre `/sys`, no recibe rutas, no
lee archivos del host y no puede cambiar un perfil energético.

## Contrato del adaptador

`legion-core::PlatformProfileAdapter` recibe un `PlatformProfileInput` con dos
cadenas opcionales ya presentes en memoria:

- `raw.active`: texto original del perfil activo;
- `raw.choices`: texto original de la lista de perfiles disponibles.

`None` significa que el dato no fue suministrado. Es diferente de `Some("")`,
que se rechaza como una respuesta vacía y malformada. El resultado conserva
ambos textos exactos en `raw` y expone el resultado sin espacios ni saltos de
línea superfluos en `normalized`.

Cuando ambos datos existen, el adaptador exige que el perfil activo aparezca
una sola vez en la lista normalizada. Las entradas duplicadas, el perfil activo
ausente de la lista y los nombres con una estructura inválida se rechazan con
errores diferenciados. Si se recibe un nombre con formato válido pero fuera del
vocabulario conocido, se conserva como `unknown` y el resultado pasa a estado
`unvalidated`; no se descarta ni se trata como una capacidad de control.

La lista de vocabulario conocida sirve solo para describir datos estáticos. No
es una lista de autorización y no habilita operaciones sobre el hardware.

## Fixtures y CLI

`legion-fixtures` es la única fuente de la Fase 5B.2. Sus escenarios cerrados
incluyen `valid`, `active-only`, `missing-active`, `empty-active`,
`empty-choices`, `unknown`, `duplicate`, `inconsistent` y `malformed`. No se
aceptan rutas, nombres de archivo ni valores del entorno como fuentes.

La consulta se muestra con:

```text
legion-cli platform-profile --scenario valid --format json
```

La salida JSON usa el envoltorio `source: "simulated"`; además, la procedencia
interna tiene `artifact: "static-simulation"`. La salida de texto presenta por
separado la entrada bruta y los valores normalizados. Ninguno de ambos formatos
es una lectura del equipo.

## Límite para una integración Linux futura

Una capa específica de Linux, aún inexistente, podría obtener texto de una
interfaz aprobada y pasarlo al adaptador. Esa capa deberá diseñarse y revisarse
por separado: tendrá que usar descubrimiento controlado, identificar la ruta
y la procedencia, tratar errores de lectura sin sustituciones y permanecer
estrictamente separada del parser de `legion-core`. Esta etapa no constituye
autorización para implementarla ni para leer el host.
