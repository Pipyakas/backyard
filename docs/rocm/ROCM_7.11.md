# ROCm 7.11 for gfx1031 (RX 6700 XT)

> **Version**: ROCm 7.11 only (`therock-7.11`). Not forward-compatible.

## Build

### Using Dockerfile (Recommended)

You can build the ROCm 7.11 container with `llama.cpp` using the single Dockerfile:

```bash
docker build -t backyard:llama-rocm-7.11 -f containers/llama.cpp/Dockerfile.rocm.7.11 .
```

This Dockerfile is multi-stage and will:
1.  Clone and patch `ROCm/TheRock` (v7.11).
2.  Build a minimal ROCm runtime (Compiler, Core Runtime, HIP).
3.  Clone and build `llama.cpp` against the newly built ROCm.
4.  Produce a clean runtime image.

### Local Build (Legacy)

> **Note**: This method is deprecated in favor of the Dockerfile.

- **Base**: `ROCm/TheRock` @ `therock-7.11`
- **Patches**: `ChristophBellmann/rocm-7.11-gfx103x`
- **Required patches**:
  - `0001-Ensure-to-use-libamdhip64-with-major-version`
  - `0002-hipcc-fix-default-include-path`
  - `0003-HACK-Handle-ROCM-installation-layout`
  - `0002-Revert-hsakmt-bump-vgpr-count-for-gfx1151`
  - `0003-Use-is_versioned-true-consistently`


### Not Needed

`rocm-libraries/`, `amdsmi/`, `spirv-llvm-translator/`, other llvm patches.

## Build

### Minimal Config (llama.cpp only)

```yaml
amdgpu_targets: gfx1031
features:
  enable_compiler: true
  enable_core_runtime: true
  enable_hip_runtime: true
  # everything else: false
```

```bash
cmake -B build -GNinja . \
  -DTHEROCK_AMDGPU_TARGETS=gfx1031 \
  -DTHEROCK_ENABLE_COMPILER=ON \
  -DTHEROCK_ENABLE_CORE_RUNTIME=ON \
  -DTHEROCK_ENABLE_HIP_RUNTIME=ON \
  -DTHEROCK_ENABLE_ALL=OFF
cmake --build build
```

### Build Container

```bash
docker exec -it rocm-builder bash
cd /build && source .venv/bin/activate

# Stage 1 (amd-llvm)
cd build-stage1 && CMAKE_BUILD_PARALLEL_LEVEL=6 ninja -j6 -k0 2>&1 | tee build.log

# Stage 2 (ROCm)
cd build-stage2 && CMAKE_BUILD_PARALLEL_LEVEL=6 ninja -j6 -k0 2>&1 | tee build.log
```

### Fixing Build Issues

| Issue | Fix |
|-------|-----|
| Missing `xxd` | `apt-get install -y vim-common` |
| Missing OpenGL | `apt-get install -y libgl1-mesa-dev libglu1-mesa-dev` |
| Missing Python3 dev | `apt-get install -y python3-dev` |
| ROCM_PATH not set in sub-builds | Edit `cmake/therock_subproject.cmake`, add `list(APPEND _build_env_pairs "ROCM_PATH=/opt/rocm-test")` after `--unset=ROCM_PATH` line |

### Extracting Built Artifacts

```bash
mkdir -p /opt/rocm-test/{bin,lib,include}

cp /build/build-stage1/compiler/amd-llvm/dist/lib/llvm/bin/* /opt/rocm-test/bin/
cp /build/build-stage1/core/clr/dist/bin/* /opt/rocm-test/bin/
cp /build/build-stage1/compiler/amd-llvm/dist/lib/* /opt/rocm-test/lib/
cp /build/build-stage1/core/clr/dist/lib/* /opt/rocm-test/lib/
cp /build/build-stage1/artifacts/core-runtime_lib_generic/core/ROCR-Runtime/stage/lib/* /opt/rocm-test/lib/
cp -r /build/build-stage1/core/clr/dist/include/* /opt/rocm-test/include/
```

## Runtime

```bash
export HSA_OVERRIDE_GFX_VERSION=10.3.0
export HSA_ENABLE_SDMA=0            # CRITICAL: prevents hang on gfx1031
```

## Container Quick Start (llama.cpp)

```bash
docker run -d --name rocm-server \
  --device /dev/kfd --device /dev/dri/card1 --device /dev/dri/renderD128 \
  -v ./models:/models:ro -p 8080:8080 \
  -e HIP_PLATFORM=rocm \
  -e HSA_OVERRIDE_GFX_VERSION=10.3.0 \
  -e HSA_ENABLE_SDMA=0 \
  backyard:llama-rocm \
  /app/llama-server -m /models/model.gguf --host 0.0.0.0 --port 8080 \
  --n-gpu-layers 999 --threads 8 --ubatch-size 128
```

## Benchmarks (TinyLlama 1.1B Q4, 32 tokens)

| Backend | Avg | GPU Layers |
|---------|-----|------------|
| ROCm    | 108ms | 23/23 |
| Vulkan  | 362ms | 23/23 |

ROCm is ~3-4x faster on gfx1031.
