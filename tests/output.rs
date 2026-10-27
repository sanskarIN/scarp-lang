use scarp::{
    ast::{OutputTarget, PrintArgument, Statement},
    diagnostics::{location, DiagnosticKind, Span},
    interpreter,
    lexer::{lex, TokenKind},
    ExecutionLimits, RunError,
};
use std::{
    cell::RefCell,
    io::{self, Write},
    rc::Rc,
};

fn output(source: &str) -> (Vec<u8>, Vec<u8>) {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    scarp::run_with_io(source, &mut stdout, &mut stderr).unwrap();
    (stdout, stderr)
}

#[test]
fn stdout_forms_have_exact_bytes_for_empty_single_and_multiple_arguments() {
    for (source, expected) in [
        ("print(\"hi\");", "hi\n"),
        ("println(\"hi\");", "hi\n"),
        ("write(\"hi\");", "hi"),
        ("print();", "\n"),
        ("println();", "\n"),
        ("write();", ""),
        ("print(\"\");", "\n"),
        ("write(\"\");", ""),
        ("print(\"a\", 1, true);", "a 1 true\n"),
        ("println(\"a\", 1, true);", "a 1 true\n"),
        ("write(\"a\", 1, true);", "a 1 true"),
        ("write(\"A\"); print(\"B\");", "AB\n"),
        ("print(\"\", \"x\");", " x\n"),
        ("write(\"\", \"x\");", " x"),
        ("print(\"x\", \"\");", "x \n"),
        ("print(\"one\\ntwo\");", "one\ntwo\n"),
        ("print(\"नमस्ते\", \"世界\");", "नमस्ते 世界\n"),
        ("print(1 + 2 * 3, (1 + 2) * 3);", "7 9\n"),
        ("print(1,);", "1\n"),
        ("print(\n\"a\", 2,\n);", "a 2\n"),
        ("print(1.5, [1.0, 2.0]);", "1.5 [1.0, 2.0]\n"),
        (
            "write(\"A\0B\"); print(-9223372036854775808);",
            "A\0B-9223372036854775808\n",
        ),
    ] {
        assert_eq!(
            output(source),
            (expected.as_bytes().to_vec(), Vec::new()),
            "{source}"
        );
    }
}

#[test]
fn aliases_share_ast_shapes_and_keep_single_argument_compatibility() {
    for body in ["\"hello\"", "1 + 2", "", "1, true,", "\"a\", \"b\""] {
        assert_eq!(
            scarp::parse(&format!("print  ({body});")).unwrap(),
            scarp::parse(&format!("println({body});")).unwrap()
        );
    }
    let syntax = scarp::parse("print(\"literal\",); write(1, 2);").unwrap();
    assert!(
        matches!(&syntax.statements[0], Statement::Println { argument: PrintArgument::String(value), .. } if value == "literal")
    );
    assert!(
        matches!(&syntax.statements[1], Statement::Output { target: OutputTarget::Stdout, newline: false, arguments, .. } if arguments.len() == 2)
    );
}

#[test]
fn all_spellings_reject_empty_middle_arguments_and_require_terminators() {
    for name in ["print", "println", "write", "eprint", "ewrite"] {
        for body in [
            "(,);", "(,1);", "(1,,2);", "(1 2);", "(1)", "(1, 2", "(1, 2,", "(1;", "(1,;",
            "((1,2));",
        ] {
            let source = format!("{name}{body}");
            let error = scarp::compile(&source).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::UnexpectedToken, "{source}");
            assert!(error.span.start <= error.span.end && error.span.end <= source.len());
        }
    }
    assert!(scarp::compile("print(1)")
        .unwrap_err()
        .message
        .contains("';'"));
    assert!(scarp::compile("print(1, 2")
        .unwrap_err()
        .message
        .contains("')'"));
}

