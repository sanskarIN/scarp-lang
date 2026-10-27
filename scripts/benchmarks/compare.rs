use std::{hint::black_box, time::Instant};

fn paired(
    mut before: impl FnMut(),
    mut after: impl FnMut(),
    count: usize,
) -> (f64, f64, f64, f64, f64) {
    for _ in 0..10 {
        before();
        after();
    }
    let mut old = Vec::new();
    let mut new = Vec::new();
    let mut ratios = Vec::new();
    for sample in 0..21 {
        let time = |action: &mut dyn FnMut()| {
            let start = Instant::now();
            for _ in 0..count {
                action();
            }
            start.elapsed().as_secs_f64() * 1_000_000.0 / count as f64
        };
        let (b, a) = if sample % 2 == 0 {
            (time(&mut before), time(&mut after))
        } else {
            let a = time(&mut after);
            (time(&mut before), a)
        };
        old.push(b);
        new.push(a);
        ratios.push(b / a);
    }
    old.sort_by(f64::total_cmp);
    new.sort_by(f64::total_cmp);
    ratios.sort_by(f64::total_cmp);
    (old[10], new[10], ratios[10], ratios[1], ratios[19])
}

fn main() {
    let before_uses_var = scarp_before::compile("var x = 0;").is_ok();
    let after_uses_var = scarp_after::compile("var x = 0;").is_ok();
    println!("workload,stage,before_us,after_us,paired_speedup,p05_speedup,p95_speedup");
    for (name, source, expected) in [
        (
            "assignments",
            format!("var x = 0;{}println(x);", "x += 1;".repeat(4096)),
            "4096\n".to_string(),
        ),
        (
            "branches",
            format!(
                "var x = 0;{}println(x);",
                "if x < 20000 and not false { x += 2; } else { x += 1; }".repeat(1024)
            ),
            "2048\n".to_string(),
        ),
        (
            "prints",
            "println(1 + 2 * 3);".repeat(4096),
            "7\n".repeat(4096),
        ),
        (
            "long_expression",
            format!("println({}1);", "1 + ".repeat(4095)),
            "4096\n".to_string(),
        ),
    ] {
        // Only these controlled workloads have this exact declaration prefix.
        let legacy_source = source.replacen("var x = 0;", "let mut x = 0;", 1);
        let before_source = if before_uses_var {
            &source
        } else {
            &legacy_source
        };
        let after_source = if after_uses_var {
            &source
        } else {
            &legacy_source
        };
        let old = scarp_before::compile(before_source).unwrap();
        let new = scarp_after::compile(after_source).unwrap();
        let mut before_output = Vec::new();
        let mut after_output = Vec::new();
        scarp_before::interpreter::execute(&old, &mut before_output).unwrap();
        scarp_after::interpreter::execute(&new, &mut after_output).unwrap();
        assert_eq!(before_output, expected.as_bytes());
        assert_eq!(after_output, expected.as_bytes());
        let compile = paired(
            || {
                black_box(scarp_before::compile(black_box(before_source)).unwrap());
            },
            || {
                black_box(scarp_after::compile(black_box(after_source)).unwrap());
            },
            10,
        );
        let validate = paired(
            || scarp_before::interpreter::validate(black_box(&old)).unwrap(),
            || scarp_after::interpreter::validate(black_box(&new)).unwrap(),
            200,
        );
        let execute = paired(
            || scarp_before::interpreter::execute(black_box(&old), &mut std::io::sink()).unwrap(),
            || scarp_after::interpreter::execute(black_box(&new), &mut std::io::sink()).unwrap(),
            200,
        );
        for (stage, (b, a, ratio, lo, hi)) in [
            ("compile", compile),
            ("validate", validate),
            ("execute", execute),
        ] {
            println!("{name},{stage},{b:.3},{a:.3},{ratio:.3},{lo:.3},{hi:.3}");
        }
    }
}
