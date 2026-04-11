# Performance Improvement Plan

Target: 165 Hz monitor, currently achieving 105-115 FPS.

## Bottlenecks Identified

1. **Per-frame 6.3MB instance buffer rebuild + clone + upload** — `instances_dirty` is set unconditionally every frame, triggering full CPU rebuild, Arc clone, and GPU upload of ~6.3MB. At 165 FPS this is ~1 GB/s memory thrashing.
2. **All animation runs on CPU** — every particle's world position computed on CPU per frame instead of GPU.
3. **No view frustum culling** — all 196K particles drawn every frame including ones far behind camera.

## Optimization Plan

### 1. GPU-Driven Animation (Highest Impact, High Effort)

Move animation to the vertex shader:
- Upload 2D orbit points to a storage buffer (once per orbit change, ~226KB every 3s)
- Upload per-set metadata (z_position, z_rotation, color) as small uniform buffer (~1.5KB/frame)
- Instance data reduced to just indices derivable from instance_index
- Vertex shader reads orbit point + per-set data, computes world position

**Eliminates:** 6.3MB CPU build, 6.3MB clone, 6.3MB GPU upload per frame → ~1.5KB/frame.

### 2. Eliminate Per-Frame Arc Clone (High Impact, Tiny Effort)

`app.rs:808` clones ~6.3MB every frame: `Arc::new(self.instance_buffer.clone())`.

Replace with `std::mem::take` to move the buffer into the Arc instead of copying:
```rust
let instances = Arc::new(std::mem::take(&mut self.instance_buffer));
self.instance_buffer = Vec::with_capacity(instances.len());
```

Saves ~1-2ms per frame.

### 3. Cache Per-Set Colors (Low Impact, Tiny Effort)

`renderer.rs:338` calls `hsv_to_rgba()` per set per frame (49 calls/frame), but hue only changes every 3s. Cache the RGBA color in `ParticleSetState` and recalculate only when `needs_update` is true.

### 4. Pack Instance Data (Medium Impact, Medium Effort)

Change `ParticleInstance` from 32 bytes to 16 bytes:
- `color: [f32; 4]` → `color: [u8; 4]` (Unorm8x4 vertex format)
- Remove `_pad: f32`
- Bandwidth: 196K × 16B = 3.14MB instead of 6.27MB (50% reduction)

Best combined with optimization #1.

### 5. View Frustum Culling of Particle Sets (Medium Impact, Low Effort)

Skip entire particle sets whose z_position is far behind the camera. Each set is ~4000 particles, so skipping 10-20 sets saves 40K-80K draw instances.

### 6. Mipmap Sprite Texture (Low-Medium Impact, Low Effort)

`renderer.rs:103` sets `mip_level_count: 1`. Adding mipmaps improves texture cache locality for distant/small particles.

### 7. Present Mode Check (Unknown Impact, Low Effort)

Verify wgpu present mode is Mailbox or Immediate, not Fifo (vsync), to ensure no FPS cap.

### 8. Pre-allocate Orbit Point Storage (Minor Impact, Low Effort)

Use `Arc<Vec<[f32; 2]>>` for orbit data sharing instead of cloning ~28KB per set per wrap.

### 9. Remove micromath Dependency (Marginal Impact, Tiny Effort)

Standard `f32::exp()` uses hardware intrinsics on modern CPUs. Only used once in `sim.rs:218`.

### 10. Inline Hot Paths (Marginal Impact, Tiny Effort)

Add `#[inline]` to `hsv_to_rgba` and `build_instances_into` hot loop. LTO likely handles this already.

## Implementation Order

| Priority | Optimization | Impact | Effort |
|----------|-------------|--------|--------|
| 2 | Eliminate Arc clone | High | Tiny |
| 3 | Cache per-set colors | Low | Tiny |
| 9 | Remove micromath | Marginal | Tiny |
| 10 | Inline hot paths | Marginal | Tiny |
| 5 | View frustum culling | Medium | Low |
| 6 | Mipmap sprite | Low-Medium | Low |
| 7 | Present mode check | Unknown | Low |
| 8 | Arc orbit data | Minor | Low |
| 4 | Pack instance data | Medium | Medium |
| 1 | GPU-driven animation | Very High | High |