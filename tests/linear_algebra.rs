use scarp::{
    diagnostics::DiagnosticKind as Kind,
    numeric::{self, Builtin},
    value::{FiniteFloat, NumericTile, NumericVector, Value, ValueType, MAX_VECTOR_LENGTH},
    ExecutionLimits, ExecutionOptions, RunError,
};

fn tile(values: &[f64], height: usize, width: usize) -> Value {
    Value::Tile(NumericTile::new(values, height, width).unwrap())
}
fn vector(values: &[f64]) -> Value {
    Value::Vector(NumericVector::new(values).unwrap())
}
fn scalar(value: f64) -> Value {
    Value::Float(FiniteFloat::new(value).unwrap())
}
fn call(function: Builtin, args: &[Value]) -> Value {
    numeric::evaluate(function, args, 10_000_000).unwrap()
}
fn output(source: &str) -> Vec<u8> {
    let mut out = Vec::new();
    scarp::run(source, &mut out).unwrap();
    scarp::check(source).unwrap();
    out
}

#[test]
fn hand_calculated_rectangular_products_and_tile_arithmetic() {
    let a = tile(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 2, 3);
    let b = tile(&[7.0, 8.0, 9.0, 10.0, 11.0, 12.0], 3, 2);
    assert_eq!(
        call(Builtin::MatMul, &[a.clone(), b]),
        tile(&[58.0, 64.0, 139.0, 154.0], 2, 2)
    );
    assert_eq!(
        call(Builtin::MatVec, &[a.clone(), vector(&[2.0, -1.0, 0.5])]),
        vector(&[1.5, 6.0])
    );
    assert_eq!(
        call(Builtin::TileAdd, &[a.clone(), a.clone()]),
        tile(&[2.0, 4.0, 6.0, 8.0, 10.0, 12.0], 2, 3)
    );
    assert_eq!(
        call(Builtin::TileScale, &[a.clone(), scalar(-2.0)]),
        tile(&[-2.0, -4.0, -6.0, -8.0, -10.0, -12.0], 2, 3)
    );
    assert_eq!(a, tile(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 2, 3));
}

#[test]
fn identity_and_zero_matrices_preserve_rectangular_shapes() {
    let a = tile(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 2, 3);
    assert_eq!(
        call(
            Builtin::MatMul,
            &[tile(&[1.0, 0.0, 0.0, 1.0], 2, 2), a.clone()]
        ),
        a
    );
    assert_eq!(
        call(
            Builtin::MatMul,
            &[
                a.clone(),
                tile(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0], 3, 3)
            ]
        ),
        a
    );
    assert_eq!(
        call(Builtin::MatMul, &[a, tile(&[0.0; 6], 3, 2)]),
        tile(&[0.0; 4], 2, 2)
    );
}

#[test]
fn integer_reference_checks_all_small_shapes_independently() {
    // Exact integer arithmetic is an independent reference for binary64 kernels.
    // Both products and totals here are small enough to be exactly representable.
    for m in 0..5 {
        for k in 0..5 {
            for n in 0..5 {
                let a: Vec<i64> = (0..m * k).map(|i| (i as i64 * 7 + 3) % 13 - 6).collect();
                let b: Vec<i64> = (0..k * n).map(|i| (i as i64 * 11 + 2) % 17 - 8).collect();
                let mut expected = vec![0i64; m * n];
                for i in 0..m {
                    for j in 0..n {
                        for h in 0..k {
                            expected[i * n + j] += a[i * k + h] * b[h * n + j];
                        }
                    }
                }
                let floats = |v: &[i64]| v.iter().map(|&x| x as f64).collect::<Vec<_>>();
                assert_eq!(
                    call(
                        Builtin::MatMul,
                        &[tile(&floats(&a), m, k), tile(&floats(&b), k, n)]
                    ),
                    tile(&floats(&expected), m, n),
                    "{m}x{k} * {k}x{n}"
                );
                let v: Vec<i64> = (0..k).map(|j| j as i64 - 2).collect();
                let expected: Vec<i64> = a
                    .chunks(k.max(1))
                    .map(|row| row.iter().zip(&v).map(|(x, y)| x * y).sum())
                    .collect();
                let expected = if k == 0 { vec![0; m] } else { expected };
                assert_eq!(
                    call(
                        Builtin::MatVec,
                        &[tile(&floats(&a), m, k), vector(&floats(&v))]
                    ),
                    vector(&floats(&expected))
                );
            }
        }
    }
}

