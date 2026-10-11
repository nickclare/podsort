#![deny(unsafe_code)]

pub mod matchers;
pub mod runner;

use matchers::dsl;
use serde::{Deserialize, Deserializer, de::Visitor};
use std::path::PathBuf;
use url::Url;

use crate::matchers::dsl::Expr;

#[derive(Debug, Clone, Deserialize)]
pub struct Output {
    pub id: String,
    pub output: PathBuf,

    #[serde(deserialize_with = "parse_matcher_expr")]
    pub matcher: Box<Expr>,
}

#[doc(hidden)]
fn parse_matcher_expr<'de, D>(deserializer: D) -> Result<Box<Expr>, D::Error>
where
    D: Deserializer<'de>,
{
    struct ExprVisitor;
    impl<'de> Visitor<'de> for ExprVisitor {
        type Value = Box<Expr>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "expecting a matcher expression as a string")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            match dsl::parse_expr(v) {
                Ok(expr) => Ok(Box::new(expr)),
                Err(e) => Err(E::custom(format!(
                    "failed to parse matcher expression: {e:?}"
                ))),
            }
        }
    }
    deserializer.deserialize_str(ExprVisitor)
}

#[derive(Debug, Clone, Deserialize)]
pub struct PodcastJob {
    pub id: String,
    pub source: Url,

    pub outputs: Vec<Output>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use color_eyre::eyre::{self};
    use indoc::indoc;

    #[test]
    fn parse_basic_config() -> eyre::Result<()> {
        let input = indoc! {r#"
            id = "nstaaf"
            source = "https://audioboom.com/channels/2399216.rss"
            
            [[outputs]]
            id = "little-fish"
            output = "/tmp/little-fish.rss"
            matcher = "title like /Little Fish/"
        "#};

        let job = toml::from_str::<PodcastJob>(input)?;
        assert_eq!(job.outputs.len(), 1);
        let output = job.outputs.first().unwrap();
        assert!(matches!(
            *output.matcher,
            Expr::Comparison {
                lhs: _,
                op: dsl::ComparisonOp::Like,
                rhs: _
            }
        ));
        Ok(())
    }
}