#[test]
fn output_names_are_reserved_case_sensitive_and_not_expression_functions() {
    for name in ["print", "println", "write", "eprint", "ewrite"] {
        for source in [format!("bind {name} = 1;"), format!("var {name} = 1;")] {
            assert_eq!(
                scarp::compile(&source).unwrap_err().kind,
                DiagnosticKind::ReservedName
            );
        }
        assert_eq!(
            scarp::compile(&format!("bind x = {name}(1);"))
                .unwrap_err()
                .kind,
            DiagnosticKind::UnknownFunction
        );
    }
    for name in [
        "Print", "PRINTLN", "Write", "Eprint", "prnt", "echo", "say", "out", "puts", "cout",
    ] {
        let error = scarp::compile(&format!("{name}(1);")).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::UnknownFunction);
        assert!(error.help.unwrap().contains("print"));
    }
    assert_eq!(
        output("bind printer = 1; var writer = 2; print(printer, writer);"),
        (b"1 2\n".to_vec(), Vec::new())
    );
}

#[test]
fn comma_spans_and_statement_spans_refer_to_original_unicode_crlf_source() {
    let source = "//🌍\r\nprint(\"π\",,1);";
    let error = scarp::parse(source).unwrap_err();
    assert_eq!(error.span, Span::new(19, 20));
    assert_eq!(
        (
            location(source, error.span.start).line,
            location(source, error.span.start).column
        ),
        (2, 11)
    );
    assert_eq!(
        lex(source)
            .unwrap()
            .iter()
            .filter(|token| token.kind == TokenKind::Comma)
            .count(),
        2
    );
    let source = "//🌍\r\nwrite(\"π\", 1,);";
    let statement = &scarp::parse(source).unwrap().statements[0];
    assert_eq!(statement.span(), Span::new(8, source.len()));
}

#[test]
fn output_parentheses_do_not_reduce_the_expression_nesting_limit() {
    let grouped = format!("{}1{}", "(".repeat(64), ")".repeat(64));
    assert_eq!(
        output(&format!("print({grouped}, 2);")),
        (b"1 2\n".to_vec(), Vec::new())
    );
    assert_eq!(
        scarp::compile(&format!("print(({grouped}));"))
            .unwrap_err()
            .kind,
        DiagnosticKind::ExpressionNesting
    );
}

#[test]
fn arguments_evaluate_left_to_right_with_short_circuiting() {
    for (source, kind) in [
        (
            "print(1 / 0, 9223372036854775807 + 1);",
            DiagnosticKind::DivisionByZero,
        ),
        (
            "print(9223372036854775807 + 1, 1 / 0);",
            DiagnosticKind::ArithmeticOverflow,
        ),
    ] {
        assert_eq!(scarp::check(source).unwrap_err().kind, kind);
    }
    assert_eq!(
        output("print(false and 1 / 0 == 0, true or 1 / 0 == 0);"),
        (b"false true\n".to_vec(), Vec::new())
    );
}

#[test]
fn static_and_runtime_failures_prevent_output_to_both_streams() {
    for suffix in [
        "@",
        "print(missing);",
        "write(1 / 0);",
        "eprint(1, 1 / 0);",
        "if false { eprint(missing); }",
        "print(1 + true);",
    ] {
        let source =
            format!("print(\"before\"); write(\"partial\"); eprint(\"warning\"); {suffix}");
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        assert!(matches!(
            scarp::run_with_io(&source, &mut stdout, &mut stderr),
            Err(RunError::Diagnostic(_))
        ));
        assert!(stdout.is_empty() && stderr.is_empty());
    }
    assert_eq!(
        output("if false { print(1 / 0); } else { write(\"ok\"); }"),
        (b"ok".to_vec(), Vec::new())
    );
}

#[test]
fn snapshots_survive_later_mutation_across_both_streams() {
    assert_eq!(
        output("var x = \"old\"; print(x, x); eprint(x); x = \"new\"; write(x); ewrite(x, x);"),
        (b"old old\nnew".to_vec(), b"old\nnew new".to_vec())
    );
}

#[test]
fn stderr_forms_have_independent_exact_bytes_and_no_diagnostic_prefixes() {
    for (source, expected) in [
        ("eprint();", "\n"),
        ("ewrite();", ""),
        ("eprint(\"warning\", 2, false,);", "warning 2 false\n"),
        ("ewrite(\"A\"); eprint(\"B\");", "AB\n"),
        ("ewrite(\"\", \"x\");", " x"),
        ("eprint(\"世界\\nनमस्ते\");", "世界\nनमस्ते\n"),
    ] {
        assert_eq!(output(source), (Vec::new(), expected.as_bytes().to_vec()));
    }
}

