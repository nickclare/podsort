pub mod dsl;

use dsl::*;
use regex::Regex;

pub trait Matcher: std::fmt::Debug {
    /// Checks if the item matches some criteria
    fn matches(&self, item: &rss::Item) -> bool;
}

#[derive(Clone, Debug)]
pub enum Field {
    Title,
    Description,
    PubDate,
    Summary,
    Keywords,
    Episode,
    EpisodeType,
    Season,

    Unknown(String),
}

impl Field {
    pub(crate) fn get_from_item<'a>(&'_ self, item: &'a rss::Item) -> Option<&'a str> {
        match self {
            Field::Title => item.title(),
            Field::Description => item.description(),
            Field::PubDate => item.pub_date(),
            Field::Summary => item.itunes_ext().and_then(|it| it.summary()),
            Field::Keywords => item.itunes_ext().and_then(|it| it.keywords()),
            Field::Episode => item.itunes_ext().and_then(|it| it.episode()),
            Field::EpisodeType => item.itunes_ext().and_then(|it| it.episode_type()),
            Field::Season => item.itunes_ext().and_then(|it| it.season()),
            Field::Unknown(_) => todo!(),
        }
    }

    fn from_str(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "title" => Self::Title,
            "description" => Self::Description,
            "pubdate" => Self::PubDate,
            "summary" => Self::Summary,
            "keywords" => Self::Keywords,
            "episode" => Self::Episode,
            "episodetype" => Self::EpisodeType,
            "season" => Self::Season,

            _other => Field::Unknown(value.into()),
        }
    }
}

impl Matcher for dsl::Filter {
    fn matches(&self, item: &rss::Item) -> bool {
        match self.expr.evaluate(item) {
            dsl::Value::Bool(result) => result,
            _ => unreachable!(),
        }
    }
}

impl Expr {
    fn evaluate(&self, item: &rss::Item) -> Value {
        match self {
            dsl::Expr::Literal(value) => value.clone(),
            dsl::Expr::Ident(field) => {
                let typ = field.typ();
                match field.get_from_item(item) {
                    Some(value) => typ.parse_value(value).unwrap(),
                    None => Value::None,
                }
            }
            dsl::Expr::Comparison { lhs, op, rhs } => {
                let lhs = lhs.evaluate(item);
                let rhs = rhs.evaluate(item);
                if lhs.is_none() || rhs.is_none() {
                    Value::None
                } else {
                    op.evaluate(&lhs, &rhs)
                }
            }
            dsl::Expr::And { lhs, rhs } => match (lhs.evaluate(item), rhs.evaluate(item)) {
                (Value::Bool(l), Value::Bool(r)) => Value::Bool(l && r),
                (_, _) => Value::None,
            },
            dsl::Expr::Or { lhs, rhs } => match (lhs.evaluate(item), rhs.evaluate(item)) {
                (Value::Bool(l), Value::Bool(r)) => Value::Bool(l || r),
                (_, _) => Value::None,
            },
            dsl::Expr::Not { expr } => match expr.evaluate(item) {
                Value::Bool(value) => Value::Bool(!value),
                _ => Value::None,
            },
            dsl::Expr::Id { expr } => expr.evaluate(item),
        }
    }
}