#[test]
fn zero_inner_dimensions_produce_explicit_zeros_and_keep_axes() {
    assert_eq!(
        call(Builtin::MatMul, &[tile(&[], 2, 0), tile(&[], 0, 3)]),
        tile(&[0.0; 6], 2, 3)
    );
    assert_eq!(
        call(Builtin::MatVec, &[tile(&[], 3, 0), vector(&[])]),
        vector(&[0.0; 3])
    );
    assert_eq!(
        numeric::evaluate_with_vector_limit(
            Builtin::MatMul,
            &[tile(&[], 0, 3), tile(&[1.0; 6], 3, 2)],
            0,
            0
        )
        .unwrap(),
        tile(&[], 0, 2)
    );
    assert_eq!(
        numeric::evaluate_with_vector_limit(
            Builtin::MatMul,
            &[tile(&[], 65_536, 0), tile(&[], 0, 0)],
            0,
            0
        )
        .unwrap(),
        tile(&[], 65_536, 0)
    );
    assert_eq!(
        numeric::evaluate_with_vector_limit(
            Builtin::TileAdd,
            &[tile(&[], 0, 3), tile(&[], 0, 3)],
            0,
            0
        )
        .unwrap(),
        tile(&[], 0, 3)
    );
}

#[test]
fn compensation_subnormals_signed_zeros_and_intermediate_overflow() {
    let a = tile(&[1e16, 1.0, -1e16], 1, 3);
    assert_eq!(
        call(Builtin::MatVec, &[a.clone(), vector(&[1.0; 3])]),
        vector(&[1.0])
    );
    assert_eq!(
        call(Builtin::MatMul, &[a, tile(&[1.0; 3], 3, 1)]),
        tile(&[1.0], 1, 1)
    );
    let tiny = f64::from_bits(1);
    assert_eq!(
        call(Builtin::MatVec, &[tile(&[tiny], 1, 1), vector(&[1.0])]),
        vector(&[tiny])
    );
    let scaled = call(Builtin::TileScale, &[tile(&[-0.0], 1, 1), scalar(2.0)]);
    assert_eq!(
        scaled.as_tile().unwrap().cells()[0].get().to_bits(),
        (-0.0f64).to_bits()
    );
    let added = call(
        Builtin::TileAdd,
        &[tile(&[-0.0], 1, 1), tile(&[-0.0], 1, 1)],
    );
    assert_eq!(
        added.as_tile().unwrap().cells()[0].get().to_bits(),
        (-0.0f64).to_bits()
    );
    for (function, args) in [
        (
            Builtin::MatVec,
            vec![tile(&[1e308, 1e308], 1, 2), vector(&[1.0, 1.0])],
        ),
        (
            Builtin::MatMul,
            vec![tile(&[1e308, 1e308], 1, 2), tile(&[2.0, -2.0], 2, 1)],
        ),
        (
            Builtin::TileAdd,
            vec![tile(&[1e308], 1, 1), tile(&[1e308], 1, 1)],
        ),
        (Builtin::TileScale, vec![tile(&[1e308], 1, 1), scalar(2.0)]),
    ] {
        assert_eq!(
            numeric::evaluate(function, &args, u64::MAX)
                .unwrap_err()
                .kind,
            Kind::NonFiniteNumber
        );
    }
}