#[test]
fn legacy_single_writer_apis_reject_required_stderr_before_any_stdout() {
    let source = "write(\"before\"); eprint(\"warning\");";
    let program = scarp::compile(source).unwrap();
    let mut stdout = Vec::new();
    for failure in [
        scarp::run(source, &mut stdout).unwrap_err(),
        scarp::run_with_limits(source, &mut stdout, ExecutionLimits::default()).unwrap_err(),
        interpreter::execute(&program, &mut stdout).unwrap_err(),
        interpreter::execute_with_limits(&program, &mut stdout, ExecutionLimits::default())
            .unwrap_err(),
    ] {
        let RunError::Output(error) = failure else {
            panic!("{failure}")
        };
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(error.to_string().contains("stderr destination"));
    }
    assert!(stdout.is_empty());
}

#[test]
fn single_writer_allows_unselected_stderr_and_zero_byte_raw_writes() {
    let mut stdout = Vec::new();
    scarp::run("if false { eprint(\"skip\"); } ewrite(); ewrite(\"\"); bind empty = \"\"; ewrite(empty); print(\"ok\");", &mut stdout).unwrap();
    assert_eq!(stdout, b"ok\n");
    let mut stdout = Vec::new();
    assert!(matches!(
        scarp::run("write(\"before\"); ewrite(\"\", \"\");", &mut stdout),
        Err(RunError::Output(_))
    ));
    assert!(stdout.is_empty());
}

#[test]
fn evaluation_errors_take_precedence_over_a_missing_stderr_destination() {
    let mut output = Vec::new();
    let failure = scarp::run(
        "print(\"before\"); eprint(\"warning\"); print(1 / 0);",
        &mut output,
    )
    .unwrap_err();
    assert!(
        matches!(failure, RunError::Diagnostic(error) if error.kind == DiagnosticKind::DivisionByZero)
    );
    assert!(output.is_empty());
}

#[test]
fn validation_evaluates_every_selected_argument_without_requiring_destinations() {
    let source = "write(\"x\"); print(1, true); eprint(\"warning\"); ewrite();";
    scarp::check(source).unwrap();
    interpreter::validate(&scarp::compile(source).unwrap()).unwrap();
    assert_eq!(
        scarp::check("eprint(1, 1 / 0);").unwrap_err().kind,
        DiagnosticKind::DivisionByZero
    );
}

#[test]
fn output_statement_and_separator_budgets_match_in_validation_and_execution() {
    for source in ["print();", "write();", "eprint();", "ewrite();"] {
        let limits = ExecutionLimits {
            max_steps: 100,
            max_prints: 0,
        };
        assert_eq!(
            scarp::check_with_limits(source, limits).unwrap_err().kind,
            DiagnosticKind::OutputLimit
        );
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        assert!(
            matches!(scarp::run_with_io_and_limits(source, &mut stdout, &mut stderr, limits), Err(RunError::Diagnostic(error)) if error.kind == DiagnosticKind::OutputLimit)
        );
        assert!(stdout.is_empty() && stderr.is_empty());
    }
    let source = "print(\"a\", \"b\", \"c\");";
    let limits = ExecutionLimits {
        max_steps: 3,
        max_prints: 1,
    };
    scarp::check_with_limits(source, limits).unwrap();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    scarp::run_with_io_and_limits(source, &mut stdout, &mut stderr, limits).unwrap();
    assert_eq!(stdout, b"a b c\n");
    assert!(stderr.is_empty());
    assert_eq!(
        scarp::check_with_limits(
            source,
            ExecutionLimits {
                max_steps: 2,
                ..limits
            }
        )
        .unwrap_err()
        .kind,
        DiagnosticKind::ExecutionLimit
    );
}

