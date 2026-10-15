//! Operators and built-in functions on [`Value`]s. Shared by the scan and by
//! the constant folding of initial values.

use super::ast::{BinOp, Func, UnOp};
use super::value::{DataType, Value};

/// Why an operation failed at run time.
pub type OpResult = Result<Value, String>;

pub fn unary(op: UnOp, v: Value) -> OpResult {
    Ok(match (op, v) {
        (UnOp::Not, Value::Bool(b)) => Value::Bool(!b),
        (UnOp::Not, Value::Int(i)) => Value::Int(!i),
        (UnOp::Neg, Value::Int(i)) => Value::Int(i.wrapping_neg()),
        (UnOp::Neg, Value::Real(r)) => Value::Real(-r),
        (UnOp::Neg, Value::Time(t)) => Value::Time(-t),
        (UnOp::Not, other) => return Err(format!("NOT does not apply to {other}")),
        (UnOp::Neg, other) => return Err(format!("`-` does not apply to {other}")),
    })
}

fn is_real(v: Value) -> bool {
    matches!(v, Value::Real(_))
}

pub fn binary(op: BinOp, a: Value, b: Value) -> OpResult {
    use BinOp::*;
    Ok(match op {
        And | Or | Xor => match (a, b) {
            (Value::Bool(x), Value::Bool(y)) => Value::Bool(match op {
                And => x && y,
                Or => x || y,
                _ => x ^ y,
            }),
            (Value::Real(_), _)
            | (_, Value::Real(_))
            | (Value::Time(_), _)
            | (_, Value::Time(_)) => {
                return Err("AND, OR and XOR need BOOL or integers".into());
            }
            (x, y) => {
                let (x, y) = (x.as_int(), y.as_int());
                Value::Int(match op {
                    And => x & y,
                    Or => x | y,
                    _ => x ^ y,
                })
            }
        },
        Eq | Ne | Lt | Gt | Le | Ge => {
            let ord = if is_real(a) || is_real(b) {
                a.as_real().partial_cmp(&b.as_real())
            } else {
                Some(a.as_int().cmp(&b.as_int()))
            };
            Value::Bool(match (op, ord) {
                (Eq, o) => o == Some(std::cmp::Ordering::Equal),
                (Ne, o) => o != Some(std::cmp::Ordering::Equal),
                (_, None) => false,
                (Lt, Some(o)) => o.is_lt(),
                (Gt, Some(o)) => o.is_gt(),
                (Le, Some(o)) => o.is_le(),
                (_, Some(o)) => o.is_ge(),
            })
        }
        Add | Sub => match (a, b) {
            (Value::Time(x), Value::Time(y)) => Value::Time(if op == Add {
                x.wrapping_add(y)
            } else {
                x.wrapping_sub(y)
            }),
            (x, y) if is_real(x) || is_real(y) => Value::Real(if op == Add {
                x.as_real() + y.as_real()
            } else {
                x.as_real() - y.as_real()
            }),
            (Value::Bool(_), _) | (_, Value::Bool(_)) => {
                return Err("arithmetic does not apply to BOOL".into());
            }
            (x, y) => Value::Int(if op == Add {
                x.as_int().wrapping_add(y.as_int())
            } else {
                x.as_int().wrapping_sub(y.as_int())
            }),
        },
        Mul => match (a, b) {
            (Value::Time(t), n) | (n, Value::Time(t)) if !matches!(n, Value::Time(_)) => {
                Value::Time((t as f64 * n.as_real()).round() as i64)
            }
            (x, y) if is_real(x) || is_real(y) => Value::Real(x.as_real() * y.as_real()),
            (Value::Bool(_), _) | (_, Value::Bool(_)) => {
                return Err("arithmetic does not apply to BOOL".into());
            }
            (x, y) => Value::Int(x.as_int().wrapping_mul(y.as_int())),
        },
        Div => match (a, b) {
            (Value::Time(t), n) if !matches!(n, Value::Time(_)) => {
                if n.as_real() == 0.0 {
                    return Err("division by zero".into());
                }
                Value::Time((t as f64 / n.as_real()).round() as i64)
            }
            (x, y) if is_real(x) || is_real(y) => Value::Real(x.as_real() / y.as_real()),
            (Value::Bool(_), _) | (_, Value::Bool(_)) => {
                return Err("arithmetic does not apply to BOOL".into());
            }
            (x, y) => {
                if y.as_int() == 0 {
                    return Err("division by zero".into());
                }
                Value::Int(x.as_int().wrapping_div(y.as_int()))
            }
        },
        // IEC: x MOD 0 is 0.
        Mod => {
            let (x, y) = (a.as_int(), b.as_int());
            Value::Int(if y == 0 { 0 } else { x.wrapping_rem(y) })
        }
        Pow => Value::Real(a.as_real().powf(b.as_real())),
    })
}

fn real_fn(v: &[Value], f: impl Fn(f64) -> f64) -> OpResult {
    Ok(Value::Real(f(v[0].as_real())))
}

