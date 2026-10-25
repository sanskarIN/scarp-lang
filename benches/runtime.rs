//! Dependency-free measurements of separate compilation and execution stages.

use std::{hint::black_box, time::Instant};

fn median_us(repetitions: usize, samples: usize, mut action: impl FnMut()) -> f64 {
    for _ in 0..3 {
        action();
    }
    let mut times = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        for _ in 0..repetitions {
            action();
        }
        times.push(start.elapsed().as_secs_f64() * 1_000_000.0 / repetitions as f64);
    }
    times.sort_by(|left, right| left.partial_cmp(right).unwrap());
    times[samples / 2]
}

fn main() {
    let mut quick = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--quick" => quick = true,
            "--bench" => {}
            _ => {
                eprintln!("Usage: cargo bench --bench runtime -- [--quick]");
                std::process::exit(2);
            }
        }
    }
    let (compile_repetitions, runtime_repetitions, samples) =
        if quick { (3, 20, 3) } else { (10, 200, 7) };
    eprintln!("Median microseconds per call; {samples} samples; compile batch={compile_repetitions}, runtime batch={runtime_repetitions}; arch={}, os={}", std::env::consts::ARCH, std::env::consts::OS);
    println!(
        "workload,compile_us,validate_us,execute_us,source_bytes,operations,bindings,output_bytes"
    );
    let assignments = format!("let mut x = 0;{}println(x);", "x += 1;".repeat(4096));
    let branches = format!(
        "let mut x = 0;{}println(x);",
        "if x < 20000 and not false { x += 2; } else { x += 1; }".repeat(1024)
    );
    let prints = "println(1 + 2 * 3);".repeat(4096);
    let chain = format!("println({}1);", "1 + ".repeat(4095));
    for (name, source, expected) in [
        ("assignments", assignments, "4096\n".to_string()),
        ("branches", branches, "2048\n".to_string()),
        ("prints", prints, "7\n".repeat(4096)),
        ("long_expression", chain, "4096\n".to_string()),
    ] {
        let program = scarp::compile(&source).unwrap();
        let mut actual = Vec::new();
        scarp::interpreter::execute(&program, &mut actual).unwrap();
        assert_eq!(actual, expected.as_bytes(), "benchmark workload {name}");
        let compile = median_us(compile_repetitions, samples, || {
            black_box(scarp::compile(black_box(&source)).unwrap());
        });
        let validate = median_us(runtime_repetitions, samples, || {
            scarp::interpreter::validate(black_box(&program)).unwrap();
        });
        let execute = median_us(runtime_repetitions, samples, || {
            scarp::interpreter::execute(black_box(&program), &mut std::io::sink()).unwrap();
        });
        println!(
            "{name},{compile:.3},{validate:.3},{execute:.3},{},{},{},{}",
            source.len(),
            program.statement_count(),
            program.binding_count(),
            actual.len()
        );
    }
}
