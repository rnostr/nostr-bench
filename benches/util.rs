use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nostr_bench::util::gen_note_event;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("generate event", |b| {
        b.iter(|| gen_note_event(black_box("demo")))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
