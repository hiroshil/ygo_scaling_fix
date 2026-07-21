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
2. apply stretch or aspect-preserving scaling there;
3. publish the finished frame to the game window with one `BitBlt`.

This keeps the implementation small, removes D3D9/OpenGL dependencies, and avoids visible intermediate black frames.

## Focus behavior

The wrapper does not block `WM_ACTIVATE` or `WM_ACTIVATEAPP`. The relay window procedure forwards messages to the original engine procedure. Because there is no exclusive low-resolution display mode, Alt+Tab and Win+Tab do not force the rest of Windows into 800x600 or 640x480.

## Threading assumptions

The implementation targets a legacy engine that primarily renders from one thread. Objects use atomic reference counts and mutexes for shared state, but this is not a complete multi-threaded DirectDraw implementation.

## Scaling quality

The default presenter uses GDI `HALFTONE`. The optional `nearest-neighbor` feature switches back to `COLORONCOLOR` for sharper pixel-art style edges.

## Flicker-free presentation

The logical DirectDraw DIB is scaled into a persistent client-sized compatible bitmap selected into a memory DC. Only after the entire frame, including any letterbox bars, is complete is it copied to the window DC with one `BitBlt`. The real window DC is never cleared as a separate visible step.
