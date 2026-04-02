# ROCm VRAM Cleanup

ROCm can leave VRAM allocated after containers stop. Symptoms: `hipErrorOutOfMemory`, high VRAM with no active containers.

## Quick Fix

```bash
# Stop all ROCm containers
docker ps -q --filter "ancestor=backyard-rocm-builder:gfx1031" | xargs -r docker stop -t 0
docker ps -a -q | xargs -r docker rm -f

# Kill orphaned host processes
pkill -9 -f "llama|bench|hip|rocm"

# Check /dev/kfd is free (requires sudo)
sudo fuser -v /dev/kfd
```

## Full Reset

```bash
docker stop $(docker ps -aq) && docker rm -f $(docker ps -aq)
pkill -9 -f "hip|rocm|llama"
sudo fuser -v /dev/kfd  # should show nothing
```

If VRAM still stuck:
```bash
sudo modprobe -r amdgpu && sudo modprobe amdgpu
```

## Verify

```bash
export LD_LIBRARY_PATH=build_workdir/rocm-7.11-gfx103x/build-u2404-stage2/dist/rocm/lib
./build_workdir/rocm-7.11-gfx103x/build-u2404-stage2/dist/rocm/bin/rocm-smi --showmeminfo vram
```

Target: < 200MB used.
