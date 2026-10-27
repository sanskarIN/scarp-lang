use scarp::{
    diagnostics::{DiagnosticKind, Span},
    lexer::{lex, TokenKind},
};

#[test]
fn all_five_escapes_decode_and_print_exact_bytes() {
    for (source, want) in [
        (r#"println("");"#, "\n"),
        (r#"println("\t\r\n\\\"");"#, "\t\r\n\\\"\n"),
        (r#"println("\\t\\r");"#, "\\t\\r\n"),
        (r#"let text="π\t数\r終"; println(text);"#, "π\t数\r終\n"),
        (r#"println("\r");"#, "\r\n"),
    ] {
        let mut out = Vec::new();
        scarp::run(source, &mut out).unwrap();
        assert_eq!(out, want.as_bytes());
        scarp::check(source).unwrap();
    }
}

#[test]
fn escapes_retain_source_spans_in_lf_and_crlf_files() {
    for newline in ["\n", "\r\n"] {
        let source = format!("// café 🌍{newline}println(\"π\\t\\r\");{newline}");
        let tokens = lex(&source).unwrap();
        let string = tokens
            .iter()
            .find(|t| matches!(t.kind, TokenKind::String(_)))
            .unwrap();
        assert_eq!(&source[string.span.start..string.span.end], "\"π\\t\\r\"");
        assert_eq!(string.kind, TokenKind::String("π\t\r".into()));
        assert_eq!(
            scarp::diagnostics::location(&source, string.span.start).line,
            2
        );
        let mut out = Vec::new();
        scarp::run(&source, &mut out).unwrap();
        assert_eq!(out, "π\t\r\n".as_bytes());
    }
}

#[test]
fn invalid_escapes_and_physical_linebreaks_keep_their_codes_and_spans() {
    for suffix in ["x", "b", "0", "u", "🌍"] {
        let source = format!("\"π\\{suffix}\"");
        let error = lex(&source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::InvalidEscape);
        assert_eq!(error.span, Span::new(3, 4 + suffix.len()));
        assert!(error.help.unwrap().contains(r"\t"));
    }
    for source in ["\"a\rb\"", "\"a\nb\"", "\"a\r\nb\"", "\"a\\\r\n", "\"a\\"] {
        assert_eq!(
            lex(source).unwrap_err().kind,
            DiagnosticKind::UnterminatedString
        );
    }
}

#[test]
fn decoded_controls_are_escaped_in_token_and_diagnostic_displays() {
    let tokens = lex(r#"println("\t\r");"#).unwrap();
    assert_eq!(tokens[2].kind.to_string(), r#"string("\t\r")"#);
    let source = "// π\r\n\tprintln(\"\\t\\r\"); println(1/0);";
    let error = scarp::check(source).unwrap_err();
    let rendered = error.render("controls.srp", source);
    assert!(!rendered.contains('\r') && !rendered.contains('\t'));
    let mut out = Vec::new();
    assert!(scarp::run(source, &mut out).is_err());
    assert!(out.is_empty());
}
