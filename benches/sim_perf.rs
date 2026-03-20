use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use hopalong_visoxide::sim::{
    DEFAULT_FOV, DEFAULT_LEVELS, DEFAULT_POINTS_SUBSET, DEFAULT_ROTATION_SPEED, DEFAULT_SPEED,
    DEFAULT_SUBSETS, HopalongSim, OrbitParams, SimSettings, generate_orbit, hsv_to_rgba,
};

/// Benchmark orbit generation at different scales
fn bench_generate_orbit(c: &mut Criterion) {
    let mut group = c.benchmark_group("generate_orbit");

    let params = OrbitParams {
        a: 0.0,
        b: 1.0,
        c: 10.0,
        d: 5.0,
        e: 6.0,
    };

    // Test different sizes
    let sizes = [
        (1, 1000, "small"),
        (5, 4000, "medium"),
        (10, 10000, "large"),
    ];

    for (subsets, points, name) in sizes {
        group.bench_with_input(
            BenchmarkId::new(name, format!("{}x{}", subsets, points)),
            &(subsets, points),
            |b, (s, p)| {
                b.iter(|| generate_orbit(black_box(&params), *s, *p));
            },
        );
    }

    group.finish();
}

/// Benchmark instance building from simulation state
fn bench_build_instances(c: &mut Criterion) {
    let mut group = c.benchmark_group("build_instances");

    // Create a simulation with default settings
    let sim = HopalongSim::new();

    group.bench_function("default_196k_particles", |b| {
        b.iter(|| {
            let instances = hopalong_visoxide::renderer::build_instances(black_box(&sim));
            black_box(instances);
        });
    });

    // Test with smaller configuration
    let mut small_sim = HopalongSim::new();
    small_sim.settings.points_per_subset = 1000;
    small_sim.settings.subset_count = 3;
    small_sim.settings.level_count = 3;
    small_sim.full_rebuild();

    group.bench_function("small_9k_particles", |b| {
        b.iter(|| {
            let instances = hopalong_visoxide::renderer::build_instances(black_box(&small_sim));
            black_box(instances);
        });
    });

    group.finish();
}

/// Benchmark simulation update step
fn bench_sim_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("sim_update");
    let dt = 1.0 / 60.0; // 60fps

    // Use iter_batched to create fresh sim for each iteration
    // This prevents state mutation from leaking across iterations
    group.bench_function("single_step_196k", |b| {
        b.iter_batched(
            || HopalongSim::new(),
            |mut sim| {
                sim.update(black_box(dt));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Benchmark 60 frames (1 second) - fresh sim each iteration
    group.bench_function("60_frames_196k", |b| {
        b.iter_batched(
            || HopalongSim::new(),
            |mut sim| {
                for _ in 0..60 {
                    sim.update(black_box(dt));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark HSV to RGBA conversion
fn bench_hsv_to_rgba(c: &mut Criterion) {
    let mut group = c.benchmark_group("hsv_to_rgba");

    // Test different hues across the spectrum
    let hues: Vec<f32> = (0..12).map(|i| i as f32 / 12.0).collect();

    group.bench_function("many_conversions", |b| {
        b.iter(|| {
            for hue in &hues {
                black_box(hsv_to_rgba(*hue, 0.8, 1.0));
            }
        });
    });

    group.finish();
}

/// Benchmark full rebuild (expensive operation)
fn bench_full_rebuild(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_rebuild");

    // Use iter_batched to ensure fair measurement of rebuild cost
    group.bench_function("default_196k", |b| {
        b.iter_batched(
            || HopalongSim::new(),
            |mut sim| {
                sim.full_rebuild();
                black_box(&sim);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Test with smaller configuration
    group.bench_function("small_9k", |b| {
        b.iter_batched(
            || {
                let mut sim = HopalongSim::new();
                sim.settings.points_per_subset = 1000;
                sim.settings.subset_count = 3;
                sim.settings.level_count = 3;
                sim
            },
            |mut sim| {
                sim.full_rebuild();
                black_box(&sim);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_generate_orbit,
    bench_build_instances,
    bench_sim_update,
    bench_hsv_to_rgba,
    bench_full_rebuild
);
criterion_main!(benches);