pub fn call(func: &Func, args: &[Value]) -> OpResult {
    match func {
        Func::Abs => Ok(match args[0] {
            Value::Real(r) => Value::Real(r.abs()),
            Value::Time(t) => Value::Time(t.abs()),
            v => Value::Int(v.as_int().abs()),
        }),
        Func::Sqrt => {
            if args[0].as_real() < 0.0 {
                return Err("SQRT of a negative number".into());
            }
            real_fn(args, f64::sqrt)
        }
        Func::Ln | Func::Log => {
            if args[0].as_real() <= 0.0 {
                return Err("logarithm of a number that is not positive".into());
            }
            real_fn(
                args,
                if *func == Func::Ln {
                    f64::ln
                } else {
                    f64::log10
                },
            )
        }
        Func::Exp => real_fn(args, f64::exp),
        Func::Sin => real_fn(args, f64::sin),
        Func::Cos => real_fn(args, f64::cos),
        Func::Tan => real_fn(args, f64::tan),
        Func::Asin => real_fn(args, f64::asin),
        Func::Acos => real_fn(args, f64::acos),
        Func::Atan => real_fn(args, f64::atan),
        Func::Trunc => Ok(Value::Int(args[0].as_real().trunc() as i64)),
        Func::Expt => Ok(Value::Real(args[0].as_real().powf(args[1].as_real()))),
        Func::Min | Func::Max => {
            let mut best = args[0];
            for &a in &args[1..] {
                let less = binary(BinOp::Lt, a, best)?.as_bool();
                if less == (*func == Func::Min) && a != best {
                    best = a;
                }
            }
            Ok(best)
        }
        Func::Limit => {
            let (mn, x, mx) = (args[0], args[1], args[2]);
            let low = if binary(BinOp::Lt, x, mn)?.as_bool() {
                mn
            } else {
                x
            };
            Ok(if binary(BinOp::Gt, low, mx)?.as_bool() {
                mx
            } else {
                low
            })
        }
        Func::Sel => Ok(if args[0].as_bool() { args[2] } else { args[1] }),
        Func::Mux => {
            let k = args[0].as_int();
            let n = (args.len() - 1) as i64;
            if !(0..n).contains(&k) {
                return Err(format!("MUX selector {k} is outside 0..{}", n - 1));
            }
            Ok(args[1 + k as usize])
        }
        Func::Shl | Func::Shr | Func::Rol | Func::Ror => {
            // Words are 16 bit unless the value needs more.
            let v = args[0].as_int();
            let n = args[1].as_int().clamp(0, 63) as u32;
            let width = if (0..=0xFFFF).contains(&v) { 16 } else { 32 };
            let mask: u64 = if width == 16 { 0xFFFF } else { 0xFFFF_FFFF };
            let u = (v as u64) & mask;
            let n = if matches!(func, Func::Rol | Func::Ror) {
                n % width
            } else {
                n
            };
            let r = match func {
                Func::Shl => u.checked_shl(n).unwrap_or(0) & mask,
                Func::Shr => u.checked_shr(n).unwrap_or(0),
                Func::Rol => ((u << n) | (u >> (width - n).min(63))) & mask,
                _ => ((u >> n) | (u << (width - n).min(63))) & mask,
            };
            Ok(Value::Int(r as i64))
        }
        Func::To(ty) => ty
            .coerce(args[0])
            .ok_or_else(|| format!("cannot convert {} to {ty}", args[0])),
    }
}

/// The value as it is stored in a variable of type `ty`.
pub fn store(ty: DataType, v: Value) -> OpResult {
    ty.coerce(v)
        .ok_or_else(|| format!("cannot store {v} in a {ty}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_follows_iec() {
        assert_eq!(
            binary(BinOp::Div, Value::Int(7), Value::Int(2)),
            Ok(Value::Int(3))
        );
        assert_eq!(
            binary(BinOp::Mod, Value::Int(7), Value::Int(0)),
            Ok(Value::Int(0))
        );
        assert!(binary(BinOp::Div, Value::Int(1), Value::Int(0)).is_err());
        assert_eq!(
            binary(BinOp::Mul, Value::Time(2_000_000), Value::Int(3)),
            Ok(Value::Time(6_000_000))
        );
        assert_eq!(
            binary(BinOp::And, Value::Int(0b1100), Value::Int(0b1010)),
            Ok(Value::Int(0b1000))
        );
    }

    #[test]
    fn functions() {
        let v = |i| Value::Int(i);
        assert_eq!(call(&Func::Limit, &[v(0), v(15), v(10)]), Ok(v(10)));
        assert_eq!(call(&Func::Max, &[v(2), v(9), v(4)]), Ok(v(9)));
        assert_eq!(call(&Func::Min, &[v(2), v(9), v(4)]), Ok(v(2)));
        assert_eq!(call(&Func::Mux, &[v(1), v(10), v(20)]), Ok(v(20)));
        assert_eq!(call(&Func::Shl, &[v(1), v(3)]), Ok(v(8)));
        assert_eq!(call(&Func::Rol, &[v(0x8001), v(1)]), Ok(v(0x0003)));
    }
}
