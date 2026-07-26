//! The compiled `DynamicExp` token stream — the payload types behind Pascal's
//! bare-`Integer` `Cmds` automation array (`General/DynamicExp.pas`).
//!
//! Pascal documents the encoding in the `InterpretDiffEq` header comment
//! (`DynamicExp.pas:458-467`, identical in EPRI r4133 `DynamicExp.pas:564-573`):
//!
//! > Positive integers represent the index to a variable slot (dbl). If the
//! > integer is a value >= 50000, it means that it is the index to a numeric
//! > constant that can be located at `VarConsts`. If is a negative integer,
//! > represents one of the following operations: -2 = Add, -3 = Subtraction,
//! > -4 = Mult, -5 = Div .... etc. For details, check `opCodes` array defined
//! > above. If the negative integer is -50, it means the begining of a new
//! > equation.
//!
//! In other words a `Cmds` cell is a *tagged union* squeezed into one `Integer`,
//! with `50000` and `-50` as the tags. [`DynToken`] is that union spelled out;
//! [`DynOp`] is the operator half (the `opCodes` index, stored negated), and
//! [`Lexeme`] is what `InterpretDiffEq`'s `case OpCode of` does with each
//! `opCodes` entry. The compiler ([`super::DynamicExpObj::interpret_diff_eq`])
//! emits `DynToken`s and the evaluator (`solve_eq`) consumes them, so the
//! sentinel arithmetic exists only in the `#[cfg(test)]` [`DynToken::ordinal`]
//! pin channel that still asserts the compiled stream cell-for-cell against the
//! Pascal notation above.

/// Pascal `opCodes` (index 0..28, `DynamicExp.pas:101-106`; EPRI r4133 `myOps`,
/// `DynamicExp.pas:60-64` — the two tables are identical): the operators
/// `Get_Closer_Op`/`InterpretDiffEq` recognise, in priority order (a tie on
/// position keeps the *earlier* index, which is why `sqrt`/`atan2` are
/// unreachable — see the `substring_tiebreak_*` test).
pub(super) const OP_CODES: [&str; 29] = [
    "dt", "=", "+", "-", "*", "/", "(", ")", ";", "[", "]", "sqr", "sqrt", "inv", "ln", "exp",
    "log10", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "rollup", "rolldn", "swap",
    "pi", "^",
];

/// An RPN operator carried by the compiled stream. The discriminant is the
/// [`OP_CODES`] index — Pascal stores its *negative* in `Cmds`
/// (`Cmds[High(Cmds)] := -1 * OpCode`, `DynamicExp.pas:550`) and `SolveEq`
/// dispatches on it (`DynamicExp.pas:397-449`, r4133 `:520-548`).
///
/// The set is closed: `InterpretDiffEq` emits an operator only from its `else`
/// branch and only when `OpCode <> 10`, so the reachable codes are exactly
/// `{2,3,4,5} ∪ {11..28}` — `dt`(0), `=`(1), `(`(6), `)`(7), `;`(8), `[`(9) are
/// [`Lexeme::Notation`]/[`Lexeme::Dt`] and `]`(10) is [`Lexeme::End`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DynOp {
    /// `+` — `opCodes[2]`, `Cmds` cell -2.
    Add,
    /// `-` — `opCodes[3]`, cell -3.
    Sub,
    /// `*` — `opCodes[4]`, cell -4.
    Mul,
    /// `/` — `opCodes[5]`, cell -5.
    Div,
    /// `sqr` — `opCodes[11]`, cell -11.
    Sqr,
    /// `sqrt` — `opCodes[12]`, cell -12. Unreachable: `sqr` shadows it on the
    /// `Get_Closer_Op` position tie.
    Sqrt,
    /// `inv` — `opCodes[13]`, cell -13.
    Inv,
    /// `ln` — `opCodes[14]`, cell -14.
    Ln,
    /// `exp` — `opCodes[15]`, cell -15.
    Exp,
    /// `log10` — `opCodes[16]`, cell -16.
    Log10,
    /// `sin` — `opCodes[17]`, cell -17.
    Sin,
    /// `cos` — `opCodes[18]`, cell -18.
    Cos,
    /// `tan` — `opCodes[19]`, cell -19.
    Tan,
    /// `asin` — `opCodes[20]`, cell -20.
    ASin,
    /// `acos` — `opCodes[21]`, cell -21.
    ACos,
    /// `atan` — `opCodes[22]`, cell -22.
    ATan,
    /// `atan2` — `opCodes[23]`, cell -23. Unreachable: `atan` shadows it on the
    /// position tie.
    ATan2,
    /// `rollup` — `opCodes[24]`, cell -24.
    RollUp,
    /// `rolldn` — `opCodes[25]`, cell -25.
    RollDn,
    /// `swap` — `opCodes[26]`, cell -26.
    Swap,
    /// `pi` — `opCodes[27]`, cell -27.
    Pi,
    /// `^` — `opCodes[28]`, cell -28.
    Pow,
}

