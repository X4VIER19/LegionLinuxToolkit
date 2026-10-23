# Modelo offline de índices de curva GMZN

## Alcance y evidencia

Este documento describe únicamente el comportamiento reconstruido de la DSDT
estática obtenida para el 83EX con BIOS `NUCN15WW`. No es una especificación
general de Lenovo y no habilita escrituras, `SFAN`, `Fan_Set_Table`, EC ni
consultas WMI adicionales. La DSDT y las capturas crudas no se redistribuyen en
este repositorio público.

El recorrido demostrado para el método WMI 5 es:

```text
WMAB(Arg1 == 5, Arg2) -> toma Arg2[0] y Arg2[1] -> GFAN(...) -> FAT2 (88 bytes)
```

Aunque `WMAB` entrega los dos bytes a `GFAN`, el cuerpo estático de `GFAN` no
los usa para seleccionar tablas. FAT2 contiene `FTLE = 10`, diez campos `FTS`,
`FTSL = 10` y diez campos `FSS`: dos arreglos de diez `u32` little-endian.

## Dos rutas de GFAN

- Si `GZ44 == 7`, GFAN copia el arreglo estático `EFST`, cuyos elementos son
  `1..10`, en ambas tablas de FAT2.
- En la otra ruta, si `F10A != 0`, copia los diez valores EC `F101..F10A`
  (reflejados en `CFST`) en ambas tablas. Si `F10A == 0`, llama a `INIF(2)` y
  después devuelve `CFST`. Esa rama AML puede escribir los campos EC; el
  modelo Rust **no** ejecuta ni reproduce tal llamada.

Por tanto, los veinte valores `10` de la única captura FAT2 real son índices
de configuración válidos dentro de la forma demostrada. La captura es
incompatible con la rama `GZ44 == 7` y apunta, de manera estática, a la ruta
que leyó `CFST` con sus diez elementos en 10. No demuestra una velocidad ni
una temperatura.

## Catálogos y transformación demostrada

La DSDT declara cinco catálogos `FNT0..FNT4`, cada uno con 15 paquetes de 35
valores, y `FI00..FI04`, cada uno con 3 paquetes de 22 valores. `SFAN` es la
ruta independiente de escritura/configuración: selecciona paquetes según
campos de modo y coloca índices en `CFST` y EC `F101..F10A`.

Para los campos `CRP`, `GRP` y `ERP`, AML usa expresiones equivalentes a:

```text
Local0[configuration_index + 2] / 100
```

Esto demuestra el desplazamiento `+2` y el divisor matemático `100`; no
demuestra una unidad física ni el significado expandido de CRP, GRP o ERP. AML
no introduce una comprobación de límites visible antes de indexar. El modelo
offline impone de forma más segura el único dominio evidenciado: índice
`1..=10` y, por tanto, offset `3..=12`.

La selección del catálogo FNT y de uno de sus 15 paquetes no aparece en FAT2.
Por eso el intérprete Rust exige `FntProfileSelection` explícito y nunca lo
deduce de una respuesta GFAN.

## Herramienta offline

`legion-fan` expone un parser de texto AML y un intérprete de FAT2. Conserva el
literal AML original (por ejemplo `0x1388`) junto con su `u32`, conserva los
índices FAT2 y etiqueta la interpretación física como `unknown`. No abre
archivos ACPI, no acepta una ruta de firmware y no tiene operaciones de
escritura.

Ejemplo condicional: para la captura formada por índices `10`, una selección
explícita `FNT0`, paquete `0` produce offset `10 + 2 = 12`. Si ese paquete
contiene el literal `0x1388` en dicho offset, el modelo informa literal
`0x1388`, valor `5000` y divisor `100`; no lo llama RPM, °C ni ningún otro
valor físico.

## Límites restantes

- La correspondencia de `FanID`/`SensorID` con un ventilador o sensor físico
  sigue sin demostrarse.
- No se ha validado qué selección FNT/SFAN estaba activa en la captura.
- La semántica física de los valores de FNT/FI, CRP, GRP y ERP requiere otra
  evidencia independiente.
- Nada de lo anterior valida una operación de escritura o `Fan_Set_Table`.
