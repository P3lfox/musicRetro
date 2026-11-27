# Reproductor Retro — Especificación del proyecto

> Documento de contexto para Claude Code. Leelo completo antes de escribir código.

## 1. Visión

Reproductor de música **nativo de escritorio** con estética de menú de consola de la era PS2/PSP (≈2000–2006).
Debe ser **liviano como el software de esa época**: arranque rápido, poco uso de RAM/CPU, sin runtimes pesados.

- **Plataforma inicial:** Windows (x86_64, MSVC).
- **Plataformas futuras:** Android e iOS (y Linux/macOS de rebote). La arquitectura debe permitirlo desde el día 1.
- **Prohibido:** navegador, Electron, Tauri, webviews, React/React Native, cualquier stack basado en HTML/JS.

## 2. Stack técnico

| Área | Elección | Notas |
|---|---|---|
| Lenguaje | **Rust** (edición 2021+, stable) | |
| UI | **Slint** (crate `slint`) | Corre en Windows/Linux/macOS/Android/iOS y embebidos. Usar renderer **Skia** o **FemtoVG**; evaluar el software renderer como modo "ultra liviano". |
| Efectos visuales | Shaders vía integración **WGPU** de Slint (o pre-render a imagen si es más barato) | CRT, halftone, aberración cromática, glitch. |
| Decodificación | `symphonia` | FLAC, MP3, AAC/M4A, OGG/Vorbis, WAV. |
| Salida de audio | `cpal` (o `rodio` si simplifica) | Priorizar reproducción bit-perfect / sin resampleo innecesario cuando el dispositivo lo permita (el usuario usa DAC externo). |
| Metadatos / tags | `lofty` | Título, artista, álbum, año, portada embebida. |
| Imágenes | `image` | Decodificar y **redimensionar portadas a caché** (thumbnails) para no cargar imágenes grandes en RAM. |
| Persistencia | `rusqlite` (SQLite, feature `bundled`) | Índice de la biblioteca, caché de metadatos, ajustes. |
| HTTP (fase APIs) | `ureq` (bloqueante, liviano) o `reqwest` con `rustls` | Evitar traer tokio completo si no hace falta. |
| Escaneo de carpetas | `walkdir` | |
| Config | `serde` + `toml` | Archivo `config.toml` en el directorio de datos del usuario (`dirs` crate). |
| Logs | `log` + `env_logger` | |

Regla general: **antes de agregar una dependencia, justificar su peso**. Preferir crates chicos y features mínimas (`default-features = false`).

## 3. Objetivos de rendimiento

- Arranque en frío: **< 1 s** en hardware común.
- RAM en reposo con biblioteca de ~5.000 temas: apuntar a **decenas de MB**, no cientos.
- CPU en reproducción con la UI quieta: cercana a 0 % aparte de la decodificación.
- **No redibujar constantemente**: animaciones solo mientras ocurren; nada de loops a 60 fps cuando la pantalla está estática (excepto efectos opcionales que el usuario puede desactivar).
- Binario release con `lto = true`, `codegen-units = 1`, `opt-level = 3` (o `"s"` para tamaño), `strip = true`, `panic = "abort"`.

## 4. Diseño visual (referencia: menús de consola retro)

### 4.1 Elementos de pantalla
- **Fondo:** ciudad nocturna (autopista elevada, rascacielos, letreros de neón) con **trama de puntos (halftone)**, tonos desaturados verde oliva / cian / ámbar. Debe poder cambiarse por "tema".
- **Overlay CRT/VHS:** scanlines suaves, ruido/grano, viñeta, leve aberración cromática RGB. Intensidad configurable y desactivable.
- **Barra de estado** arriba a la derecha: fecha y hora (`5/24 6:35PM`, formato M/D h:mmAM/PM) + ícono de batería pixelado.
- **Barra de íconos** abajo a la derecha: inicio, búsqueda, wifi/red, mensajes/info (íconos monocromos, finos).
- **Ícono decorativo de CD tornasolado** (holográfico/iridiscente) con una nota musical, girando lentamente en la vista de biblioteca.
- **Indicador "Loading…"** pequeño, arriba a la derecha, en las transiciones entre pantallas.
- **Tipografía:** sans-serif limpia y fina para títulos/artistas; **monoespaciada** para paneles de sistema y ayudas de botones. Usar fuentes con licencia libre (OFL) incluidas en `assets/fonts`.

