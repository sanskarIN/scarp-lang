use std::io::{self, Write};

use scarp::{
    ast::{PrintArgument, Statement},
    compile,
    diagnostics::{location, Diagnostic, DiagnosticKind, Span},
    interpreter,
    lexer::{lex, Token, TokenKind},
    parser, run, RunError,
};

#[test]
fn tokenizes_a_print_statement_with_byte_spans() {
    let tokens = lex(r#"println("hi");"#).unwrap();
    assert_eq!(
        tokens,
        vec![
            Token {
                kind: TokenKind::Identifier("println".into()),
                span: Span::new(0, 7),
            },
            Token {
                kind: TokenKind::LeftParen,
                span: Span::new(7, 8),
            },
            Token {
                kind: TokenKind::String("hi".into()),
                span: Span::new(8, 12),
            },
            Token {
                kind: TokenKind::RightParen,
                span: Span::new(12, 13),
            },
            Token {
                kind: TokenKind::Semicolon,
                span: Span::new(13, 14),
            },
            Token {
                kind: TokenKind::Eof,
                span: Span::new(14, 14),
            },
        ]
    );
}

#[test]
fn comments_and_whitespace_do_not_produce_tokens() {
    let source = " // ignored 🌍\r\n\tprintln // split call\n(\"// text\"); // end";
    let tokens = lex(source).unwrap();
    assert_eq!(tokens.len(), 6);
    assert_eq!(tokens[2].kind, TokenKind::String("// text".into()));
    assert_eq!(location(source, tokens[0].span.start).line, 2);
}

#[test]
fn decodes_all_supported_escapes() {
    let tokens = lex(r#"println("a\\b\"c\nd");"#).unwrap();
    assert_eq!(tokens[2].kind, TokenKind::String("a\\b\"c\nd".into()));
}

#[test]
fn preserves_unicode_text_and_byte_boundaries() {
    let source = "println(\"नमस्ते 🌍 café\");";
    let tokens = lex(source).unwrap();
    for token in &tokens {
        assert!(source.is_char_boundary(token.span.start));
        assert!(source.is_char_boundary(token.span.end));
        assert!(token.span.start <= token.span.end);
    }
    assert_eq!(
        &source[tokens[2].span.start..tokens[2].span.end],
        "\"नमस्ते 🌍 café\""
    );
    let mut output = Vec::new();
    run(source, &mut output).unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "नमस्ते 🌍 café\n");
}

#[test]
fn accepts_a_single_leading_utf8_byte_order_mark() {
    let source = "\u{feff}println(\"ok\");";
    assert_eq!(lex(source).unwrap()[0].span.start, 3);
    let mut output = Vec::new();
    run(source, &mut output).unwrap();
    assert_eq!(output, b"ok\n");
    assert!(lex(" \u{feff}").is_err());
}

#[test]
fn empty_and_comment_only_programs_are_valid() {
    for source in ["", " \t\r\n", "// comment", "// a\r// b\n", "\u{feff}"] {
        let mut output = Vec::new();
        run(source, &mut output).unwrap();
        assert!(output.is_empty());
        assert!(compile(source).unwrap().statements.is_empty());
    }
}

#[test]
fn parses_a_print_statement_and_its_full_span() {
    let program = compile(r#"println("hello");"#).unwrap();
    assert_eq!(
        program.statements,
        vec![Statement::Println {
            argument: PrintArgument::String("hello".into()),
            span: Span::new(0, 17),
        }]
    );
}

#[test]
fn executes_two_statements_in_order_with_empty_string_support() {
    let mut output = Vec::new();
    run("println(\"\"); println(\"second\\nline\");", &mut output).unwrap();
    assert_eq!(output, b"\nsecond\nline\n");
}

#[test]
fn rejects_unterminated_strings_at_eof_or_a_physical_line_break() {
    for source in [
        "\"",
        "\"hello",
        "\"hello\\",
        "\"a\nb\"",
        "\"a\rb\"",
        "\"a\\\n",
    ] {
        let error = lex(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::UnterminatedString, "{source:?}");
        assert_eq!(error.span.start, 0);
        assert!(error.render("test.srp", source).contains("E0002"));
    }
}

#[test]
fn rejects_unknown_escapes_at_the_escape_span() {
    for escape in ["t", "r", "0", "u", "🌍"] {
        let source = format!("\"\\{escape}\"");
        let error = lex(&source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::InvalidEscape);
        assert_eq!(
            &source[error.span.start..error.span.end],
            format!("\\{escape}")
        );
    }
}

#[test]
fn rejects_invalid_characters_without_splitting_unicode() {
    for character in ['@', '%', '=', '🌍', '\0'] {
        let source = format!(" \n{character}");
        let error = lex(&source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::InvalidCharacter);
        assert_eq!(error.span, Span::new(2, 2 + character.len_utf8()));
        let rendered = error.render("test.srp", &source);
        assert!(rendered.contains("test.srp:2:1"));
        assert!(!rendered.contains('\0'));
    }
}

#[test]
fn rejects_unknown_statement_names() {
    for name in ["print", "println_extra", "Println", "_println"] {
        let source = format!("{name}(\"text\");");
        let error = compile(&source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::UnknownStatement);
        assert_eq!(error.span, Span::new(0, name.len()));
    }
}

#[test]
fn diagnoses_missing_punctuation_and_arguments() {
    for (source, expected) in [
        ("println \"x\");", "'('"),
        ("println(\"x\";", "')'"),
        ("println(\"x\")", "';'"),
        ("println", "'('"),
        ("println(", "a string literal"),
        ("println()", "a string literal"),
        ("println(\"x\"", "')'"),
        (";", "a println statement"),
        ("println(\"x\");;", "a println statement"),
        ("println(\"x\" \"y\");", "')'"),
    ] {
        let error = compile(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::UnexpectedToken, "{source}");
        assert!(error.message.contains(expected), "{error}");
    }
}

#[test]
fn invalid_later_statements_produce_no_program_output() {
    for invalid in [
        "println(\"valid\"); @",
        "println(\"valid\"); println(\"bad\")",
    ] {
        let mut output = Vec::new();
        assert!(matches!(
            run(invalid, &mut output),
            Err(RunError::Diagnostic(_))
        ));
        assert!(output.is_empty());
    }
}

#[test]
fn resolves_unicode_crlf_and_eof_locations() {
    let source = "é🌍\r\nx\n";
    assert_eq!(location(source, 2).column, 2);
    assert_eq!(location(source, 3), location(source, 2));
    assert_eq!(location(source, 8).line, 2);
    assert_eq!(location(source, source.len()).line, 3);
    assert_eq!(location(source, source.len()).column, 1);
    assert_eq!(location(source, usize::MAX), location(source, source.len()));
}

#[test]
fn renders_a_caret_at_eof_and_expands_tabs() {
    let source = "// header\r\n\tprintln(\"é\")";
    let error = compile(source).unwrap_err();
    let rendered = error.render("example.srp", source);
    assert!(rendered.contains("example.srp:2:14"), "{rendered}");
    assert!(rendered.contains("2 |     println(\"é\")"));
    assert!(rendered.contains('^'));
    assert!(rendered.contains("help:"));
}

#[test]
fn diagnostic_rendering_tolerates_arbitrary_public_spans() {
    for span in [Span::new(1, 0), Span::new(2, 3), Span::new(usize::MAX, 0)] {
        let error = Diagnostic::new(DiagnosticKind::UnexpectedToken, span, "test");
        let rendered = error.render("unicode.srp", "🌍\r\n");
        assert!(rendered.contains('^'));
    }
}

#[test]
fn parses_external_token_streams_without_panics() {
    assert!(parser::parse(&[]).unwrap().statements.is_empty());
    let mut tokens = lex("println(\"ok\");").unwrap();
    tokens.pop();
    assert_eq!(parser::parse(&tokens).unwrap().statements.len(), 1);
    tokens.pop();
    assert!(parser::parse(&tokens).is_err());
    let mut tokens = lex("").unwrap();
    tokens.push(tokens[0].clone());
    assert!(parser::parse(&tokens).is_err());
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "test sink"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn propagates_output_failures_through_the_library() {
    let program = compile("println(\"x\");").unwrap();
    let error = interpreter::execute(&program, &mut FailingWriter).unwrap_err();
    assert!(
        matches!(error, RunError::Output(error) if error.kind() == io::ErrorKind::PermissionDenied)
    );
    assert!(matches!(
        run("println(\"x\");", &mut FailingWriter),
        Err(RunError::Output(_))
    ));
}

#[test]
fn malformed_input_corpus_keeps_error_spans_within_source() {
    // A deterministic mutation corpus checks truncation at every UTF-8 boundary.
    let program = "// comment\r\nprintln(\"hé🌍\\n\\\"\\\\\"); println(\"next\");";
    for end in (0..=program.len()).filter(|end| program.is_char_boundary(*end)) {
        let source = &program[..end];
        if let Err(error) = compile(source) {
            assert!(error.span.start <= error.span.end);
            assert!(error.span.end <= source.len());
            assert!(source.is_char_boundary(error.span.start));
            assert!(source.is_char_boundary(error.span.end));
            assert!(error.render("corpus.srp", source).contains('^'));
        }
    }
}

#[test]
fn handles_many_statements_without_recursive_parsing() {
    let source = "println(\"x\");\n".repeat(10_000);
    let mut output = Vec::new();
    run(&source, &mut output).unwrap();
    assert_eq!(output.len(), 20_000);
}
