alias r := run
alias rr := run-release

run:
    cargo run

run-release:
    cargo run --release

# Run all tests
test:
    cargo test

# Run tests in release mode (faster for computationally heavy tests)
test-release:
    cargo test --release

# Run benchmarks
bench:
    cargo bench --bench sim_perf

# Save a baseline for comparison
# Usage: just bench-baseline before
bench-baseline name:
    cargo bench --bench sim_perf -- --save-baseline {{name}}

# Compare against a saved baseline
# Usage: just bench-compare before
bench-compare name:
    cargo bench --bench sim_perf -- --baseline {{name}}

# Run benchmarks and generate HTML report
bench-report:
    cargo bench --bench sim_perf && open target/criterion/report/index.html
