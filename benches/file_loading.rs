use criterion::{Criterion, criterion_group, criterion_main};

fn benchmark_placeholder(c: &mut Criterion) {
    c.bench_function("placeholder", |b| b.iter(|| 1 + 1));
}

criterion_group!(benches, benchmark_placeholder);
criterion_main!(benches);
