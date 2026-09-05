# Modelo de objetos

Una `class` define campos, métodos y un constructor `fn this(...)`. Las instancias tienen identidad por referencia; `this` está disponible solo dentro de miembros de instancia. Los campos requieren tipo y valor inicial, y son privados por defecto. `pub`, `priv` y `protected` controlan acceso; `protected` se reserva hasta que exista herencia y se trata como privado en 0.1.

`static` pertenece a la clase y no recibe `this`. Los traits declaran firmas y no contienen estado. Una clase declara los traits que implementa después de `:`, por ejemplo `class Counter: Printable, Resettable { ... }`. El MVP favorece composición y traits. Herencia de clases, sobrecarga, reflexión y herencia múltiple quedan fuera de 0.1.
