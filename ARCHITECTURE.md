# Architecture

## Hook boundary

`retour::static_detour!` intercepts only `ole32!CoCreateInstance`.

- Non-DirectDraw CLSID/IID pairs are forwarded to the original function.
- `CLSID_DirectDraw + IID_IDirectDraw` receives a Rust-managed COM object.
- No system DirectDraw object vtable is patched.
- No external DirectDraw wrapper DLL is loaded.

## Logical display mode

`IDirectDraw::SetDisplayMode(width, height, bpp)` stores the requested mode in `DrawState` but does not change the physical desktop mode. `SetCooperativeLevel` converts the game window into a borderless popup covering the nearest monitor.

## Surface model

Each surface is a COM object with:

- the 36-slot `IDirectDrawSurface` v1 vtable;
- a top-down DIB section;
- a memory DC;
- 32-bit aligned pitch;
- a reference count;
- an optional attached back buffer;
- optional palette, clipper, and source color key state.

The primary surface is presented after `Unlock`, `ReleaseDC`, `Blt` to primary, or `Flip`.

## Renderer

The renderer is pure GDI:

1. compose the complete output into a persistent compatible memory DC;
2. scale directly from the DIB pixel buffer with `StretchDIBits`, bypassing any mapping transform left on the game-facing surface HDC;
3. save and normalize the window DC, publish the finished frame with one `BitBlt`, then restore the original DC state.

This keeps the implementation small, removes D3D9/OpenGL dependencies, and avoids visible intermediate black frames.

## Focus behavior

The wrapper does not block `WM_ACTIVATE` or `WM_ACTIVATEAPP`. The relay window procedure forwards messages to the original engine procedure. Because there is no exclusive low-resolution display mode, Alt+Tab and Win+Tab do not force the rest of Windows into 800x600 or 640x480.

## Threading assumptions

The implementation targets a legacy engine that primarily renders from one thread. Objects use atomic reference counts and mutexes for shared state, but this is not a complete multi-threaded DirectDraw implementation.

## Scaling quality

The default presenter uses GDI `HALFTONE`. The optional `nearest-neighbor` feature switches back to `COLORONCOLOR` for sharper pixel-art style edges.

## Flicker-free presentation

The logical DirectDraw DIB is scaled into a persistent client-sized compatible bitmap selected into a memory DC. `StretchDIBits` reads the pixel buffer directly, so custom mapping state on the HDC returned by `IDirectDrawSurface::GetDC` cannot shrink or offset the source. Only after the entire frame, including any letterbox bars, is complete is it copied to a normalized window DC with one `BitBlt`.


## Windowed primary coordinates

DirectDraw windowed primary-surface blits use desktop coordinates. Before a
primary `Blt` or `BltFast`, the wrapper compares the logical-surface overlap of the raw
rectangle with a rectangle translated by the HWND client origin. The translated
form is selected only when it covers more of the logical primary, preserving
compatibility with engine paths that already emit local coordinates. Clipping
keeps the original source/destination mapping instead of rescaling after clamp.
