### 1. Panorama de compatibilidad por sistema operativo (Investigación parcial)

| Característica | Windows | Linux | macOS |
| :--- | :--- | :--- | :--- |
| **Mapeo Teclado / Mouse** | Muy bueno (`WH_KEYBOARD_LL` o drivers) | **Excelente** (`evdev` / `uinput`) | **Bueno** (`CGEventTap` con Accesibilidad) |
| **Bloqueo nativo del input físico** | Bueno (el hook decide si consumir el evento) | **Perfecto** (`EVIOCGRAB` aisla el dispositivo) | Bueno (el tap puede descartar el evento) |
| **Detección de App en foco** | Directo (`GetForegroundWindow`) | Fácil en X11, limitado/variable en Wayland | Fácil (`NSWorkspace.frontmostApplication`) |
| **Lectura de Mandos (Gamepads)** | Directo (XInput / RawInput / HID) | Directo (`evdev` / HID) | Directo (`GameController` / IOHIDManager) |
| **Mandos Virtuales (Xbox/PS)** | **Requiere Driver** (ViGEmBus / HidHide) | **Nativo y sin drivers** (`uinput`) | **Extremadamente difícil / Restringido** (DriverKit) |
| **Ocultar mando físico (Double Input)** | Requiere Driver (HidHide) | Nativo (`EVIOCGRAB` en `evdev`) | Muy complejo en DriverKit |

---

> [!IMPORTANT]
> Esta Investigación es **parcial y no exhaustiva**. Se centra en la investigación de recursos, librerías en Rust puro y APIs nativas de cada sistema operativo. Se plantea llegar a un **remapeador multiplataforma** que funcione en Windows, Linux y macOS, pero con limitaciones de mando virtual en macOS debido a restricciones de Apple. Este proyecto empiza desde el remapeo de teclas, hasta un vistualizador de mandos virtuales, mapeador de mandos físicos y virtuales, y finalmente un remapeador completo estilo reWASD.

### 2. La realidad de macOS: ¿Qué puedes y qué no puedes hacer?

#### Lo que SÍ se puede hacer en macOS (Teclado, Mouse y Macros)
Para remapear teclas y ratón en macOS, **no necesitas drivers de kernel**:
* **Intercepción**: Usar **`CGEventTap`** (CoreGraphics). Permite interceptar eventos globales de teclado y ratón antes de que lleguen al sistema o a cualquier app, modificarlos o descartarlos (evitando que el input original pase).
* **Inyección**: Crear eventos sintéticos con `CGEventCreateKeyboardEvent` y los envías con `CGEventPost`.
* **Permisos**: Requiere que el usuario conceda permisos de **Accesibilidad** (`AXIsProcessTrusted`) en *Ajustes del Sistema > Privacidad y Seguridad > Accesibilidad*.
* **App en primer plano**: En lugar de `GetForegroundWindow`, se utiliza Cocoa / AppKit (`NSWorkspace.shared.frontmostApplication`) para saber qué bundle id (e.g. `com.google.Chrome`) está activo.

> **Limitaciones de Teclado en Mac:**
> - macOS bloquea `CGEventTap` cuando el usuario entra en campos de contraseña mediante el modo *Secure Input* (`EnableSecureEventInput`).
> - Ciertas teclas especiales del sistema (Fn/Globe, brillo, Mission Control, Touch Bar) son capturadas por el WindowServer antes de llegar a los taps convencionales.

#### Lo que es casi imposible o extremadamente difícil en macOS (Mandos Virtuales y Drivers)

En **macOS moderno (Apple Silicon / macOS 11 Big Sur en adelante)**:
1. **Apple eliminó los KEXTs (Kernel Extensions)**: Antes se podían usar kexts abiertos como *Foohid* o *360Controller*. Hoy están bloqueados o exigen desactivar SIP (*System Integrity Protection*), algo impensable para un usuario común.
2. **DriverKit y SystemExtensions**: Para crear un mando virtual reconocido por los juegos y por Apple (`GameController.framework`), tendría que programar un driver con **`HIDDriverKit`**. Lo que se vuelve desde ya inviable, se plantea un desarrollo a limitación para macOS, donde las features avanzadas muestran una señal de incompatibilidad, en UI se mostraría el apartado pero bloqueado por una advertencia y una dialog transparente.
3. **La barrera de Apple**: Para compilar y distribuir una extensión de DriverKit necesitaría una cuenta de Apple Developer ($99 USD/año) y **solicitar un derecho especial (*entitlement*) a Apple** (`com.apple.developer.driverkit`), el cual Apple aprueba manualmente caso por caso.

**Conclusión para macOS:**
* **Versión teclado, ratón, macros y alguna implementación para mandos físico:** 100% viable y funciona de maravilla.
* **Versión mando virtual (reimplementación de reWASD completo):** Muy poco práctico y con una fricción regulatoria y técnica enorme. La gran mayoría de herramientas en macOS (como *Karabiner-Elements*) se limitan estrictamente a teclado y ratón por esta misma razón.

---

### 3. Linux: La plataforma para este tipo de software

