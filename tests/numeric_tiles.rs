use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind as Kind},
    numeric::{self, Builtin},
    value::{NumericTile, NumericVector, Value, ValueType, MAX_VECTOR_LENGTH},
    ExecutionLimits, RunError,
};

fn output(source: &str) -> String {
    let mut stdout = Vec::new();
    scarp::run(source, &mut stdout).unwrap();
    scarp::check(source).unwrap();
    String::from_utf8(stdout).unwrap()
}

fn error(source: &str, kind: Kind) -> Diagnostic {
    let expected = scarp::check(source).unwrap_err();
    assert_eq!(expected.kind, kind, "{source}");
    let mut stdout = Vec::new();
    let RunError::Diagnostic(actual) = scarp::run(source, &mut stdout).unwrap_err() else {
        panic!("expected a runtime diagnostic");
    };
    assert!(stdout.is_empty());
    assert_eq!(actual, expected);
    actual
}

fn flat(values: &[f64]) -> Value {
    Value::Vector(NumericVector::new(values).unwrap())
}

fn shaped(values: &[f64], height: usize, width: usize) -> Value {
    Value::Tile(NumericTile::new(values, height, width).unwrap())
}

#[test]
fn construct_inspect_read_rows_and_swap_axes() {
    let source = "bind t = tile([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 2, 3); print(t); print(height(t), width(t), cell(t, 1, 2)); print(lane(t, 0), lane(t, 1)); print(unroll(t)); print(turn(t)); print(turn(turn(t)) == t);";
    assert_eq!(
        output(source),
        "tile(2x3)[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]\n2 3 6.0\n[1.0, 2.0, 3.0] [4.0, 5.0, 6.0]\n[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]\ntile(3x2)[[1.0, 4.0], [2.0, 5.0], [3.0, 6.0]]\ntrue\n"
    );
    assert_eq!(
        scarp::compile("bind t = tile([], 0, 1); bind cells = unroll(t);")
            .unwrap()
            .binding_types(),
        &[ValueType::Tile, ValueType::Vector]
    );
}

#[test]
fn empty_shapes_preserve_dimension_identity() {
    assert_eq!(
        output(
            "bind a = tile([], 0, 3); bind b = tile([], 3, 0); print(a); print(b); print(a == b); print(turn(a)); print(lane(b, 2)); print(unroll(b));"
        ),
        "tile(0x3)[]\ntile(3x0)[[], [], []]\nfalse\ntile(3x0)[[], [], []]\n[]\n[]\n"
    );
    assert_eq!(
        output("bind t = tile([], 0, 0); print(turn(t), height(t), width(t));"),
        "tile(0x0)[] 0 0\n"
    );
}

#[test]
fn numeric_cells_are_immutable_and_unroll_shares_storage() {
    let original = NumericVector::new(&[1.0, 2.0, 3.0, 4.0]).unwrap();
    let grid = NumericTile::from_vector(original.clone(), 2, 2).unwrap();
    assert!(std::ptr::eq(grid.cells().as_ptr(), original.as_slice().as_ptr()));
    let unrolled = numeric::evaluate(Builtin::Unroll, &[Value::Tile(grid.clone())], 1)
        .unwrap();
    assert!(std::ptr::eq(
        unrolled.as_vector().unwrap().as_slice().as_ptr(),
        original.as_slice().as_ptr()
    ));
    assert_eq!(grid.cell(1, 1).unwrap().get(), 4.0);
    assert_eq!(grid.row(0).unwrap()[1].get(), 2.0);
    assert!(grid.cell(2, 0).is_none());
    assert!(grid.cell(0, 2).is_none());
    assert!(grid.row(2).is_none());
    let second = NumericTile::new(&[1.0, 2.0, 3.0, 4.0], 2, 2).unwrap();
    assert_eq!(grid, second);
    assert_ne!(grid, NumericTile::new(&[1.0, 2.0, 3.0, 4.0], 1, 4).unwrap());
    let turn = numeric::evaluate(Builtin::Turn, &[Value::Tile(grid)], 4).unwrap();
    assert_eq!(
        turn,
        shaped(&[1.0, 3.0, 2.0, 4.0], 2, 2)
    );
    assert_eq!(original.as_slice()[2].get(), 3.0);
}

#[test]
fn typed_functions_contracts_and_result_snapshots_work() {
    let source = r#"
        pure funcra make(x: vector, w: integer) -> tile
            needs w > 0;
            ensures height(result) == 2;
        {
            return tile(x, 2, w);
        }
        pure funcra read(x: tile, r: integer, c: integer) -> float {
            return cell(x, r, c);
        }
        bind t = make([1.0, 2.0, 3.0, 4.0], 2);
        print(read(t, 1, 0), width(t));
        var u = t;
        u = turn(t);
        print(t, u);
    "#;
    assert_eq!(
        output(source),
        "3.0 2\ntile(2x2)[[1.0, 2.0], [3.0, 4.0]] tile(2x2)[[1.0, 3.0], [2.0, 4.0]]\n"
    );
    assert_eq!(
        output("pure funcra identity(v: tile = tile([2.0], 1, 1)) -> tile { return v; } print(identity());"),
        "tile(1x1)[[2.0]]\n"
    );
}