/// What `InterpretDiffEq`'s `case OpCode of` (`DynamicExp.pas:501-554`) does
/// with an [`OP_CODES`] entry. Total by construction: [`OP_LEXEMES`] classifies
/// every one of the 29 indices, so the compiler never needs a fall-through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lexeme {
    /// `dt` (`case 0`): the preceding token is the equation's output variable.
    Dt,
    /// `=`, `(`, `)`, `;`, `[` (`case 1, 6, 7, 8, 9`): "just for notation
    /// reference at the user side" — nothing is emitted, not even operands.
    Notation,
    /// `]` (the `else` branch with `OpCode = 10`): flush the pending operands,
    /// but emit no operator — this is the end of the expression.
    End,
    /// A basic operation or a function (the `else` branch, `OpCode <> 10`):
    /// flush the operands, then the operator.
    Op(DynOp),
}

/// Parallel to [`OP_CODES`]: the `case OpCode of` classification of each index.
pub(super) const OP_LEXEMES: [Lexeme; 29] = [
    Lexeme::Dt,                // 0  dt
    Lexeme::Notation,          // 1  =
    Lexeme::Op(DynOp::Add),    // 2  +
    Lexeme::Op(DynOp::Sub),    // 3  -
    Lexeme::Op(DynOp::Mul),    // 4  *
    Lexeme::Op(DynOp::Div),    // 5  /
    Lexeme::Notation,          // 6  (
    Lexeme::Notation,          // 7  )
    Lexeme::Notation,          // 8  ;
    Lexeme::Notation,          // 9  [
    Lexeme::End,               // 10 ]
    Lexeme::Op(DynOp::Sqr),    // 11 sqr
    Lexeme::Op(DynOp::Sqrt),   // 12 sqrt
    Lexeme::Op(DynOp::Inv),    // 13 inv
    Lexeme::Op(DynOp::Ln),     // 14 ln
    Lexeme::Op(DynOp::Exp),    // 15 exp
    Lexeme::Op(DynOp::Log10),  // 16 log10
    Lexeme::Op(DynOp::Sin),    // 17 sin
    Lexeme::Op(DynOp::Cos),    // 18 cos
    Lexeme::Op(DynOp::Tan),    // 19 tan
    Lexeme::Op(DynOp::ASin),   // 20 asin
    Lexeme::Op(DynOp::ACos),   // 21 acos
    Lexeme::Op(DynOp::ATan),   // 22 atan
    Lexeme::Op(DynOp::ATan2),  // 23 atan2
    Lexeme::Op(DynOp::RollUp), // 24 rollup
    Lexeme::Op(DynOp::RollDn), // 25 rolldn
    Lexeme::Op(DynOp::Swap),   // 26 swap
    Lexeme::Op(DynOp::Pi),     // 27 pi
    Lexeme::Op(DynOp::Pow),    // 28 ^
];

/// One cell of the compiled `Cmds` automation array.
///
/// Every value the Pascal `Integer` cell can legally hold has a variant, so the
/// evaluator's dispatch is exhaustive and the two magic numbers (`50000`,
/// `-50`) live only in the test-only [`DynToken::ordinal`] encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DynToken {
    /// Pascal `-50`: the marker that follows an equation's output-variable
    /// slot ("denotes the begining of an equation", `DynamicExp.pas:519`).
    EqMark,
    /// A negative cell other than `-50`: an RPN operator.
    Op(DynOp),
    /// A non-negative cell below `50000`: a state-variable slot index (a row of
    /// the host's memory space).
    Var(usize),
    /// A cell `>= 50000`: `50000 + index` into `VarConsts`.
    Const(usize),
}

/// The result of Pascal `Get_Var_Idx` (`DynamicExp.pas:283-312`), whose
/// `Integer` return is a three-way tagged value: a state-variable index, the
/// `50001` "it is a numeric constant" code, or `-1` for "neither".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VarRef {
    /// The name is state variable number `.0` (`Result := idx`).
    State(usize),
    /// The name parses as a number — Pascal `Result := 50001` (`CONST_CODE`).
    Const,
    /// Neither a state variable nor a number — Pascal `Result := -1`.
    NotFound,
}

/// The raw `Cmds` cell encoding this enum replaces. Test-only on purpose: the
/// engine never converts a token back to an integer, but the unit tests pin the
/// compiled stream cell-for-cell against the Pascal notation, which is the
/// bit-neutrality proof for the retype.
#[cfg(test)]
mod encoding {
    use super::{DynOp, DynToken, Lexeme, OP_CODES, OP_LEXEMES};

