//! Microbenchmark: `Theme` sub-palette dispatch.
//!
//! These methods are called from every component's render path every frame.
//! Any regression here scales by (fps × component_count) — catch it early.

use criterion::{Criterion, criterion_group, criterion_main};
use dear_imgui_custom_mod::theme::Theme;
use std::hint::black_box;

fn bench_titlebar(c: &mut Criterion) {
    c.bench_function("theme::titlebar/Dark", |b| {
        b.iter(|| black_box(Theme::Dark).titlebar());
    });
    c.bench_function("theme::titlebar/all-variants", |b| {
        b.iter(|| {
            for t in Theme::ALL {
                black_box(black_box(*t).titlebar());
            }
        });
    });
}

fn bench_nav(c: &mut Criterion) {
    c.bench_function("theme::nav/Dark", |b| {
        b.iter(|| black_box(Theme::Dark).nav());
    });
}

fn bench_dialog(c: &mut Criterion) {
    c.bench_function("theme::dialog/Dark", |b| {
        b.iter(|| black_box(Theme::Dark).dialog());
    });
}

fn bench_statusbar(c: &mut Criterion) {
    c.bench_function("theme::statusbar/Dark", |b| {
        b.iter(|| black_box(Theme::Dark).statusbar());
    });
}

fn bench_next(c: &mut Criterion) {
    c.bench_function("theme::next", |b| {
        b.iter(|| black_box(Theme::Dark).next());
    });
}

criterion_group!(benches, bench_titlebar, bench_nav, bench_dialog, bench_statusbar, bench_next);
criterion_main!(benches);