#[test]
fn shape_overflow_and_invalid_cells_are_atomic_errors() {
    for source in [
        "print(tile([1.0], 1, 2));",
        "print(tile([], -1, 0));",
        "print(tile([], 0, -1));",
        "print(tile([], 65537, 0));",
        "print(tile([], 0, 65537));",
        "print(tile([], 65536, 65536));",
    ] {
        error(source, Kind::VectorLength);
    }
    for source in [
        "print(cell(tile([1.0], 1, 1), 1, 0));",
        "print(cell(tile([1.0], 1, 1), -1, 0));",
        "print(cell(tile([1.0], 1, 1), 0, -1));",
        "print(lane(tile([], 0, 3), 0));",
        "print(lane(tile([], 3, 0), 3));",
    ] {
        error(source, Kind::VectorIndex);
    }
    for source in [
        "print(tile([1.0], 1.0, 1));",
        "print(tile(1.0, 1, 1));",
        "print(cell([1.0], 0, 0));",
        "print(lane(tile([1.0], 1, 1), 0.0));",
        "print(tile([1.0], 1, 1)[0]);",
    ] {
        error(source, Kind::TypeMismatch);
    }
    for source in [
        "print(tile([1.0], 1));",
        "print(cell(tile([1.0], 1, 1), 0));",
        "print(height());",
        "print(turn(tile: tile([1.0], 1, 1)));",
    ] {
        error(source, Kind::CallArguments);
    }
    error(
        "print(\"before\"); print(cell(tile([1.0], 1, 1), 2, 0));",
        Kind::VectorIndex,
    );
}

#[test]
fn distinct_shape_is_not_equal_even_with_identical_cells() {
    assert_eq!(
        output(
            "bind a = tile([1.0, 2.0], 1, 2); bind b = tile([1.0, 2.0], 2, 1); print(a == b, a != b); print(tile([-0.0], 1, 1) == tile([0.0], 1, 1));"
        ),
        "false true\ntrue\n"
    );
    assert_eq!(
        output(
            "pure funcra height(t: integer) -> integer { return t + 1; } print(height(4));"
        ),
        "5\n"
    );
}

#[test]
fn budget_accounting_is_exact_and_errors_are_atomic() {
    let source = shaped(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 2, 3);
    let tile_args = [flat(&[1.0, 2.0, 3.0, 4.0]), Value::Integer(2), Value::Integer(2)];
    numeric::evaluate(Builtin::Tile, &tile_args, 1).unwrap();
    assert_eq!(
        numeric::evaluate(Builtin::Tile, &tile_args, 0)
            .unwrap_err()
            .kind,
        Kind::ExecutionLimit
    );
    for (builtin, args, passes) in [
        (Builtin::Cell, vec![source.clone(), Value::Integer(0), Value::Integer(0)], 1),
        (Builtin::Lane, vec![source.clone(), Value::Integer(1)], 3),
        (Builtin::Unroll, vec![source.clone()], 1),
        (Builtin::Turn, vec![source.clone()], 6),
        (Builtin::Height, vec![source.clone()], 0),
        (Builtin::Width, vec![source.clone()], 0),
    ] {
        numeric::evaluate(builtin, &args, passes).unwrap();
        if passes != 0 {
            assert_eq!(
                numeric::evaluate(builtin, &args, passes - 1)
                    .unwrap_err()
                    .kind,
                Kind::ExecutionLimit
            );
        }
    }
    numeric::evaluate(Builtin::Lane, &[shaped(&[], 2, 0), Value::Integer(1)], 0).unwrap();
    numeric::evaluate(Builtin::Turn, &[shaped(&[], 0, 10)], 0).unwrap();
    let source = "print(\"ok\"); repeat 10 as i { print(turn(tile([1.0, 2.0, 3.0, 4.0], 2, 2))); }";
    let limits = ExecutionLimits { max_steps: 20, max_prints: 50 };
    let expected = scarp::check_with_limits(source, limits).unwrap_err();
    assert_eq!(expected.kind, Kind::ExecutionLimit);
    let mut out = Vec::new();
    let RunError::Diagnostic(actual) =
        scarp::run_with_limits(source, &mut out, limits).unwrap_err()
    else {
        panic!("expected diagnostic");
    };
    assert_eq!(actual, expected);
    assert!(out.is_empty());
}

#[test]
fn largest_tile_round_trips_under_host_bounds() {
    let cells = NumericVector::new(&vec![2.0; MAX_VECTOR_LENGTH]).unwrap();
    let original = NumericTile::from_vector(cells.clone(), 256, 256).unwrap();
    let value = numeric::evaluate(Builtin::Unroll, &[Value::Tile(original)], 1).unwrap();
    assert_eq!(value.as_vector().unwrap().len(), MAX_VECTOR_LENGTH);
    assert!(NumericTile::from_vector(cells.clone(), MAX_VECTOR_LENGTH + 1, 0).is_none());
    assert!(NumericTile::from_vector(cells.clone(), 65_536, 65_536).is_none());
    assert!(NumericTile::from_vector(cells, 0, 0).is_none());
}
