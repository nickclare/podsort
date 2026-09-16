#![allow(unused)] // TODO: for development
use super::*;
use chumsky::input::{BorrowInput, MappedInput, StrInput, Stream, ValueInput};
use chumsky::prelude::*;
use color_eyre::eyre;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Type {
    Bool,
    Int,
    String,
    Pattern,
    Error,
}

trait Typed {
    fn typ(&self) -> Type;
}

#[derive(Debug)]
pub struct Filter {
    // This must have ty == bool to be valid
    expr: Expr,
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

#[derive(Debug)]
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

impl Typed for Expr {
    fn typ(&self) -> Type {
        match self {
            Expr::Literal(value) => value.typ(),
            Expr::Ident(field) => field.typ(),
            Expr::Comparison { .. } => Type::Bool,
            Expr::And { .. } => Type::Bool,
            Expr::Or { .. } => Type::Bool,
            Expr::Not { .. } => Type::Bool,
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

#[derive(Debug)]
pub enum Value {
    Bool(bool),
    String(String),
    Int(i64),
    Pattern(regex::Regex),
    InvalidPattern(String, SimpleSpan),
}

impl Typed for Value {
    fn typ(&self) -> Type {
        match self {
            Value::Bool(_) => Type::Bool,
            Value::String(_) => Type::String,
            Value::Int(_) => Type::Int,
            Value::Pattern(_) => Type::Pattern,
            Value::InvalidPattern(_, _) => Type::Error,
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
    use color_eyre::eyre::{self, bail};
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

    // #[test]
    fn test_parse_simple_expression() -> eyre::Result<()> {
        bail!("{:?}", parse_expr(r#"summary like /no/ and season == 2"#))
    }
}
