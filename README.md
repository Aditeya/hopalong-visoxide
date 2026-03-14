# Hopalong Visoxide

A real-time Barry Martin's Hopalong Orbits visualizer built with Rust, using **eframe/egui** for the UI and **wgpu** for GPU-accelerated particle rendering.

This is a native desktop port of the [Hopalong Orbits Visualizer](https://github.com/samleatherdale/hopalong-redux) (originally by Iacopo Sassarini, updated by Sam Leatherdale), rewritten from TypeScript/Three.js to Rust.

## What is it?

The Hopalong attractor is a mathematical fractal discovered by Barry Martin. This visualizer generates orbit patterns from the attractor equations and renders them as ~196,000 glowing particles flying toward the camera, creating a mesmerizing tunnel effect. New orbit patterns are generated every 3 seconds with smooth fly-through transitions between them.

## Features

- **GPU-accelerated rendering** via wgpu (Vulkan/Metal/DX12 backends)
- **Instanced billboard particles** with galaxy sprite texturing and additive blending
- **Exponential fog** for depth perception
- **Smooth orbit transitions** - old patterns fly past as new ones emerge from behind
- **Real-time settings panel** (Tab to toggle) with sliders for:
  - Speed, rotation speed, camera FOV
  - Points per subset, subset count, level count
- **Keyboard shortcuts** for speed, rotation, mouse lock, camera centre, reset
- **FPS counter** (toggleable in settings)

## Building

Requires Rust 2024 edition (1.85+).

```sh
cargo build --release
```

## Running

```sh
cargo run --release
```

## Controls

| Key | Action |
|-----|--------|
| Tab | Toggle settings panel |
| Arrow Up/Down | Increase/decrease speed |
| Arrow Left/Right | Increase/decrease rotation |
| L | Toggle mouse lock |
| C | Centre camera + toggle lock |
| R | Reset all settings to defaults |
| Q | Quit |

Mouse movement controls camera position when unlocked.

## Architecture

```
src/
  main.rs              Entry point (eframe::run_native)
  app.rs               eframe::App impl, egui UI, input handling
  sim.rs               Hopalong attractor math, camera state, particle management
  renderer.rs          wgpu pipeline, instanced billboards, CallbackTrait impl
  shaders/
    particle.wgsl      Billboard vertex + fragment shader with fog
assets/
  galaxy.png           Particle sprite texture
```

### How it works

1. **Orbit generation** (`sim.rs`): Computes 2D points using Barry Martin's Hopalong equations with three formula variants (sqrt, fourth-root, logarithmic). Points are normalized to a fixed coordinate range.

2. **Particle sets**: The 2D orbit is replicated across multiple depth levels and color subsets (default: 7x7 = 49 sets of 4,000 points each). Each set has its own Z position, rotation, and baked copy of orbit data.

3. **Animation loop**: Every frame, particle sets advance toward the camera. When a set passes the camera, it wraps to the back. Every 3 seconds, a new orbit is generated - sets pick up the new pattern only when they individually wrap, creating a gradual transition.

4. **Rendering** (`renderer.rs`): Each particle is an instanced camera-facing quad textured with a galaxy sprite. Additive blending makes overlapping particles glow brighter. Exponential fog fades distant particles to black.

## Dependencies

| Crate | Purpose |
|-------|---------|
| eframe | Window + event loop + wgpu init |
| egui / egui-wgpu | UI widgets + custom render callback |
| wgpu | GPU rendering |
| bytemuck | Safe struct-to-byte-buffer casting |
| glam | Math (Mat4, Vec3, perspective projection) |
| rand | Random orbit parameters |
| image | Load galaxy.png sprite |

## License

MIT
