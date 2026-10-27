use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind as Kind},
    ExecutionLimits, RunError,
};

fn output(source: &str) -> String {
    let mut out = Vec::new();
    scarp::run(source, &mut out).unwrap();
    scarp::check(source).unwrap();
    String::from_utf8(out).unwrap()
}
fn error(source: &str, limits: ExecutionLimits) -> Diagnostic {
    let expected = scarp::check_with_limits(source, limits).unwrap_err();
    let mut out = Vec::new();
    let RunError::Diagnostic(actual) =
        scarp::run_with_limits(source, &mut out, limits).unwrap_err()
    else {
        panic!()
    };
    assert!(out.is_empty());
    assert_eq!(actual, expected);
    actual
}

#[test]
fn zero_one_and_multiple_counts_use_exclusive_zero_based_indices() {
    assert_eq!(output("repeat 0 as i { println(1/0); } repeat 1 as i { println(i); } var total=0; repeat 5 as index { total+=index; } println(total);"),"0\n10\n");
}

#[test]
fn count_is_a_snapshot_evaluated_once_before_index_binding() {
    assert_eq!(
        output("var n=4; repeat n+1 as index { n=0; println(index); } println(n);"),
        "0\n1\n2\n3\n4\n0\n"
    );
    assert_eq!(
        output("bind n=3; repeat n as n { println(n); } println(n);"),
        "0\n1\n2\n3\n"
    );
    assert_eq!(
        output("repeat 4 as i { repeat i as j { println(i*10+j); } }"),
        "10\n20\n21\n30\n31\n32\n"
    );
}

#[test]
fn break_and_continue_advance_only_the_nearest_repeat() {
    assert_eq!(
        output("repeat 4 as i { if i==1 { continue; } if i==3 { break; } { println(i); } }"),
        "0\n2\n"
    );
    assert_eq!(output("repeat 3 as i { repeat 5 as j { if j==1 {continue;} if j==3 {break;} println(i*10+j); } }"),"0\n2\n10\n12\n20\n22\n");
}

#[test]
fn mixed_while_repeat_control_targets_the_correct_loop() {
    assert_eq!(output("var x=0; while x<3 { x+=1; repeat 4 as i { if i==1 {continue;} if i==3 {break;} println(x*10+i); } }"),"10\n12\n20\n22\n30\n32\n");
    assert_eq!(output("repeat 3 as i { var j=0; while true { j+=1; if j==1 {continue;} if j==3 {break;} println(i*10+j); } }"),"2\n12\n22\n");
    assert_eq!(
        output("repeat 4 as i { if i==1 {continue;} elif i==3 {break;} else {println(i);} }"),
        "0\n2\n"
    );
}

#[test]
fn index_and_per_iteration_locals_follow_existing_scope_rules() {
    assert_eq!(output("bind index=99; repeat 3 as index { var copy=index; copy+=10; println(copy); { bind index=8; println(index); } } println(index);"),"10\n8\n11\n8\n12\n8\n99\n");
    for source in ["repeat 1 as i { i=2; }", "repeat 0 as i { i+=1; }"] {
        let error = error(source, ExecutionLimits::default());
        assert_eq!(error.kind, Kind::ImmutableAssignment);
        assert_eq!(error.related.len(), 1);
    }
    assert_eq!(
        error("repeat 0 as i { bind i=1; }", ExecutionLimits::default()).kind,
        Kind::DuplicateBinding
    );
    for source in [
        "repeat 1 as i {} println(i);",
        "repeat 0 as i { bind local=1; } println(local);",
    ] {
        assert_eq!(
            error(source, ExecutionLimits::default()).kind,
            Kind::UnknownName
        );
    }
}

#[test]
fn contextual_words_preserve_ordinary_bindings_and_assignments() {
    assert_eq!(output("var repeat=2; repeat+=1; bind as=repeat; repeat as as repeat { println(repeat); } println(as); println(repeat);"),"0\n1\n2\n3\n3\n");
    assert_eq!(
        output("bind as=2; repeat as as as {println(as);}"),
        "0\n1\n"
    );
    assert_eq!(
        output("bind Repeat=1; bind repeatable=2; println(Repeat+repeatable);"),
        "3\n"
    );
    assert_eq!(
        error("repeat 1 as while {}", ExecutionLimits::default()).kind,
        Kind::ReservedName
    );
}

#[test]
fn count_type_sign_and_unused_body_are_checked() {
    for source in ["repeat true as i {}", "repeat \"2\" as i {}"] {
        assert_eq!(
            error(source, ExecutionLimits::default()).kind,
            Kind::TypeMismatch
        );
    }
    let source = "println(\"hidden\"); repeat -(1+2) as i {}";
    let diagnostic = error(source, ExecutionLimits::default());
    assert_eq!(diagnostic.kind, Kind::RepeatCount);
    assert_eq!(
        &source[diagnostic.span.start..diagnostic.span.end],
        "-(1+2)"
    );
    assert!(diagnostic.help.unwrap().contains("zero"));
    assert_eq!(
        error(
            "repeat 0 as i { println(missing); }",
            ExecutionLimits::default()
        )
        .kind,
        Kind::UnknownName
    );
    assert_eq!(
        error(
            "repeat 0 as i { println(true+1); }",
            ExecutionLimits::default()
        )
        .kind,
        Kind::TypeMismatch
    );
}

