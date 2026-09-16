#![deny(unsafe_code)]

pub(crate) mod db;
pub mod matchers;

use matchers::Matcher;

use std::io::BufRead;

use color_eyre::eyre::{self, Context};

/// Sort a podcast stream into multiple streams, based on a set of `Matchers`
pub struct PodcastSorter<'a> {
    id: String,
    channel: rss::Channel,
    matchers: Vec<&'a dyn Matcher>,
}

impl<'a> PodcastSorter<'a> {
    pub fn from_reader<I: Into<String>, S: BufRead>(id: I, input: S) -> eyre::Result<Self> {
        let channel = rss::Channel::read_from(input).wrap_err("unable to parse input feed")?;

        Ok(Self {
            id: id.into(),
            channel,
            matchers: Vec::new(),
        })
    }

    pub fn add_matcher(&mut self, matcher: &'a dyn Matcher) -> &mut Self {
        self.matchers.push(matcher);
        self
    }
}
