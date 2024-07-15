# Nodo de procesamiento

Este repositorio hospeda el codigo relacionado al modulo de procesamiento del proyecto Cloud Gaming Rental Service descripto en el siguiente [informe](https://drive.google.com/file/d/1G9Y-qSAztYXd9f97DJ-oina4pQhgBauq/view?usp=sharing).

# Dependencias

## Gstreamer

Gstreamer es una herramienta de código abierto para la manipulación de flujos multimedia. Para instalarla se pueden seguir las instrucciones descriptas en la pagina oficial de [Gstreamer](https://gstreamer.freedesktop.org/documentation/installing/on-windows.html?gi-language=c).

Todas las demás dependecias necesarias estan definidas en el archivo Cargo.toml. Para instalarlas, ejecutar el comando:

```bash
cargo build
```

# Ejecución

Para ejecutar el programa, ejecutar el comando:

```bash
cargo run
```

# Consideraciones

Este proyecto solo soporta la ejecución en sistemas operativos Windows.

Una vez iniciado, el sistema queda a la espera de una conexión TCP local en el puerto 2930. Para conocer los mensajes soportados, refiérase a la sección "Protocolos" dentro del anexo del informe.

Otra consideración importante es que el sistema necesitará conectarse al [servidor intermediario]((https://github.com/Tpp-Cloud-Gaming/server)), también implementado para este proyecto, el cual deberá estar disponible antes de la ejecución del mismo. Nuevamente, para más detalles, refiérase al informe