Paradójicamente, **Linux es el mejor sistema operativo para emular reWASD o aplicaciones similares**:
* **`evdev`**: Permite abrir cualquier archivo `/dev/input/event*` (teclados, ratones, mandos físicos de Xbox, PS5, Switch Pro, etc.).
* **Exclusividad con `EVIOCGRAB`**: Puedo decirle al kernel: *"este mando físico o teclado solo me envía eventos a mí"*. El sistema y los juegos no ven nada del dispositivo real, eliminando por completo el problema del *double input* sin necesidad de drivers adicionales.
* **`/dev/uinput`**: Es una interfaz del kernel que permite a cualquier proceso de usuario crear **dispositivos virtuales** completos (teclados virtuales, ratones virtuales y **mandos de Xbox 360 / PlayStation virtuales**). Steam y los juegos de Linux los detectan al instante como mandos nativos.
* **Crates de Rust**: En Linux se pueden usar crates como [`evdev`](https://crates.io/crates/evdev) para hacer todo esto directamente en Rust puro y seguro (solo configurando una regla de `udev` para permisos de usuario).

---

### 4. ¿Cómo adaptar la arquitectura actual en Rust para escalar?

> Se toma ejemplo de la arquitectura de [ArtCraft](https://github.com/storytold)

El workspace está muy bien estructurado (`apps/remapr`, `crates/engine`, `crates/platform`, `crates/ui`), pero actualmente tiene dos acoplamientos fuertes a Windows que debes abstraer:

#### A. Desvincular el Virtual Key (`Vk`) de Win32
En `crates/engine/src/lib.rs` tienes:
```rust
pub type Vk = u8; // Acoplado a VK_* de Win32
```
Si quieres dar soporte a macOS y Linux, la lógica pura de [`remap.rs`](./crates/engine/src/remap.rs) no debería conocer qué es un `VK_SPACE`.
* **Solución**: Adoptar como identificador universal los **USB HID Usage IDs** (el estándar de hardware universal para teclados y botones) o definir un enum canónico `KeyCode` agnóstico.
* Luego, cada plataforma tiene su traductor:
  * Windows: `Win32 VK <-> KeyCode` (vía `MapVirtualKeyW`).
  * macOS: `macOS KeyCode <-> KeyCode`.
  * Linux: `Linux Input KeyCode (evdev) <-> KeyCode`.

#### B. La capa de Hooks / Backends de Entrada
En `crates/engine`, separar la implementación del hook con un trait o abstracción condicional:

```text
crates/engine/
├── src/
│   ├── lib.rs          # Tipos agnósticos (Chord, Rule, Mods, KeyCode)
│   ├── remap.rs        # Lógica de decisión pura
│   └── backend/
│       ├── windows.rs  # WH_KEYBOARD_LL + SendInput
│       ├── macos.rs    # CGEventTap + CGEventPost (core-graphics)
│       └── linux.rs    # evdev + uinput
```

En Rust puedes activarlos limpiamente con compilación condicional:
```rust
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
```

#### C. `crates/platform`: Abstracción del Sistema Operativo
El crate [`crates/platform`](./crates/platform/src/lib.rs) actualmente incluye utilidades como single instance, app en primer plano, etc.
Se Pueden definir interfaces comunes:
```rust
pub fn active_app_name() -> Option<String> {
    #[cfg(target_os = "windows")] { windows::get_foreground_exe() }
    #[cfg(target_os = "macos")]   { macos::get_frontmost_bundle() }
    #[cfg(target_os = "linux")]   { linux::get_active_window_class() }
}
```

---

### 5. Hoja de ruta recomendada para tu proyecto

El objetivo es construir algo sólido que escale gradualmente:

1. **Fase 1 (Corto Plazo - Teclados y Ratones Multiplataforma)**:
   * Se Mantiene Windows como plataforma principal de pruebas.
   * Agregar soporte de **macOS** para teclado y ratón utilizando `CGEventTap` y el crate `core-graphics`. La UI (`eframe`/`egui`) ya funciona de forma nativa en Mac sin cambios.
   * Agregar soporte de **Linux** para teclado y ratón usando `evdev` y `uinput`.

2. **Fase 2 (Mediano Plazo - Mandos y Virtual Gamepads en Windows & Linux)**:
   * En **Windows**: Integrar **ViGEmBus** (a través del crate [`vigem-client`](https://crates.io/crates/vigem-client)) para crear mandos virtuales de Xbox 360 / DS4, y usar **HidHide** para evitar el doble input del mando físico.
   * En **Linux**: Implementar mandos virtuales con `/dev/uinput` y capturar mandos físicos con `evdev`.
   * En **macOS**: Mantener macOS como un *"Keyboard & Mouse Remapper"* avanzado (estilo Karabiner) y aclara a los usuarios que la emulación de mandos virtuales es exclusiva de Windows y Linux por restricciones de la plataforma de Apple.

> Como se mencionó al inicio esta investigación es parcial, con el tiempo se puede apliar o mejorar cualquier punto de esta que no este acorde con los estandares para la escalabilidad del proyecto. Para esto tomar de ejemplo los repos de ArtCraft que tienen un muy bien trabajo estructurando proyectos en Rust y multiplataforma.