#[test]
fn work_and_payload_caps_are_exact_and_independent() {
    let a = tile(&[1.0; 6], 2, 3);
    for (function, args, work, bytes) in [
        (Builtin::MatVec, vec![a.clone(), vector(&[1.0; 3])], 14, 16),
        (
            Builtin::MatMul,
            vec![a.clone(), tile(&[1.0; 6], 3, 2)],
            28,
            32,
        ),
        (Builtin::TileAdd, vec![a.clone(), a.clone()], 6, 48),
        (Builtin::TileScale, vec![a, scalar(2.0)], 6, 48),
        (
            Builtin::MatMul,
            vec![tile(&[], 2, 0), tile(&[], 0, 3)],
            6,
            48,
        ),
    ] {
        numeric::evaluate_with_vector_limit(function, &args, work, bytes).unwrap();
        assert_eq!(
            numeric::evaluate_with_vector_limit(function, &args, work - 1, bytes)
                .unwrap_err()
                .kind,
            Kind::ExecutionLimit
        );
        assert_eq!(
            numeric::evaluate_with_vector_limit(function, &args, work, bytes - 1)
                .unwrap_err()
                .kind,
            Kind::VectorBudget
        );
        // Work is charged first; neither expensive loops nor reservations run.
        assert_eq!(
            numeric::evaluate_with_vector_limit(function, &args, 0, 0)
                .unwrap_err()
                .kind,
            Kind::ExecutionLimit
        );
    }
}

#[test]
fn maximum_outputs_and_small_results_with_large_work_are_bounded() {
    let count = MAX_VECTOR_LENGTH;
    let args = [
        tile(&vec![1.0; count], 1, count),
        tile(&vec![1.0; count], count, 1),
    ];
    assert_eq!(
        numeric::evaluate(Builtin::MatMul, &args, 2 * count as u64)
            .unwrap_err()
            .kind,
        Kind::ExecutionLimit
    );
    assert_eq!(
        numeric::evaluate_with_vector_limit(Builtin::MatMul, &args, 2 * count as u64 + 1, 8)
            .unwrap(),
        tile(&[count as f64], 1, 1)
    );
    let args = [tile(&[], count, 0), tile(&[], 0, 1)];
    let result =
        numeric::evaluate_with_vector_limit(Builtin::MatMul, &args, count as u64, count * 8)
            .unwrap();
    assert_eq!(result.as_tile().unwrap().len(), count);
    assert!(result
        .as_tile()
        .unwrap()
        .cells()
        .iter()
        .all(|x| x.get() == 0.0));
    assert_eq!(
        numeric::evaluate(
            Builtin::MatMul,
            &[tile(&[], count, 0), tile(&[], 0, count)],
            u64::MAX
        )
        .unwrap_err()
        .kind,
        Kind::VectorLength
    );
}

#[test]
fn shapes_and_user_function_precedence_are_enforced() {
    for source in [
        "print(matvec(tile([1.0],1,1),[]));",
        "print(matmul(tile([],0,2),tile([],3,0)));",
        "print(tileadd(tile([],0,2),tile([],0,3)));",
    ] {
        assert_eq!(scarp::check(source).unwrap_err().kind, Kind::VectorLength);
    }
    for source in [
        "print(matvec([1.0],[1.0]));",
        "print(tilescale(tile([],0,0),2));",
    ] {
        assert_eq!(scarp::compile(source).unwrap_err().kind, Kind::TypeMismatch);
    }
    for source in [
        "print(matmul(tile([],0,0)));",
        "print(matvec(left: tile([],0,0),right: []));",
    ] {
        assert_eq!(
            scarp::compile(source).unwrap_err().kind,
            Kind::CallArguments
        );
    }
    assert_eq!(
        output("pure funcra matmul(x: integer) -> integer {return x+1;} print(matmul(4));"),
        b"5\n"
    );
    assert_eq!(
        Builtin::MatMul.parameter_types(),
        &[ValueType::Tile, ValueType::Tile]
    );
    assert_eq!(Builtin::MatMul.result_type(), ValueType::Tile);
    assert_eq!(Builtin::MatVec.result_type(), ValueType::Vector);
}

