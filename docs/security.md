# Seguridad y límites

La versión 0.1 procesa como máximo 1 MiB por archivo, 256 módulos y 64 niveles de imports. La VM permite por defecto 1 000 000 instrucciones, 64 llamadas, 10 000 objetos, 1 MiB de salida y 2 segundos. `RuntimeLimits` permite al host reducir estos valores. IPC limita cada respuesta a 1000 diagnósticos, 20 000 tokens con lexemas de 256 caracteres y 5000 símbolos.

La aplicación solo declara `core:default` y permiso para abrir el diálogo de carpetas. No instala los plugins shell o filesystem. La raíz elegida se canonicaliza en Rust y cada módulo vuelve a validarse para impedir escapes mediante rutas o enlaces simbólicos. Los comandos limitan rutas a 1024 bytes y nunca devuelven errores internos o backtraces.

La red, procesos, lectura arbitraria, telemetría, actualizaciones y paquetes remotos están fuera del MVP. Los artefactos deben firmarse cuando el proyecto disponga de certificados; CI no almacena credenciales.