struct LoggedWriter {
    name: &'static str,
    log: Rc<RefCell<Vec<String>>>,
}
impl Write for LoggedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.log
            .borrow_mut()
            .push(format!("{}:{}", self.name, String::from_utf8_lossy(bytes)));
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.log.borrow_mut().push(format!("{}:flush", self.name));
        Ok(())
    }
}

#[test]
fn replay_keeps_source_order_and_flushes_only_at_nonempty_stream_switches() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut stdout = LoggedWriter {
        name: "out",
        log: log.clone(),
    };
    let mut stderr = LoggedWriter {
        name: "err",
        log: log.clone(),
    };
    scarp::run_with_io(
        "write(\"A\"); ewrite(\"\"); ewrite(\"B\"); write(\"C\");",
        &mut stdout,
        &mut stderr,
    )
    .unwrap();
    assert_eq!(
        *log.borrow(),
        ["out:A", "out:flush", "err:B", "err:flush", "out:C"]
    );
}

struct FailingWriter {
    bytes: Vec<u8>,
    remaining: usize,
    fail_flush: bool,
}
impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let count = bytes.len().min(self.remaining);
        self.bytes.extend_from_slice(&bytes[..count]);
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(io::ErrorKind::Other.into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn either_writer_can_fail_partway_through_replay() {
    for target in [OutputTarget::Stdout, OutputTarget::Stderr] {
        let mut failing = FailingWriter {
            bytes: Vec::new(),
            remaining: 1,
            fail_flush: false,
        };
        let mut other = Vec::new();
        let result = match target {
            OutputTarget::Stdout => scarp::run_with_io(
                "ewrite(\"earlier\"); print(\"abc\");",
                &mut failing,
                &mut other,
            ),
            OutputTarget::Stderr => scarp::run_with_io(
                "write(\"earlier\"); eprint(\"abc\");",
                &mut other,
                &mut failing,
            ),
        };
        assert!(
            matches!(result, Err(RunError::Output(error)) if error.kind() == io::ErrorKind::BrokenPipe)
        );
        assert_eq!(failing.bytes, b"a");
        assert_eq!(other, b"earlier");
    }
}

#[test]
fn boundary_flush_failures_stop_before_writing_the_next_destination() {
    for first_is_error in [false, true] {
        let mut first = FailingWriter {
            bytes: Vec::new(),
            remaining: 100,
            fail_flush: true,
        };
        let mut next = Vec::new();
        let result = if first_is_error {
            scarp::run_with_io("ewrite(\"first\"); write(\"next\");", &mut next, &mut first)
        } else {
            scarp::run_with_io("write(\"first\"); ewrite(\"next\");", &mut first, &mut next)
        };
        assert!(matches!(result, Err(RunError::Output(_))));
        assert_eq!(first.bytes, b"first");
        assert!(next.is_empty());
    }
}

#[test]
fn large_flat_output_and_wide_argument_lists_remain_iterative() {
    let flat = "write(\"x\");".repeat(10_000);
    assert_eq!(output(&flat), (vec![b'x'; 10_000], Vec::new()));
    let wide = format!("print({});", "\"x\",".repeat(4096));
    let expected = format!("{}x\n", "x ".repeat(4095)).into_bytes();
    assert_eq!(output(&wide), (expected, Vec::new()));
}

#[test]
fn repeated_dual_stream_execution_has_fresh_storage() {
    let program =
        scarp::compile("var count = 1; print(count, count + 1); count += 2; eprint(count);")
            .unwrap();
    for _ in 0..3 {
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        interpreter::execute_with_io(&program, &mut stdout, &mut stderr).unwrap();
        assert_eq!(stdout, b"1 2\n");
        assert_eq!(stderr, b"3\n");
    }
}

#[test]
fn interpolation_and_macro_syntax_remain_unsupported_and_braces_literal() {
    assert_eq!(
        output("print(\"Hello, {name}! %s\");"),
        (b"Hello, {name}! %s\n".to_vec(), Vec::new())
    );
    assert!(scarp::compile("print(f\"Hello, {name}!\");").is_err());
    assert!(scarp::compile("print!(\"hello\");").is_err());
}