#[test]
fn huge_counts_are_bounded_without_range_allocation_or_overflow() {
    let small = ExecutionLimits {
        max_steps: 100,
        max_prints: 10,
    };
    for source in [
        "repeat 9223372036854775807 as i {}",
        "repeat 9223372036854775807 as i {continue;}",
        "println(\"hidden\"); repeat 999999999 as i { repeat 999999999 as j {} }",
    ] {
        assert_eq!(error(source, small).kind, Kind::ExecutionLimit);
    }
    assert_eq!(
        output("repeat 9223372036854775807 as i { println(i); break; }"),
        "0\n"
    );
    assert_eq!(
        error("repeat -9223372036854775808 as i {}", small).kind,
        Kind::RepeatCount
    );
}

#[test]
fn exact_step_and_print_budgets_agree_between_execution_and_validation() {
    for n in 0..4 {
        let source = format!("repeat {n} as i {{}}");
        let limits = ExecutionLimits {
            max_steps: 3 + 3 * n,
            max_prints: 0,
        };
        scarp::check_with_limits(&source, limits).unwrap();
        assert_eq!(
            error(
                &source,
                ExecutionLimits {
                    max_steps: limits.max_steps - 1,
                    ..limits
                }
            )
            .kind,
            Kind::ExecutionLimit
        );
    }
    scarp::check_with_limits(
        "",
        ExecutionLimits {
            max_steps: 0,
            max_prints: 0,
        },
    )
    .unwrap();
    assert_eq!(
        error(
            "repeat 0 as i {}",
            ExecutionLimits {
                max_steps: 0,
                max_prints: 0
            }
        )
        .kind,
        Kind::ExecutionLimit
    );
    let source = "repeat 3 as i {println(i);}";
    scarp::check_with_limits(
        source,
        ExecutionLimits {
            max_steps: 100,
            max_prints: 3,
        },
    )
    .unwrap();
    assert_eq!(
        error(
            source,
            ExecutionLimits {
                max_steps: 100,
                max_prints: 2
            }
        )
        .kind,
        Kind::OutputLimit
    );
}

#[test]
fn loops_have_fixed_storage_and_reusable_parallel_execution() {
    let program = scarp::compile(
        "var total=0; repeat 100000 as i { bind value=i+1; total+=value; } println(total);",
    )
    .unwrap();
    assert_eq!(program.binding_count(), 4);
    let limits = ExecutionLimits {
        max_steps: 2000000,
        max_prints: 1,
    };
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let program = &program;
            scope.spawn(move || {
                for _ in 0..2 {
                    let mut out = Vec::new();
                    scarp::interpreter::execute_with_limits(program, &mut out, limits).unwrap();
                    assert_eq!(out, b"5000050000\n");
                }
            });
        }
    });
}

#[test]
fn malformed_syntax_and_nesting_boundaries_are_precise() {
    for source in [
        "repeat",
        "repeat 2 {}",
        "repeat 2 as {}",
        "repeat 2 as i",
        "repeat 2 as i {",
        "repeat 1 as i {};",
        "repeat 1 as i { continue }",
    ] {
        assert!(scarp::compile(source).is_err(), "{source}");
    }
    let valid = format!(
        "{}println(1);{}",
        "repeat 0 as i {".repeat(64),
        "}".repeat(64)
    );
    scarp::check(&valid).unwrap();
    let invalid = format!(
        "{}println(1);{}",
        "repeat 0 as i {".repeat(65),
        "}".repeat(65)
    );
    assert_eq!(
        error(&invalid, ExecutionLimits::default()).kind,
        Kind::BlockNesting
    );
}

#[test]
fn original_unicode_crlf_spans_cover_count_index_and_statement() {
    let source = "// café 🌍\r\nrepeat 2+1 as index { println(index); }";
    let program = scarp::parse(source).unwrap();
    let scarp::ast::Statement::Repeat {
        count,
        index_span,
        span,
        ..
    } = &program.statements[0]
    else {
        panic!()
    };
    assert_eq!(
        &source[count.nodes()[count.root()].span.start..count.nodes()[count.root()].span.end],
        "2+1"
    );
    assert_eq!(&source[index_span.start..index_span.end], "index");
    assert_eq!(
        &source[span.start..span.end],
        "repeat 2+1 as index { println(index); }"
    );
    assert_eq!(
        scarp::diagnostics::location(source, span.start),
        scarp::diagnostics::Location { line: 2, column: 1 }
    );
}

#[test]
fn deterministic_counted_programs_match_an_independent_reference() {
    for n in 0..100 {
        let source=format!("var total=0; repeat {n} as i {{ if i==3 {{continue;}} if i==9 {{break;}} total+=i; }} println(total);");
        let want: i64 = (0..n).take_while(|i| *i != 9).filter(|i| *i != 3).sum();
        assert_eq!(output(&source), format!("{want}\n"));
    }
}

#[test]
fn truncation_corpus_terminates_and_retains_valid_spans() {
    let source = "// π\r\nrepeat (2+1) as i { if i==1 {continue;} println(i); }";
    for (end, _) in source.char_indices() {
        if let Err(error) = scarp::compile(&source[..end]) {
            assert!(error.span.start <= error.span.end && error.span.end <= end);
            assert!(
                source.is_char_boundary(error.span.start)
                    && source.is_char_boundary(error.span.end)
            );
        }
    }
}
