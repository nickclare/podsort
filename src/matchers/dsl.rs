#![allow(unused)] // TODO: for development
use super::*;
use chumsky::input::{BorrowInput, MappedInput, StrInput, Stream, ValueInput};
use chumsky::prelude::*;
use color_eyre::eyre::{self, Context, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Type {
    Bool,
    Int,
    String,
    Pattern,
    None,
    Error,
}

impl Type {
    pub fn parse_value(&self, input: &str) -> eyre::Result<Value> {
        match self {
            Type::Bool => match input {
                "true" => Ok(Value::Bool(true)),
                "false" => Ok(Value::Bool(false)),
                _ => bail!("invalid boolean: {input}"),
            },
            Type::Int => {
                let value = input.parse::<i64>().wrap_err("invalid integer: {input}")?;
                Ok(Value::Int(value))
            }
            Type::String => Ok(Value::String(input.into())),
            Type::Pattern => unreachable!(),
            Type::None => Ok(Value::None),
            Type::Error => bail!("can't parse anything as an 'error' type"),
        }
    }
}

pub(crate) trait Typed {
    fn typ(&self) -> Type;
}

#[derive(Debug)]
pub struct Filter {
    // This must have ty == bool to be valid
    pub(crate) expr: Expr,
}

impl Filter {
    pub fn from_expr(expr: Expr) -> eyre::Result<Self> {
        let typ = expr.typ();
        if typ != Type::Bool {
            eyre::bail!("Boolean expression expected, got type {typ:?}")
        } else {
            Ok(Self { expr })
        }
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Value),
    Ident(Field),
    Comparison {
        lhs: Box<Expr>,
        op: ComparisonOp,
        rhs: Box<Expr>,
    },
    And {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Or {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Not {
        expr: Box<Expr>,
    },
    Id {
        expr: Box<Expr>,
    },
}

impl Typed for Field {
    fn typ(&self) -> Type {
        match self {
            Field::Episode => Type::Int,
            Field::Season => Type::Int,
            _ => Type::String,
        }
    }
}

#[inline(always)]
fn typecheck_boolean_op(lhs: &Expr, rhs: &Expr) -> Type {
    match (lhs.typ(), rhs.typ()) {
        (Type::Bool, Type::Bool) => Type::Bool,
        (_, _) => Type::Error,
    }
}

#[inline(always)]
fn typecheck_comparison_op(lhs: &Expr, op: ComparisonOp, rhs: &Expr) -> Type {
    match (op, lhs.typ(), rhs.typ()) {
        (ComparisonOp::Like, Type::String, Type::String | Type::Pattern) => Type::Bool,
        (_, lt, rt) if lt == rt => Type::Bool,
        (_, _, _) => Type::Error,
    }
}

impl Typed for Expr {
    fn typ(&self) -> Type {
        match self {
            Expr::Literal(value) => value.typ(),
            Expr::Ident(field) => field.typ(),
            Expr::Comparison { lhs, rhs, op } => typecheck_comparison_op(lhs, *op, rhs),
            Expr::And { lhs, rhs, .. } => typecheck_boolean_op(lhs, rhs),
            Expr::Or { lhs, rhs, .. } => typecheck_boolean_op(lhs, rhs),
            Expr::Not { expr, .. } => {
                if expr.typ() == Type::Bool {
                    Type::Bool
                } else {
                    Type::Error
                }
            }
            Expr::Id { expr } => expr.typ(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ComparisonOp {
    Eq,
    Neq,
    Lt,
    Leq,
    Gt,
    Geq,
    Like,
}

macro_rules! eq_op_impl {
    ($lhs:ident $op:tt $rhs:ident) => {
        match ($lhs, $rhs) {
            (Value::Bool(lhs), Value::Bool(rhs)) => Value::Bool(lhs == rhs),
            (Value::String(lhs), Value::String(rhs)) => Value::Bool(lhs == rhs),
            (Value::Int(lhs), Value::Int(rhs)) => Value::Bool(lhs == rhs),
            (_, _) => Value::None,
        }
    };
}

macro_rules! ord_op_impl {
    ($lhs:ident $op:tt $rhs:ident) => {
        match ($lhs, $rhs) {
            (Value::String(l), Value::String(r)) => Value::Bool(l $op r),
            (Value::Int(l), Value::Int(r)) => Value::Bool(l $op r),
            (_, _) => Value::None,
        }
    }
}

impl ComparisonOp {
    pub fn evaluate(&self, lhs: &Value, rhs: &Value) -> Value {
        match self {
            ComparisonOp::Eq => eq_op_impl!(lhs == rhs),
            ComparisonOp::Neq => eq_op_impl!(lhs != rhs),
            ComparisonOp::Lt => ord_op_impl!(lhs < rhs),
            ComparisonOp::Leq => ord_op_impl!(lhs <= rhs),
            ComparisonOp::Gt => ord_op_impl!(lhs > rhs),
            ComparisonOp::Geq => ord_op_impl!(lhs >= rhs),
            ComparisonOp::Like => match (lhs, rhs) {
                (Value::String(s), Value::Pattern(pattern)) => Value::Bool(pattern.is_match(s)),
                (_, _) => Value::None,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Bool(bool),
    String(String),
    Int(i64),
    Pattern(regex::Regex),
    InvalidPattern(String, SimpleSpan),
    None,
}

impl Value {
    pub fn is_none(&self) -> bool {
        matches!(self, Value::None)
    }
}

impl Typed for Value {
    fn typ(&self) -> Type {
        match self {
            Value::Bool(_) => Type::Bool,
            Value::String(_) => Type::String,
            Value::Int(_) => Type::Int,
            Value::Pattern(_) => Type::Pattern,
            Value::InvalidPattern(_, _) => Type::Error,
            Value::None => Type::None,
        }
    }
}

pub type ParseError<'src> = Rich<'src, char>;

fn validate_pattern<'src, E: extra::ParserExtra<'src, &'src str>>(
    pattern: &'src str,
    e: &mut chumsky::input::MapExtra<'src, '_, &'src str, E>,
    emitter: &mut chumsky::input::Emitter<ParseError<'src>>,
) -> Value {
    match Regex::new(pattern) {
        Ok(regex) => Value::Pattern(regex),
        Err(err) => {
            emitter.emit(Rich::custom(
                e.span(),
                "invalid regular expression: {err:?}",
            ));
            Value::InvalidPattern(pattern.into(), e.span())
        }
    }
}

#[allow(clippy::let_and_return)]
fn parser<'src>() -> impl Parser<'src, &'src str, Expr, extra::Err<ParseError<'src>>> {
    let str_escape = just('\\')
        .ignore_then(choice((
            just('\\'),
            just('/'),
            just('"'),
            just('b').to('\x08'),
            just('f').to('\x0C'),
            just('n').to('\n'),
            just('r').to('\r'),
            just('t').to('\t'),
            just('u').ignore_then(text::digits(16).exactly(4).to_slice().validate(
                |digits, e, emitter| {
                    char::from_u32(u32::from_str_radix(digits, 16).unwrap()).unwrap_or_else(|| {
                        emitter.emit(Rich::custom(e.span(), "invalid unicode character"));
                        '\u{FFFD}'
                    })
                },
            )),
        )))
        .boxed();

    let string = none_of("\\\"")
        .or(str_escape)
        .repeated()
        .collect::<String>()
        .map(Value::String)
        .delimited_by(just('"'), just('"'))
        .boxed();

    let pattern = just("\\/")
        .to('/')
        .or(none_of("/"))
        .repeated()
        .to_slice()
        .validate(validate_pattern)
        .delimited_by(just('/'), just('/'))
        .boxed();

    let number = just('-')
        .or_not()
        .then(text::int(10))
        .to_slice()
        .map(|x: &str| Value::Int(x.parse().unwrap()));

    let literal = choice((number, string, pattern))
        .padded()
        .map(Expr::Literal);

    let comparison_op = choice((
        just("==").to(ComparisonOp::Eq),
        just("!=").to(ComparisonOp::Neq),
        just("<").to(ComparisonOp::Lt),
        just("<=").to(ComparisonOp::Leq),
        just(">").to(ComparisonOp::Gt),
        just(">=").to(ComparisonOp::Geq),
        just("~").to(ComparisonOp::Like),
        just("like").to(ComparisonOp::Like),
    ))
    .padded();

    let boolean_op = |a, b| just(a).or(just(b)).ignored().padded();

    let field = text::ident().map(|ident| Expr::Ident(Field::from_str(ident)));

    let expr = recursive(|expr| {
        use chumsky::pratt::*;
        let nested = expr
            .delimited_by(just('('), just(')'))
            .padded()
            .map(|e| Expr::Id { expr: Box::new(e) });

        let atom = choice((literal, field, nested)).padded();

        atom.pratt((
            infix(left(3), comparison_op, |l, op, r, _| Expr::Comparison {
                lhs: Box::new(l),
                rhs: Box::new(r),
                op,
            }),
            infix(left(1), boolean_op("and", "&&"), |l, _op, r, _| Expr::And {
                lhs: Box::new(l),
                rhs: Box::new(r),
            }),
            infix(left(2), boolean_op("or", "||"), |l, _op, r, _| Expr::Or {
                lhs: Box::new(l),
                rhs: Box::new(r),
            }),
            prefix(2, boolean_op("not", "!"), |_op, e, _| Expr::Not {
                expr: Box::new(e),
            }),
        ))
    });

    expr
}

pub fn parse_expr<'src>(input: &'src str) -> Result<Expr, Vec<ParseError<'src>>> {
    let result = parser().parse(input).into_result().unwrap();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use color_eyre::eyre::{self, WrapErr, bail};
    use indoc::indoc;

    #[test]
    fn test_parse_str_literal() -> eyre::Result<()> {
        let tests = vec![
            (r#""test""#, "test"),
            (r#""something else goes here""#, "something else goes here"),
            (r#""new\nline""#, "new\nline"),
        ];

        for (input, expected) in tests {
            let output = parse_expr(input).unwrap();
            if let Expr::Literal(Value::String(result)) = output {
                assert_eq!(result, expected);
            } else {
                bail!("expected string literal, got {output:?}");
            }
        }

        Ok(())
    }

    #[test]
    fn test_parse_pattern_literal() -> eyre::Result<()> {
        let tests = vec![(r"/test/", "test"), (r"/a|b+/", "a|b+")];

        for (input, expected) in tests {
            let output = parse_expr(input).unwrap();
            if let Expr::Literal(Value::Pattern(result)) = output {
                assert_eq!(result.as_str(), expected);
            } else {
                bail!("expected pattern literal, got {output:?}");
            }
        }

        Ok(())
    }

    #[test]
    fn test_parse_simple_expression() -> eyre::Result<()> {
        let expr = parse_expr(r#"season > 2 and description like /.*XL./"#)
            .map_err(|e| eyre::eyre!("parse errors: {e:?}"))?;
        dbg!(&expr);
        let filter = Filter::from_expr(expr)?;

        Ok(())
    }
}
