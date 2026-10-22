# Adaptador Linux simulado para `platform_profile` (Fase 5B.3)

Esta fase añade `legion-linux`, una biblioteca Linux-específica que organiza la
lectura futura de `platform_profile` sin abrir todavía ninguna ruta del host.
No hay punto de entrada de CLI, lectura de `/sys`, fallback automático ni API
de escritura.

## Capas

```text
SimulatedPlatformProfileTree (bytes en memoria)
        │ solo PlatformProfileEntry::{Active, Choices}
LinuxPlatformProfileReader
        │ UTF-8, tamaño, errores explícitos y procedencia
legion-core::PlatformProfileAdapter
        │ validación del formato y coherencia
LinuxPlatformProfileReading
```

Los únicos nombres relativos definidos son `platform_profile` y
`platform_profile_choices`. No existe una variante para rutas arbitrarias, por
lo que el consumidor no puede solicitar `/sys`, `..`, un dispositivo ni otro
archivo.

## Confinamiento de esta etapa

El workspace prohíbe código `unsafe`. La biblioteca estándar de Rust no ofrece
una API suficiente para abrir una ruta relativa a un descriptor de directorio
con `O_NOFOLLOW` y mantener el confinamiento frente a sustituciones concurrentes
de la raíz o de entradas. `canonicalize()` no sería una defensa suficiente.

Por ello, en lugar de implementar una lectura de archivos incompleta, 5B.3 usa
exclusivamente `SimulatedPlatformProfileTree`: un árbol virtual de bytes en
memoria, con una etiqueta diagnóstica que **no es una ruta**. Este árbol modela
archivos regulares, ausencias, permisos denegados, lecturas parciales, errores
de lectura y enlaces simbólicos. Los enlaces se rechazan antes de aportar bytes;
no pueden seguirse ni escapar del árbol.

La interfaz de proveedor está sellada: desde otro crate no se puede incorporar
un lector de host ni otro proveedor. En 5B.3, el único proveedor instanciable es
el árbol virtual incluido en `legion-linux`.

La futura implementación de sistema de archivos requerirá una revisión
independiente. Deberá utilizar primitivas seguras de descriptor de directorio
con protección contra enlaces simbólicos y recorridos, límites antes de reservar
memoria y pruebas de sustitución. Hasta entonces no se debe añadir un proveedor
basado en rutas reales.

## Validación y errores

`LinuxPlatformProfileReader` solicita ambos nombres constantes, limita cada
respuesta y exige UTF-8 antes de crear `PlatformProfileInput`. Después llama
obligatoriamente a `PlatformProfileAdapter`; una respuesta no puede saltarse la
normalización ni la comprobación de que el perfil activo se encuentre entre las
opciones.

Ausencia, permiso insuficiente, UTF-8 inválido, lectura parcial, tamaño
excesivo, enlace simbólico y fallo de lectura se representan como
`PlatformProfileSourceError`. Una lectura parcial se rechaza antes de procesar
sus bytes, aunque parecieran un perfil válido. Texto vacío, malformado, duplicado
o incoherente llega al parser de `legion-core` y conserva sus errores específicos.

## Procedencia

Las fuentes públicas disponibles solo pueden originar
`LinuxPlatformProfileOrigin::StaticSimulation`; la procedencia interna es
`TelemetryArtifact::StaticSimulation`. La etiqueta `LinuxReadOnly` junto con
`TelemetryArtifact::LinuxReadOnly` está reservada para el proveedor privado
bloqueado de Fase 5B.5B-2. Su capacidad de activación es inhabitable, así que
ni fixtures ni consumidores externos pueden generar esa etiqueta o ejecutar
una lectura real.

Por tanto, los resultados de las observaciones manuales del 83EX —`performance`
y `low-power balanced performance max-power custom`— se usan únicamente como
contenido de fixture de prueba, no como una lectura automática del equipo.