    /// Pascal `Cmds` marker for the start of a new equation.
    const EQ_MARK: i32 = -50;
    /// Pascal `Cmds` offset marking a constant: `50000 + index-into-VarConsts`.
    const CONST_BASE: i32 = 50000;

    impl DynOp {
        /// The [`OP_CODES`] index (`Cmds` stores `-op_code()`).
        pub(crate) fn op_code(self) -> i32 {
            OP_LEXEMES
                .iter()
                .position(|l| *l == Lexeme::Op(self))
                .expect("every DynOp appears in OP_LEXEMES") as i32
        }
    }

    impl DynToken {
        /// The Pascal `Cmds` cell value for this token.
        pub(crate) fn ordinal(self) -> i32 {
            match self {
                DynToken::EqMark => EQ_MARK,
                DynToken::Op(op) => -op.op_code(),
                DynToken::Var(slot) => slot as i32,
                DynToken::Const(i) => CONST_BASE + i as i32,
            }
        }

        /// Decode a Pascal `Cmds` cell (the inverse of [`Self::ordinal`]).
        pub(crate) fn from_ordinal(cell: i32) -> Option<Self> {
            if cell == EQ_MARK {
                return Some(DynToken::EqMark);
            }
            if cell >= CONST_BASE {
                return Some(DynToken::Const((cell - CONST_BASE) as usize));
            }
            if cell >= 0 {
                return Some(DynToken::Var(cell as usize));
            }
            OP_LEXEMES
                .get((-cell) as usize)
                .and_then(|l| match l {
                    Lexeme::Op(op) => Some(*op),
                    _ => None,
                })
                .map(DynToken::Op)
        }
    }

    /// Every operator's discriminant is its `opCodes` index, and the table that
    /// classifies the indices agrees with it in both directions.
    #[test]
    fn op_codes_pin_the_pascal_opcodes_table() {
        // The reachable operator codes are exactly {2,3,4,5} ∪ {11..=28}
        // (`InterpretDiffEq` emits from the `else` branch with `OpCode <> 10`).
        let ops: Vec<i32> = OP_LEXEMES
            .iter()
            .enumerate()
            .filter_map(|(i, l)| matches!(l, Lexeme::Op(_)).then_some(i as i32))
            .collect();
        let expected: Vec<i32> = [2, 3, 4, 5].into_iter().chain(11..=28).collect();
        assert_eq!(ops, expected, "operator opCodes indices");
        // …and each one round-trips through the negated cell encoding.
        for code in expected {
            let Lexeme::Op(op) = OP_LEXEMES[code as usize] else {
                unreachable!()
            };
            assert_eq!(op.op_code(), code, "{}", OP_CODES[code as usize]);
            assert_eq!(DynToken::Op(op).ordinal(), -code);
            assert_eq!(DynToken::from_ordinal(-code), Some(DynToken::Op(op)));
        }
        // The non-operator indices are the Pascal `case 0` / `1,6,7,8,9` /
        // `OpCode = 10` arms, so no operator cell can ever carry them.
        assert_eq!(OP_LEXEMES[0], Lexeme::Dt);
        assert_eq!(OP_LEXEMES[10], Lexeme::End);
        for i in [1usize, 6, 7, 8, 9] {
            assert_eq!(OP_LEXEMES[i], Lexeme::Notation, "{}", OP_CODES[i]);
        }
        for cell in [-1, -6, -7, -8, -9, -10, -29, -49, -51] {
            assert_eq!(DynToken::from_ordinal(cell), None, "cell {cell}");
        }
    }

    /// The payload cells: the `-50` marker, variable slots and the `50000 + i`
    /// constant offset (`DynamicExp.pas:458-467`).
    #[test]
    fn payload_cells_round_trip_the_pascal_encoding() {
        assert_eq!(DynToken::EqMark.ordinal(), -50);
        assert_eq!(DynToken::from_ordinal(-50), Some(DynToken::EqMark));
        assert_eq!(DynToken::Var(0).ordinal(), 0);
        assert_eq!(DynToken::Var(49_999).ordinal(), 49_999);
        assert_eq!(DynToken::from_ordinal(0), Some(DynToken::Var(0)));
        assert_eq!(DynToken::from_ordinal(49_999), Some(DynToken::Var(49_999)));
        assert_eq!(DynToken::Const(0).ordinal(), 50_000);
        assert_eq!(DynToken::Const(7).ordinal(), 50_007);
        assert_eq!(DynToken::from_ordinal(50_000), Some(DynToken::Const(0)));
        assert_eq!(DynToken::from_ordinal(50_007), Some(DynToken::Const(7)));
    }
}
