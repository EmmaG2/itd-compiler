use compiler_core::{
    analyze,
    bytecode::compile,
    runtime::{RuntimeLimits, execute_chunk, execute_reference},
};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn median(mut action: impl FnMut()) -> Duration {
    for _ in 0..3 {
        action();
    }
    let mut samples: Vec<_> = (0..21)
        .map(|_| {
            let start = Instant::now();
            action();
            start.elapsed()
        })
        .collect();
    samples.sort();
    samples[samples.len() / 2]
}
fn main() {
    println!("case,analyze_us,compile_us,reference_us,vm_us,speedup");
    for (name, source) in [
        (
            "arithmetic",
            "let mut i := 0; let mut total := 0; while i < 5000 { total = total + i; i = i + 1; }",
        ),
        (
            "calls",
            "fn add(x: int) -> int { return x+1; } let mut i := 0; while i < 2000 { i = add(i); }",
        ),
        (
            "recursion",
            "fn fib(n: int) -> int { if n < 2 { return n; } return fib(n-1)+fib(n-2); } fib(15);",
        ),
        (
            "decimals",
            "let mut i := 0; let mut total := 0dec; while i < 2000 { total = total + 0.01dec; i = i + 1; }",
        ),
        (
            "methods",
            "class Counter { let mut n: int := 0; pub fn this() {} pub fn add() -> int { this.n = this.n+1; return this.n; } } let counter := Counter(); let mut i := 0; while i < 2000 { i = counter.add(); }",
        ),
    ] {
        let analysis = analyze(source);
        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
        let semantic = analysis.semantic.as_ref().unwrap();
        let chunk = compile(&analysis.program, semantic).unwrap();
        let reference = execute_reference(&analysis.program, semantic);
        let vm = execute_chunk(&chunk, semantic, RuntimeLimits::default(), None);
        assert_eq!(reference, vm);
        assert!(vm.diagnostics.is_empty());
        let analysis_time = median(|| {
            black_box(analyze(black_box(source)));
        });
        let compile_time = median(|| {
            black_box(compile(black_box(&analysis.program), semantic).unwrap());
        });
        let reference_time = median(|| {
            black_box(execute_reference(black_box(&analysis.program), semantic));
        });
        let vm_time = median(|| {
            black_box(execute_chunk(
                black_box(&chunk),
                semantic,
                RuntimeLimits::default(),
                None,
            ));
        });
        println!(
            "{name},{},{},{},{},{:.2}",
            analysis_time.as_micros(),
            compile_time.as_micros(),
            reference_time.as_micros(),
            vm_time.as_micros(),
            reference_time.as_secs_f64() / vm_time.as_secs_f64()
        );
    }
}
