# ygo_fix_retour_v0.1

A minimal Rust DirectDraw v1 wrapper for the specific family of x86 EXE variants validated during this project.

The goal is to let the game keep using a logical 800x600 framebuffer **without changing the physical Windows display mode**. The game window is converted to a native-resolution borderless window and the software framebuffer is scaled to that window with a flicker-free GDI presenter.

## Status

This is an engine-focused wrapper, not a general DirectDraw replacement.

## Architecture

```text
game.exe
  └─ ole32!CoCreateInstance(CLSID_DirectDraw, IID_IDirectDraw)
       └─ retour::static_detour!
            └─ Rust IDirectDraw v1 wrapper
                 ├─ software IDirectDrawSurface v1 objects
                 ├─ DIB section framebuffer storage
                 ├─ Blt / BltFast / Flip / Lock / Unlock
                 ├─ 8-bit palette and source color key support
                 ├─ native-resolution borderless window handling
                 ├─ mouse-coordinate remapping
                 └─ flicker-free GDI scaling presenter
```

`SetDisplayMode` stores only the logical resolution. It does not call `ChangeDisplaySettings`, so Windows never broadcasts `WM_DISPLAYCHANGE` for 800x600 to other windows.

## Build

Requirements (tested environment):

- Windows x64;
- Rust nightly;
- Visual Studio Build Tools with **Desktop development with C++**;
- target `i686-pc-windows-msvc`.

The project already pins nightly in `rust-toolchain.toml` and the x86 target in `.cargo/config.toml`.

```cmd
rustup target add i686-pc-windows-msvc
cargo build --release
```

Output:

```text
target\i686-pc-windows-msvc\release\ygo_fix.dll
```

To preserve source aspect ratio, for example 4:3 on a 16:9 monitor:

```cmd
cargo build --release --features aspect-4x3
```

The default presenter uses GDI `HALFTONE` scaling to reduce aliasing at non-integer scale ratios. To restore sharper nearest-neighbor style scaling:

```cmd
cargo build --release --features nearest-neighbor
```

## Manual EXE integration

Add one import:

```text
ygo_fix.dll!YgoFixInitialize
```

The import is enough to make the Windows loader load the DLL. `DllMain` starts a bootstrap thread that installs the detour. Calling `YgoFixInitialize` explicitly is also valid; repeated calls are safe because the hook is installed only once.

Place beside the EXE:

```text
game.exe
ygo_fix.dll
```

Do not place other DirectDraw wrappers beside the game during the first test, including `ddraw.dll`, dgVoodoo, or cnc-ddraw.

## Current implemented scope

Implemented behavior:

- `IDirectDraw` v1;
- `IDirectDrawSurface` v1;
- primary surface plus one back buffer;
- off-screen surfaces;
- `Lock` / `Unlock`;
- `Blt` / `BltFast`;
- `Flip`;
- `GetDC` / `ReleaseDC`;
- `GetAttachedSurface`;
- `SetPalette` and 8-bit palette support;
- source color key;
- 8/16/24/32-bit DIB-backed surfaces;
- borderless fullscreen and mouse mapping.

Not fully supported:

- Direct3D interfaces;
- overlays;
- `BltBatch`;
- more than one back buffer;
- true hardware video-memory semantics;
- full lost-surface behavior;
- precise vsync;
- arbitrary DirectDraw games.

## Credit

Initial architecture and implementation of the Rust DirectDraw v1 wrapper, software surfaces, GDI renderer, focus/window handling, mouse mapping, and project packaging:

**OpenAI GPT-5.6 Thinking (high-reasoning mode)**

Function detouring:

**retour-rs**, by Mason Ginter, Elliott Linder, and contributors — BSD-2-Clause.

Manual EXE selection, import assignment, build validation, and runtime testing:

**Project maintainer/user**.

cnc-ddraw was used only as a behavioral reference during earlier experiments. No cnc-ddraw source code or binary is embedded in this project.

See also `ARCHITECTURE.md`, `MANUAL_INTEGRATION.md`, `THIRD_PARTY_NOTICES.md` and `KNOWN_LIMITATIONS.md`.