### 4.2 Pantalla "Biblioteca" (lista vertical)
- Columna izquierda con **portadas cuadradas apiladas verticalmente**.
- El ítem seleccionado se ve **más grande**, con título (blanco) y artista (gris) a la derecha; si es explícito, badge `E`.
- Los ítems no seleccionados quedan **más chicos y atenuados**; los de los bordes se desvanecen (fade) arriba y abajo.
- Navegación con **scroll suave e inercia** (rueda, flechas, gamepad).

### 4.3 Pantalla "Selección de álbum / tema" (stack de cajas)
- Portadas apiladas en **cascada** (como cajas de discos una detrás de otra), la activa al frente y entera.
- Texto central tenue tipo `Select a Theme`.
- **Panel de sistema** arriba a la derecha: recuadro con borde fino y fondo semitransparente, texto monoespaciado, p. ej.:
  ```
  Choose your soundtrack. Each track creates a
  different atmosphere and shapes the experience ahead.
     ✕ SELECT   △ INFO
     ○ BACK
  ```
- Al cambiar de ítem: pequeño **glitch/flash** y deslizamiento de las cajas.

### 4.4 Interacción de reproducción
1. El usuario confirma un álbum/tema.
2. **Animación: el CD sale de la caja** deslizándose hacia la derecha y empieza a girar.
3. Aparece el prompt: `PLAY?   ✕ SELECT   ○ BACK`.
4. Al confirmar: transición con glitch + `Loading…` y pasa a la pantalla de reproducción.
5. El tema del fondo puede cambiar según el álbum (tinte tomado de los colores dominantes de la portada).

### 4.5 Pantalla "Reproduciendo"
- Portada grande, título, artista, álbum.
- Barra de progreso fina, tiempos `mm:ss`.
- Info técnica en monoespaciado: formato, sample rate, bit depth (ej. `FLAC · 44.1kHz · 16bit`).
- Controles: play/pausa, anterior, siguiente, aleatorio, repetir, volumen.

### 4.6 Controles
| Acción | Teclado | Gamepad |
|---|---|---|
| Seleccionar (✕) | Enter / Espacio | A / ✕ |
| Atrás (○) | Esc / Backspace | B / ○ |
| Info (△) | I | Y / △ |
| Navegar | Flechas | D-pad / stick |
| Play/Pausa | Espacio (en reproducción) / tecla multimedia | Start |

Soporte de gamepad: crate `gilrs` (fase posterior). Soportar teclas multimedia de Windows.

### 4.7 Sonidos de UI
Sonidos cortos de navegación y confirmación (estilo consola), en `assets/sfx`, **originales o con licencia libre**. Volumen propio y opción de silenciarlos.

### 4.8 Propiedad intelectual
- No incluir logos, marcas, íconos ni sonidos de PlayStation ni de otras empresas. Los símbolos ✕ ○ △ se dibujan como glifos genéricos.
- Las portadas vienen **siempre** de los archivos del usuario o de APIs (Cover Art Archive). No se incluyen portadas en el repo.
- Las imágenes de fondo deben ser propias, generadas o con licencia libre.

## 5. Fuentes de música

### Fase 1 — Biblioteca local (núcleo)
- El usuario elige una o más carpetas; se escanean e indexan en SQLite.
- Portadas: embebidas (`lofty`) → `cover.jpg`/`folder.jpg` en la carpeta → placeholder.
- Re-escaneo incremental (por fecha de modificación).

### Fase 2 — Metadatos online (opcional, desactivable)
- **MusicBrainz** para completar metadatos (respetar su rate limit y enviar un User-Agent identificable).
- **Cover Art Archive** para portadas faltantes (cachear en disco).
- **Last.fm** para scrobbling (API key del usuario en config).

