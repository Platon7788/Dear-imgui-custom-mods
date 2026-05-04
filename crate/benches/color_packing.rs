//! Microbenchmark: color packing from [f32; 4] to u32 RGBA.
//!
//! `pack_color_f32` runs on every `draw.add_rect / add_line / add_text` call.
//! At 60 FPS × hundreds of draw calls per frame × multiple components, this
//! is one of the hottest non-ImGui functions in the library.

use criterion::{Criterion, criterion_group, criterion_main};
use dear_imgui_custom_mod::utils::color::pack_color_f32;
use std::hint::black_box;

fn bench_pack_single(c: &mut Criterion) {
    c.bench_function("pack_color_f32/single", |b| {
        let color = [0.3, 0.6, 1.0, 1.0];
        b.iter(|| pack_color_f32(black_box(color)));
    });
}

fn bench_pack_many(c: &mut Criterion) {
    // Simulates a panel rendering ~100 draw calls per frame, each with a
    // color pack — i.e. one typical frame of nav_panel + status_bar.
    let colors: Vec<[f32; 4]> = (0..100)
        .map(|i| {
            let t = i as f32 / 100.0;
            [t, 1.0 - t, 0.5, 1.0]
        })
        .collect();
    c.bench_function("pack_color_f32/100x", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for c in &colors {
                acc = acc.wrapping_add(pack_color_f32(black_box(*c)));
            }
            black_box(acc)
        });
    });
}

criterion_group!(benches, bench_pack_single, bench_pack_many);
criterion_main!(benches);
