## `recoge`

Interfaz gráfica de escritorio para la _GEstión de REgistros de COntenedores_.

[![Current Version](https://img.shields.io/badge/release-v0.2.0-85c20f.svg)](https://github.com/TIDOP-USAL/recoge/releases<)
[![MSRV](https://img.shields.io/badge/MSRV-1.95.0-69add7.svg)](https://rustup.rs/)

---

Herramienta para ayudar a desarrolladores en el manejo de un registro de contenedores privado (zot).

La interfaz muestra una lista de imágenes de contenedor en la máquina local, asociadas a la instalación de Docker o Podman, y también permite listar e interactuar con las imágenes en un registro de contenedores remoto.

`recoge` se puede compilar y ejecutar en Windows, Linux y macOS.

<img width="1813" height="960" alt="Lista de imágenes locales" src="https://github.com/user-attachments/assets/25a7d643-ee1f-4a07-96ba-a54000cf9253" />

<img width="1867" height="1106" alt="Lista de imágenes remotas" src="https://github.com/user-attachments/assets/211f6431-4d5a-4993-9166-5847bf76195f" />

### Descargar el programa

Puedes descargar y ejecutar la última versión desde el apartado Releases del repositorio.

Está disponible para las siguientes plataformas:

- `recoge_x.exe`. Portable para Windows (64 bit).
- `recoge_x_x86_64.AppImage`. Portable para Linux.
- `recoge_x_amd64.deb`. Instalador para Debian GNU/Linux, Ubuntu y similares (64 bit).
- `recoge_x_intel.dmg`. Instalador para macOS (Intel).
- `recoge_x_arm.dmg`. Instalador para macOS (Apple silicon).

Todos los ejecutables están compilados para las últimas versiones de esos sistemas operativos.

Si quieres probar una versión en desarrollo, o modificar `recoge`, compila el programa.

### Compilar el programa

#### Instalar dependencias de compilación

1. Sigue las instrucciones de [rustup.org](https://rustup.org).

`rustup` te instalará la última versión de Rust, probablemente compatible con `recoge`. La versión de Rust mínima necesaria es la 1.95.0.

Con esto ya puedes compilar y ejecutar `recoge`.

#### Compilar y ejecutar

Desde el directorio raíz, ejecuta uno de los siguientes comandos:

- `cargo run`: para compilar y ejecutar el programa en modo `debug` (compila más rápido).
- `cargo run --release`: para compilar y ejecutar el programa en modo `release` (optimizado).
- `cargo build --release`: para compilar el programa en modo `release`, sin ejecutarlo.

Si no tienes instalado un entorno de programación para Rust, te puede ayudar una [herramienta de watch](https://dystroy.org/bacon/).

La primera compilación es más lenta que las siguientes.

#### Generar instaladores del programa

Si quieres compilar paquetes/instaladores (`AppImage`, `deb`, `dmg`, `msi`, ...), ejecuta lo siguiente:

```sh
cargo install cargo-bundle
```

, para instalar las dependencias de compilación. Y después:

```sh
cargo bundle --release
```

, para generar los instaladores para tu sistema operativo. La salida del comando indicará la ruta donde se han generado los ficheros.

#### Transcompilar a Windows desde Linux

Si quieres compilar `.exe` desde Linux, instala MinGW64.

En Debian GNU/Linux:

```sh
sudo apt install mingw-w64 binutils-mingw-w64
```

, y después:

```sh
rustup target add x86_64-pc-windows-gnu
```

A la hora de compilar, usa el target adecuado:

```
cargo build --target x86_64-pc-windows-gnu
```


### Usar el programa

`recoge` está diseñado para usarse con el Registro de Contenedores del TIDOP (`cr.tidop.es`) u otras instancias de [Zot](https://github.com/project-zot/zot). Configura tu registro y credenciales usuario en la ventana de configuración (⚙).

La interfaz de `recoge` sirve para manipular imágenes de contenedor, siempre identificadas por su nombre completo.

El nombre de una imagen de contenedor tiene dos partes: `repositorio:tag`. El `tag` es una versión para cada imagen identificada con su `repositorio`, por ejemplo: `1.2.6`.

Las imágenes locales pueden ser inspeccionadas (ℹ), para obtener información y poder eliminarlas. También se pueden subir al registro (⬆).

Con el botón (🔍 Registro), se pueden ver las imágenes almacenadas en el Registro, y poder descargar (⬇) o eliminar tags asociados a cada repositorio.

Si se deja el cursor encima de un botón o texto, puede aparecer un diálogo explicativo.
