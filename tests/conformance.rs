use scarp::diagnostics::Span;

struct Generator {
    state: u64,
    source: String,
    expected: String,
    total: i64,
}

impl Generator {
    fn next(&mut self) -> i64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.state >> 32) % 21) as i64 - 10
    }

    fn scope(&mut self, outer: i64, depth: usize) {
        let delta = self.next();
        let mut value = outer + delta;
        self.source.push_str(&format!(
            "{{ let mut value = value + ({delta}); println(value);\n"
        ));
        self.expected.push_str(&format!("{value}\n"));
        let change = self.next();
        self.source
            .push_str(&format!("value += ({change}); total += value;\n"));
        value += change;
        self.total += value;
        if depth < 5 && self.next() % 3 != 0 {
            self.scope(value, depth + 1);
        }
        self.source.push_str("println(value); println(total); }\n");
        self.expected
            .push_str(&format!("{value}\n{}\n", self.total));
    }
}

#[test]
fn generated_scoped_programs_match_an_independent_value_model() {
    for seed in 0..2_000_u64 {
        let initial = (seed % 17) as i64;
        let mut generator = Generator {
            state: seed,
            source: format!("let value = {initial}; let mut total = 0;\n"),
            expected: String::new(),
            total: 0,
        };
        for _ in 0..3 {
            generator.scope(initial, 0);
        }
        generator.source.push_str("println(value); println(total);");
        generator
            .expected
            .push_str(&format!("{initial}\n{}\n", generator.total));
        scarp::check(&generator.source).unwrap();
        let mut output = Vec::new();
        scarp::run(&generator.source, &mut output).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            generator.expected,
            "seed {seed}"
        );
    }
}

fn observe(source: &str) {
    let result = std::panic::catch_unwind(|| scarp::check(source))
        .unwrap_or_else(|_| panic!("source triggered a panic: {source:?}"));
    if let Err(diagnostic) = result {
        for span in
            std::iter::once(diagnostic.span).chain(diagnostic.related.iter().map(|note| note.span))
        {
            assert!(span.start <= span.end && span.end <= source.len());
            assert!(source.is_char_boundary(span.start) && source.is_char_boundary(span.end));
        }
        let rendered = diagnostic.render("corpus.srp", source);
        assert!(rendered.contains('^'));
        assert!(!rendered.contains('\u{1b}'));
    }
}

#[test]
fn inserted_deleted_and_truncated_source_mutations_are_panic_free() {
    for source in [
        "let value = 7; println(value);",
        "// नमस्ते 🌍\r\nlet mut x = 2; x += x * 3; println(x);",
        "let mut total = 1; { let total = total + 1; println(total); } total /= -2;",
        "let x = 1; { let x = x + 1; let x = 2; }",
        "let mut text = \"नमस्ते 🌍\"; let copy = text; text = \"done\"; println(copy);",
        "let mut text = \"hello\"; { let text = 1; println(text); } text = 2;",
    ] {
        let offsets: Vec<_> = source
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(source.len()))
            .collect();
        for offset in offsets {
            observe(&source[..offset]);
            for insertion in [
                "=", "+=", "{", "}", ";", "\\", "\"", "//", "\n", "\0", "\u{1b}", "🌍", "mut",
            ] {
                observe(&format!(
                    "{}{insertion}{}",
                    &source[..offset],
                    &source[offset..]
                ));
            }
            if let Some(character) = source[offset..].chars().next() {
                observe(&format!(
                    "{}{}",
                    &source[..offset],
                    &source[offset + character.len_utf8()..]
                ));
            }
        }
    }
}

#[test]
fn altered_public_token_streams_and_spans_do_not_panic() {
    use scarp::lexer::{lex, TokenKind};
    let source = "let mut x = 1; { x += 2; println(x); }";
    let original = lex(source).unwrap();
    for index in 0..original.len() {
        for kind in [
            TokenKind::Eof,
            TokenKind::LeftBrace,
            TokenKind::RightBrace,
            TokenKind::Equal,
            TokenKind::Identifier("let".into()),
            TokenKind::Identifier("mut".into()),
            TokenKind::Identifier("missing".into()),
            TokenKind::Integer("99999999999999999999".into()),
            TokenKind::String("text\u{1b}".into()),
        ] {
            let mut tokens = original.clone();
            tokens[index].kind = kind;
            tokens[index].span = Span::new(usize::MAX, 0);
            let result = std::panic::catch_unwind(|| scarp::parser::parse(&tokens));
            match result.unwrap() {
                Ok(program) => {
                    if let Err(diagnostic) = scarp::resolver::resolve(program) {
                        assert!(diagnostic.render("tokens.srp", source).contains('^'));
                    }
                }
                Err(diagnostic) => {
                    assert!(diagnostic.render("tokens.srp", source).contains('^'));
                }
            }
        }
    }
}
