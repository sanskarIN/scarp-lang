use scarp::{
    ast::{Program, Statement},
    compile,
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    interpreter,
    parser::{MAX_BLOCK_DEPTH, MAX_EXPRESSION_DEPTH},
    RunError,
};

fn output(source: &str) -> String {
    let mut bytes = Vec::new();
    scarp::run(source, &mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn error(source: &str) -> Diagnostic {
    let mut bytes = Vec::new();
    let error = scarp::run(source, &mut bytes).unwrap_err();
    assert!(bytes.is_empty());
    match error {
        RunError::Diagnostic(error) => error,
        other => panic!("{other}"),
    }
}

#[test]
fn shadowing_initializers_read_outer_values_and_scope_exit_restores_visibility() {
    assert_eq!(output("let value = 1; { let value = value + 1; println(value); { let value = value + 1; println(value); } println(value); } println(value);"), "2\n3\n2\n1\n");
}

#[test]
fn assignment_changes_the_nearest_visible_mutable_binding() {
    assert_eq!(output("let mut x = 1; { x += 1; let mut x = x + 10; x += 1; println(x); { x += 1; } println(x); } println(x);"), "13\n14\n2\n");
}

#[test]
fn immutable_inner_bindings_hide_mutable_outer_bindings() {
    let source = "let mut x = 1; { let x = 2; x += 1; }";
    let diagnostic = error(source);
    assert_eq!(diagnostic.kind, DiagnosticKind::ImmutableAssignment);
    assert_eq!(diagnostic.related.len(), 1);
    assert_eq!(
        diagnostic.related[0].span.start,
        source.find("x = 2").unwrap()
    );
}

#[test]
fn mutable_inner_bindings_can_shadow_immutable_outer_bindings() {
    assert_eq!(
        output("let x = 1; { let mut x = 2; x *= 3; println(x); } println(x);"),
        "6\n1\n"
    );
}

#[test]
fn siblings_use_distinct_slots_and_cannot_see_each_others_names() {
    let program = compile(
        "let outer = 1; { let x = 2; println(x); } { let x = 3; println(x); } println(outer);",
    )
    .unwrap();
    assert_eq!(program.binding_count(), 3);
    assert_eq!(program.statement_count(), 6);
    let mut bytes = Vec::new();
    interpreter::execute(&program, &mut bytes).unwrap();
    assert_eq!(bytes, b"2\n3\n1\n");
    assert_eq!(
        error("{ let x = 1; } { println(x); }").kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn duplicate_declarations_are_rejected_only_within_the_current_scope() {
    let source = "let x = 0; { let x = 1; let x = 2; }";
    let diagnostic = error(source);
    assert_eq!(diagnostic.kind, DiagnosticKind::DuplicateBinding);
    assert_eq!(
        diagnostic.related[0].span.start,
        source.find("x = 1").unwrap()
    );
    assert_eq!(output("{ let x = 1; } let x = 2; println(x);"), "2\n");
}

#[test]
fn visibility_changes_only_after_the_inner_declaration() {
    assert_eq!(
        output("let x = 1; { println(x); let x = x + 1; println(x); }"),
        "1\n2\n"
    );
    assert_eq!(
        error("{ let x = x + 1; }").kind,
        DiagnosticKind::UnknownName
    );
    assert_eq!(
        error("{ println(later); } let later = 1;").kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn names_cannot_escape_a_block_through_reads_or_assignments() {
    for source in [
        "{ let x = 1; } println(x);",
        "{ let mut x = 1; } x = 2;",
        "{ let x = 1; } let y = x;",
    ] {
        assert_eq!(error(source).kind, DiagnosticKind::UnknownName);
    }
}

#[test]
fn empty_blocks_comments_and_brace_text_keep_their_meanings() {
    assert_eq!(
        output("{} { // } ignored\n } println(\"{text}\");"),
        "{text}\n"
    );
    assert_eq!(compile("{} {{}} ").unwrap().statement_count(), 0);
}

#[test]
fn nesting_limits_are_independent_and_work_at_their_boundaries() {
    let expression = format!(
        "{}1{}",
        "(".repeat(MAX_EXPRESSION_DEPTH),
        ")".repeat(MAX_EXPRESSION_DEPTH)
    );
    let source = format!(
        "{}println({expression});{}",
        "{".repeat(MAX_BLOCK_DEPTH),
        "}".repeat(MAX_BLOCK_DEPTH)
    );
    assert_eq!(output(&source), "1\n");
    let too_deep = format!("{{{source}}}");
    assert_eq!(error(&too_deep).kind, DiagnosticKind::BlockNesting);
    assert_eq!(
        error(&format!("{}{}", "{".repeat(20_000), "}".repeat(20_000))).kind,
        DiagnosticKind::BlockNesting
    );
}

#[test]
fn resolver_also_bounds_externally_constructed_syntax() {
    let mut statements = Vec::new();
    for _ in 0..=MAX_BLOCK_DEPTH {
        statements = vec![Statement::Block {
            statements,
            span: Span::new(0, 1),
        }];
    }
    let diagnostic = scarp::resolver::resolve(Program { statements }).unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::BlockNesting);
}

#[test]
fn missing_and_unexpected_braces_and_block_expressions_are_invalid() {
    for source in [
        "}",
        "{",
        "{ println(1);",
        "{ let x = 1;",
        "{};",
        "let x = {};",
        "println({});",
        "{ 1; }",
        "{ let x = 1; }}",
    ] {
        assert_eq!(
            error(source).kind,
            DiagnosticKind::UnexpectedToken,
            "{source}"
        );
    }
    let source = "{\nprintln(1);";
    let diagnostic = error(source);
    assert!(diagnostic.message.contains("expected '}'"));
    assert_eq!(diagnostic.span.start, source.len());
}

#[test]
fn later_block_errors_preserve_empty_program_output() {
    assert_eq!(
        error("println(1); { println(2); let x = 1 / 0; }").kind,
        DiagnosticKind::DivisionByZero
    );
    assert_eq!(
        error("println(1); { println(missing); }").kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn many_sibling_blocks_do_not_reuse_slots_or_recurse_during_execution() {
    let source = "{ let x = 1; }".repeat(20_000);
    let program = compile(&source).unwrap();
    assert_eq!(program.binding_count(), 20_000);
    assert_eq!(program.statement_count(), 20_000);
    let mut bytes = Vec::new();
    interpreter::execute(&program.clone(), &mut bytes).unwrap();
    assert!(bytes.is_empty());
}

#[test]
fn deterministic_scoped_updates_match_a_reference_model() {
    let mut source = String::from("let mut total = 0;\n");
    let mut expected = String::new();
    let mut total = 0_i64;
    for index in 0..500_i64 {
        source.push_str(&format!(
            "{{ let x = {index}; total += x; {{ let x = x + 1; total += x; }} println(total); }}\n"
        ));
        total += index + index + 1;
        expected.push_str(&format!("{total}\n"));
    }
    assert_eq!(output(&source), expected);
}

#[test]
fn parsed_blocks_retain_their_original_full_source_spans() {
    let source = "// café\n{ let value = 1; println(value); }";
    let program = scarp::parse(source).unwrap();
    let Statement::Block { statements, span } = &program.statements[0] else {
        panic!("expected block");
    };
    assert_eq!(
        &source[span.start..span.end],
        "{ let value = 1; println(value); }"
    );
    assert_eq!(statements.len(), 2);
}

#[test]
fn malformed_block_token_streams_do_not_require_an_eof_token() {
    let mut tokens = scarp::lexer::lex("{ println(1); }").unwrap();
    tokens.pop();
    assert!(scarp::parser::parse(&tokens).is_ok());
    tokens.pop();
    assert!(scarp::parser::parse(&tokens).is_err());
}

#[test]
fn truncated_scoped_programs_always_report_valid_utf8_spans() {
    let source = "// 🌍\nlet mut x = 1; { let x = x + 1; println(x); } x += 2;";
    for end in (0..=source.len()).filter(|offset| source.is_char_boundary(*offset)) {
        let prefix = &source[..end];
        if let Err(diagnostic) = compile(prefix) {
            for span in std::iter::once(diagnostic.span)
                .chain(diagnostic.related.iter().map(|note| note.span))
            {
                assert!(span.start <= span.end && span.end <= prefix.len());
                assert!(prefix.is_char_boundary(span.start) && prefix.is_char_boundary(span.end));
            }
            assert!(diagnostic.render("blocks.srp", prefix).contains('^'));
        }
    }
}
