# Building Native ROCm 7.11 for gfx1031 (RX 6700 XT)

## Build Environment

- **OS**: Ubuntu 24.04 (Docker)
- **Base Image**: `backyard-rocm-builder:gfx1031`
- **Workdir**: `build_workdir/rocm-7.11-gfx103x/`

## Component Stripping

Disabled to fit within ~30GB RAM:

| Component | Status | Reason |
|-----------|--------|--------|
| MIOpen | OFF | ~8GB RAM, not needed for llama.cpp |
| hipDNN | OFF | Unused by llama.cpp |
| Composable Kernel | OFF | Build time reduction |
| rocprofiler-sdk | OFF | Strict deps cause failures |

## Critical Patches

### Tensile Include Path Fix

TheRock unsets `ROCM_PATH`, breaking Tensile HIP include discovery.

**File**: `rocm-libraries/shared/tensile/Tensile/BuildCommands/SourceCommands.py`

```python
hipFlags += ["-I", outputPath, "-I", os.path.abspath(os.path.join(os.path.dirname(cxxCompiler), "../../../include"))]
```

### Finder Macro Fix

Strict finder macro crashes when profilers are disabled.

**File**: `cmake/therock_explicit_finders.cmake` - remove `roctx64` and `roctracer/roctx.h` from required deps.

### Math Library Conditional Linking

**Files**: `pre_hook_rccl.cmake`, `pre_hook_rocBLAS.cmake`, `pre_hook_rocSPARSE.cmake`

Make `roctx` linking conditional.

### LLVM Bitcode Symlink

```bash
ln -sf $ROCM_DIST/lib/llvm/amdgcn $ROCM_DIST/amdgcn
```

## Compiling llama.cpp

```bash
cmake -B build \
    -DGGML_HIP=ON -DHIP_PLATFORM=amd -DAMDGPU_TARGETS=gfx1031 \
    -DCMAKE_PREFIX_PATH="$ROCM_DIST" \
    -DCMAKE_C_COMPILER="$ROCM_DIST/llvm/bin/clang" \
    -DCMAKE_CXX_COMPILER="$ROCM_DIST/llvm/bin/clang++" \
    -DCMAKE_HIP_COMPILER="$ROCM_DIST/llvm/bin/clang++" \
    -DCMAKE_HIP_COMPILER_ROCM_ROOT="$ROCM_DIST" \
    -DCMAKE_BUILD_TYPE=Release -DGGML_CCACHE=OFF
```

## Running

```bash
export LD_LIBRARY_PATH=build_workdir/rocm-7.11-gfx103x/build-u2404-stage2/dist/rocm/lib
./llama-bench -m model.gguf -ngl 99
```

## Memory

- **RAM**: `-j4` parallelism for ~30GB limit
- **Disk**: ~80GB for full build
- **VRAM**: Clean up after builds, see `ROCM_VRAM_CLEANUP.md`
