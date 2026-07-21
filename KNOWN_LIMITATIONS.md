# Known limitations

- Engine-focused prototype, not a drop-in replacement for every DirectDraw game.
- Intercepts only the COM creation path through `CoCreateInstance`.
- Supports only `IID_IDirectDraw` v1; DirectDraw2/4/7 and Direct3D interfaces return `E_NOINTERFACE`.
- The current flip chain has one back buffer.
- `Lock` supports full-surface and sub-rectangle access, but not every legacy lock-flag or event semantic.
- `Blt` supports copy, stretch, color fill, and source color key, but not advanced ROP/effects behavior.
- Overlay APIs and `BltBatch` are not implemented.
- `EnumDisplayModes` currently returns a minimal mode set consistent with the active logical mode.
- `GetCaps` provides a minimal capability model; variants that inspect many capability bits may require additional work.
- GDI `HALFTONE` reduces aliasing at non-integer scales but is slower than `COLORONCOLOR`, provides no vsync, and does not match high-quality shader-based scalers.
- The window relay currently manages one game HWND in the process.
- The DLL is assumed to remain loaded until process exit; explicit `FreeLibrary` teardown is not implemented.
- GDI double buffering removes background-clear flicker, but it does not guarantee tear-free presentation.