### Fase 3 — Descubrimiento (opcional)
- Previews de 30 s vía APIs públicas (Deezer / iTunes Search). Verificar los términos de uso vigentes antes de implementarlo.
- Streaming completo (Spotify/Tidal): **fuera de alcance** por ahora; las APIs oficiales no permiten reproducción completa en reproductores propios. Solo reevaluarlo si aparece una vía oficial.

## 6. Arquitectura

Workspace de Cargo separando núcleo y UI para poder reutilizar el núcleo en móvil:

```
reproductor-retro/
├─ Cargo.toml              # workspace
├─ crates/
│  ├─ core/                # sin dependencias de UI
│  │  ├─ library/          # escaneo, índice SQLite, búsqueda
│  │  ├─ metadata/         # lofty, portadas, caché de thumbnails
│  │  ├─ player/           # symphonia + cpal, cola, estados, eventos
│  │  ├─ online/           # MusicBrainz, Cover Art Archive, Last.fm (feature flag)
│  │  └─ config/
│  └─ app-desktop/         # binario Slint para Windows/Linux/macOS
│     ├─ ui/               # archivos .slint (pantallas y componentes)
│     ├─ shaders/          # efectos CRT/halftone/glitch
│     └─ src/
├─ assets/
│  ├─ fonts/
│  ├─ backgrounds/
│  ├─ sfx/
│  └─ icons/
└─ docs/
```

- **Hilo de audio separado** de la UI. Comunicación por canales (`crossbeam-channel` o `std::sync::mpsc`). La UI nunca bloquea el audio ni al revés.
- El escaneo de biblioteca y la carga de portadas van en hilos de trabajo; la UI recibe resultados de a poco.
- Las portadas se muestran desde thumbnails cacheados (p. ej. 256 px y 512 px), nunca desde el original a tamaño completo.
- Más adelante, `crates/app-android` y `crates/app-ios` reutilizan `core` y los `.slint`.

## 7. Hitos

1. **M0 – Esqueleto:** workspace, ventana Slint en Windows con fondo estático y barra de estado (hora/batería). Build release funcionando.
2. **M1 – Audio:** reproducir un FLAC/MP3 desde la línea de comandos con `core::player` (play/pausa/seek/siguiente).
3. **M2 – Biblioteca:** elegir carpeta, escaneo e índice SQLite, caché de portadas.
4. **M3 – Pantalla Biblioteca:** lista vertical de portadas con selección agrandada, fades y scroll suave.
5. **M4 – Stack de álbumes + animación CD + prompt PLAY?**
6. **M5 – Pantalla Reproduciendo** + teclas multimedia.
7. **M6 – Efectos:** halftone, CRT, glitch de transición, sonidos de UI; toggles de rendimiento.
8. **M7 – Metadatos online** (MusicBrainz/Cover Art/Last.fm).
9. **M8 – Gamepad, temas de fondo, pulido.**
10. **M9 – Port a Android**, después iOS.

En cada hito, medir y anotar en `docs/perf.md`: RAM en reposo, CPU en reproducción, tiempo de arranque y tamaño del binario.

## 8. Build en Windows

- Toolchain: `rustup` con target `x86_64-pc-windows-msvc` + Visual Studio Build Tools (C++).
- `cargo run -p app-desktop` para desarrollo; `cargo build --release -p app-desktop` para distribuir.
- Ocultar la consola en release: `#![windows_subsystem = "windows"]`.
- Ícono y metadatos del .exe con `winres` o `embed-resource`.
- El ejecutable final debe ser **portable** (un .exe + carpeta `assets`) y no requerir instalar runtimes.

## 9. Convenciones

- `cargo fmt` y `cargo clippy -- -D warnings` limpios antes de cada commit.
- Errores con `thiserror` en `core` y `anyhow` en el binario.
- Sin `unwrap()` en rutas de producción.
- Tests unitarios en `core` (escaneo, parseo de tags, cola de reproducción).
- Código y comentarios en inglés, textos de la UI localizables (es/en).
- Commits chicos, uno por funcionalidad.
