pub mod dsl;

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
    fn get_from_item<'a>(&'_ self, item: &'a rss::Item) -> Option<&'a str> {
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

#[derive(Debug)]
pub struct RegexMatcher {
    field: Field,
    pattern: Regex,
}

impl Matcher for RegexMatcher {
    fn matches(&self, item: &rss::Item) -> bool {
        if let Some(value) = self.field.get_from_item(item) {
            self.pattern.is_match(value)
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub struct ExactMatcher {
    field: Field,
    value: String,
}

impl Matcher for ExactMatcher {
    fn matches(&self, item: &rss::Item) -> bool {
        if let Some(value) = self.field.get_from_item(item) {
            self.value == value
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub struct NotMatcher {
    inner: Box<dyn Matcher>,
}

impl Matcher for NotMatcher {
    fn matches(&self, item: &rss::Item) -> bool {
        !self.inner.matches(item)
    }
}

#[derive(Debug)]
pub struct AndMatcher {
    clauses: Vec<Box<dyn Matcher>>,
}

impl Matcher for AndMatcher {
    fn matches(&self, item: &rss::Item) -> bool {
        self.clauses.iter().all(|m| m.matches(item))
    }
}

#[derive(Debug)]
pub struct OrMatcher {
    clauses: Vec<Box<dyn Matcher>>,
}

impl Matcher for OrMatcher {
    fn matches(&self, item: &rss::Item) -> bool {
        self.clauses.iter().any(|m| m.matches(item))
    }
}
