# Módulos

Cada archivo `.itd` es un módulo. `module tienda.pagos;` debe coincidir con su ruta relativa `tienda/pagos.itd` desde la raíz del proyecto. `use tienda.pagos;` importa sus nombres públicos; `use tienda.pagos as pagos;` importa un alias. `export` hace público el elemento que precede.

`from` queda reservado y no forma parte de la sintaxis 0.1; `use` cubre los imports del MVP.

La resolución es estática, relativa a una única raíz canonizada. Una ruta que salga de esa raíz es error. En 0.1 no hay paquetes remotos, imports con comodín ni ciclos entre módulos.
