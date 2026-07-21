# Manual EXE integration

This project intentionally has no automatic EXE detection and no automatic EXE patching.

## Recommended approach

Add one import descriptor / IAT thunk:

```text
DLL:      ygo_fix.dll
Function: YgoFixInitialize
```

A dedicated call site is not required for the hook to work. The import is enough for the Windows loader to load the DLL, and the bootstrap thread is started from `DllMain`. Calling `YgoFixInitialize` explicitly is still valid and returns `TRUE` when the detour is installed.

## Requirements

- The EXE must be PE32/x86.
- The EXE must create DirectDraw through `CoCreateInstance(CLSID_DirectDraw, IID_IDirectDraw)`.
- The DLL must be built for `i686-pc-windows-msvc`.
- The DLL must be located beside the EXE or in another directory visible to the Windows loader.
```

## If a variant does not hook

This wrapper currently intercepts only `CoCreateInstance`. If a given EXE variant creates DirectDraw through `DirectDrawCreate`, imports `ddraw.dll` directly, or initializes DirectDraw before the detour is installed, that variant will need an additional intercept path instead of a fixed-address patch.
