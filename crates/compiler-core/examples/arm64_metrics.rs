use std::{collections::BTreeMap, process::ExitCode};

use compiler_core::compile_arm64;

const SOURCE: &str = include_str!("../../../docs/language/examples/arm64-optimization.itd");
const OPERATION_VARIANT_CONTRACT: usize = 30;

#[derive(Default)]
struct FunctionMetrics {
    static_instructions: usize,
    loads: usize,
    stores: usize,
    frame_bytes: usize,
    prologue_open: bool,
}

fn is_function_label(line: &str, program_labels: &[String]) -> Option<String> {
    let label = line.strip_suffix(':')?;
    if program_labels.iter().any(|candidate| candidate == label)
        || (label.starts_with("_itd_") && !label.starts_with("_itd_f"))
    {
        Some(label.to_owned())
    } else {
        None
    }
}

fn instruction(line: &str) -> Option<&str> {
    let line = line.trim();
    (!line.is_empty() && !line.starts_with('.') && !line.ends_with(':') && !line.starts_with('#'))
        .then_some(line)
}

fn mnemonic(line: &str) -> &str {
    line.split_whitespace().next().unwrap_or_default()
}

fn immediate_stack_subtraction(line: &str) -> Option<usize> {
    let value = line.strip_prefix("sub sp, sp, #")?;
    let (value, scale) = match value.split_once(", lsl #12") {
        Some((value, _)) => (value, 4096),
        None => (value, 1),
    };
    value.parse::<usize>().ok()?.checked_mul(scale)
}

fn collect_metrics(
    assembly: &str,
    program_labels: Vec<String>,
) -> BTreeMap<String, FunctionMetrics> {
    let mut metrics = BTreeMap::new();
    let mut in_text = false;
    let mut current = None;

    for line in assembly.lines() {
        if line.starts_with(".section ") {
            in_text = line.contains("__TEXT,__text");
            continue;
        }
        if in_text && let Some(label) = is_function_label(line, &program_labels) {
            metrics
                .entry(label.clone())
                .or_insert_with(|| FunctionMetrics {
                    prologue_open: true,
                    ..FunctionMetrics::default()
                });
            current = Some(label);
            continue;
        }
        let Some(current) = current.as_ref() else {
            continue;
        };
        let Some(line) = instruction(line) else {
            continue;
        };
        let metric = metrics.get_mut(current).expect("current function metric");
        metric.static_instructions += 1;
        match mnemonic(line) {
            mnemonic if mnemonic.starts_with("ldr") || mnemonic == "ldp" => metric.loads += 1,
            mnemonic if mnemonic.starts_with("str") || mnemonic == "stp" => metric.stores += 1,
            _ => {}
        }
        if metric.prologue_open {
            if let Some(bytes) = immediate_stack_subtraction(line) {
                metric.frame_bytes = bytes;
            }
            if !matches!(mnemonic(line), "stp" | "mov" | "sub") {
                metric.prologue_open = false;
            }
        }
    }
    metrics
}

fn main() -> ExitCode {
    let result = compile_arm64(SOURCE);
    if !result.diagnostics.is_empty() {
        for diagnostic in result.diagnostics {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
        }
        return ExitCode::FAILURE;
    }
    let Some(assembly) = result.assembly else {
        eprintln!("ARM64 assembly was not produced");
        return ExitCode::FAILURE;
    };

    let quadruples = compiler_core::compile_quadruples(SOURCE)
        .program
        .expect("ARM64 compilation already succeeded");
    let program_labels = quadruples
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| format!("_itd_f{index} ({})", function.name))
        .collect::<Vec<_>>();
    let raw_program_labels = program_labels
        .iter()
        .map(|label| label.split_once(' ').expect("label name").0.to_owned())
        .collect();

    println!("ARM64 static metrics for docs/language/examples/arm64-optimization.itd");
    println!("ir_operation_variants={OPERATION_VARIANT_CONTRACT}");
    println!("decimal_semantics=VM exact decimal; ARM64 opt-in backend f64 IEEE-754");
    println!(
        "caveat=metrics count emitted static instructions, not executed instructions or dynamic operands"
    );
    for (label, metric) in collect_metrics(&assembly, raw_program_labels) {
        let kind = if label.starts_with("_itd_f") {
            "program"
        } else {
            "runtime"
        };
        println!(
            "function kind={kind} label={label} static_instructions={} loads={} stores={} frame_bytes={}",
            metric.static_instructions, metric.loads, metric.stores, metric.frame_bytes
        );
    }
    println!("--- assembly ---");
    print!("{assembly}");

    debug_assert_eq!(OPERATION_VARIANT_CONTRACT, 30);
    ExitCode::SUCCESS
}