#[test]
fn shared_window_inputs_never_expose_hidden_elements() {
    let backing = NumericVector::new(&[99.0, 1.0, 2.0, 3.0, 4.0, 88.0]).unwrap();
    let a = Value::Tile(NumericTile::from_vector(backing.window(1, 4).unwrap(), 2, 2).unwrap());
    let b = Value::Vector(backing.window(2, 2).unwrap());
    assert_eq!(call(Builtin::MatVec, &[a.clone(), b]), vector(&[8.0, 18.0]));
    assert_eq!(
        call(Builtin::TileScale, &[a.clone(), scalar(2.0)]),
        tile(&[2.0, 4.0, 6.0, 8.0], 2, 2)
    );
    assert_eq!(
        call(Builtin::MatMul, &[a.clone(), a]),
        tile(&[7.0, 10.0, 15.0, 22.0], 2, 2)
    );
}

#[test]
fn typed_contracts_defaults_named_calls_and_snapshots_are_preserved() {
    let source = r#"
        pure funcra transform(m: tile, x: vector = lane(m,0)) -> vector
            needs width(m) == len(x); ensures len(result) == height(m);
        { return matvec(m,x); }
        var m=tile([1.0,2.0,3.0,4.0],2,2);
        print(m); m=tilescale(m,2.0);
        print(transform(m: m,x: [1.0,1.0]), transform(m));
    "#;
    assert_eq!(
        output(source),
        b"tile(2x2)[[1.0, 2.0], [3.0, 4.0]]\n[6.0, 14.0] [20.0, 44.0]\n"
    );
}

#[test]
fn all_runtime_failures_discard_both_program_streams() {
    for (source,options,kind) in [
        ("print(1); eprint(2); print(matvec(tile([1.0],1,1),[]));",ExecutionOptions::default(),Kind::VectorLength),
        ("print(1); eprint(2); print(tilescale(tile([1e308],1,1),2.0));",ExecutionOptions::default(),Kind::NonFiniteNumber),
        ("print(1); eprint(2); print(matmul(tile([1.0],1,1),tile([2.0],1,1)));",ExecutionOptions {max_vector_bytes:23,..ExecutionOptions::default()},Kind::VectorBudget),
        ("print(1); eprint(2); print(matmul(tile(fill(1.0,100),1,100),tile(fill(2.0,100),100,1)));",ExecutionOptions {limits: ExecutionLimits {max_steps:150,..ExecutionLimits::default()},..ExecutionOptions::default()},Kind::ExecutionLimit),
    ] {
        let expected=scarp::check_with_options(source,options).unwrap_err();
        assert_eq!(expected.kind,kind);
        let (mut out,mut err)=(Vec::new(),Vec::new());
        let RunError::Diagnostic(actual)=scarp::run_with_io_and_options(source,&mut out,&mut err,options).unwrap_err() else {panic!("diagnostic expected")};
        assert_eq!(actual,expected);
        assert!(out.is_empty() && err.is_empty());
    }
}

#[test]
fn compiled_matrix_programs_have_fresh_concurrent_execution_state() {
    let program = std::sync::Arc::new(
        scarp::compile("bind t=tile([1.0,2.0,3.0,4.0],2,2); print(matmul(t,t));").unwrap(),
    );
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let program = program.clone();
            std::thread::spawn(move || {
                for _ in 0..4 {
                    let mut out = Vec::new();
                    scarp::interpreter::execute_with_options(
                        &program,
                        &mut out,
                        ExecutionOptions {
                            max_vector_bytes: 64,
                            ..ExecutionOptions::default()
                        },
                    )
                    .unwrap();
                    assert_eq!(out, b"tile(2x2)[[7.0, 10.0], [15.0, 22.0]]\n");
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}
