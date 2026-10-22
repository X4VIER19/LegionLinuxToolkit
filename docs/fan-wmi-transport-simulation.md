# Adaptador simulado de tablas de ventilador sobre WMI

`legion-fan` conecta `FanTableWmiAdapter` con el trait inyectado `WmiTransport`
solo para simulaciones. El adaptador conserva el búfer de respuesta como bytes
opacos y no intenta inferir su ABI. La interpretación exige por separado un
sobre de evidencia estructurada; un búfer de tamaño coherente no valida su
formato, sus unidades ni la semántica de una curva.

Los errores permanecen separados: transporte WMI, evidencia insuficiente y
validación de interpretación. Las RPM de una curva se representan únicamente
como consignas; no son RPM medidas.

Antes de una consulta real faltan, como mínimo, el ID de método específico de
83EX/`NUCN15WW`, ABI de entrada y salida, límites de búfer, semántica de cada
campo y una vía Linux revisada y autorizada. No existe `Fan_Set_Table` ni una
operación de escritura en este adaptador.
