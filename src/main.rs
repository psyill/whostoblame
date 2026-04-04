use core::str;
use std::{borrow::Cow, cmp::Reverse, collections::HashMap, env::args, io, process::Command};

use regex_lite::Regex;

#[cfg(feature = "parallelization")]
use rayon::prelude::*;
#[cfg(feature = "parallelization")]
use std::sync::mpsc::channel;

type User = String;
type Lines = u32;
type UserLines = HashMap<User, Lines>;
const TOP_USER_COUNT: usize = 10;

struct Parser {
    group_header_re: Regex,
}

impl Parser {
    fn new() -> Self {
        Self {
            group_header_re: Regex::new(
                r"^(?<sha1>[[:xdigit:]]{40}) \d+ \d+ (?<lines_in_group>\d+)",
            )
            .unwrap(),
        }
    }

    fn find_author(line: &str) -> Option<&str> {
        const AUTHOR_PREFIX: &str = "author ";
        const AUTHOR_INDEX: usize = AUTHOR_PREFIX.len();
        if line.starts_with(AUTHOR_PREFIX) {
            return Some(&line[AUTHOR_INDEX..]);
        }
        None
    }

    fn parse_blame(&self, blame: Cow<'_, str>) -> UserLines {
        let mut result = UserLines::new();
        let mut commit_to_user = HashMap::<String, String>::new();
        let mut lines = blame
            .lines()
            // Lines starting with tab are real lines, while we are only interested in metadata.
            .filter(|line| !line.starts_with('\t'));
        while let Some(captures) = lines.find_map(|line| self.group_header_re.captures(line)) {
            let number_of_lines_in_group: u32 = captures
                .name("lines_in_group")
                .unwrap()
                .as_str()
                .parse::<u32>()
                .unwrap();
            let sha1: &str = captures.name("sha1").unwrap().as_str();
            let entry = commit_to_user.entry(sha1.into());
            let user: String = entry
                .or_insert_with(|| {
                    lines
                        .find_map(|line| Self::find_author(line))
                        .unwrap_or("unknown")
                        .into()
                })
                .to_string();
            *result.entry(user).or_default() += number_of_lines_in_group;
        }
        result
    }
}

fn blame(file_name: &str, parser: &Parser) -> Result<UserLines, io::Error> {
    let blame_result = Command::new("git")
        .arg("blame")
        .arg("--porcelain")
        .arg("--incremental")
        .arg("--")
        .arg(file_name)
        .output()?
        .stdout;
    Ok(parser.parse_blame(String::from_utf8_lossy(&blame_result)))
}

#[cfg(not(feature = "parallelization"))]
fn acquire_user_lines(parser: &Parser) -> UserLines {
    let mut user_lines = UserLines::new();
    args().skip(1).for_each(|file_name| {
        for (user, lines) in blame(file_name.as_str(), parser).unwrap_or_else(|e| {
            eprintln!("W: failed blaming {file_name} due to {e}");
            UserLines::new()
        }) {
            *user_lines.entry(user).or_default() += lines;
        }
    });
    user_lines
}

#[cfg(feature = "parallelization")]
fn acquire_user_lines(parser: &Parser) -> UserLines {
    let mut user_lines = UserLines::new();
    let arguments: Vec<String> = args().skip(1).collect();
    let (tx, rx) = channel::<(User, Lines)>();
    arguments.par_iter().for_each(|file_name| {
        let tx = tx.clone();
        for entry in blame(file_name.as_str(), parser).unwrap_or_else(|e| {
            eprintln!("W: failed blaming {file_name} due to {e}");
            UserLines::new()
        }) {
            tx.send(entry).unwrap_or_else(|e| {
                eprintln!("W: failed sending data due to {e}");
            });
        }
    });
    drop(tx);
    while let Ok((user, lines)) = rx.recv() {
        *user_lines.entry(user).or_default() += lines;
    }
    user_lines
}

fn main() {
    println!("Changed lines:");
    let parser = Parser::new();
    let user_lines: UserLines = acquire_user_lines(&parser);
    let mut users_and_lines: Vec<_> = user_lines.into_iter().collect();
    // Sort the users by number of lines change, descending.
    users_and_lines.sort_by_key(|(_, lines)| Reverse(*lines));
    for (user, lines) in users_and_lines.iter().take(TOP_USER_COUNT) {
        println!("\t{user} ({lines})");
    }
}

#[cfg(test)]
mod test {
    use std::assert_matches;

    use super::*;

    #[test]
    fn test_parse_simple_blame() {
        /// This blame output was generated for real by Git.
        const TEST_BLAME_OUTPUT: &str = r#"492c3466109b3816aaf568bd947b1a01ac452c37 1 1 6
author Hans Ellegård
author-mail <psyill.net@gmail.com>
author-time 1772996316
author-tz +0100
committer Hans Ellegård
committer-mail <psyill.net@gmail.com>
committer-time 1772996316
committer-tz +0100
summary Set up skeleton for whostoblame
boundary
filename Cargo.toml
	[package]
492c3466109b3816aaf568bd947b1a01ac452c37 2 2
	name = "whostoblame"
492c3466109b3816aaf568bd947b1a01ac452c37 3 3
	version = "0.1.0"
492c3466109b3816aaf568bd947b1a01ac452c37 4 4
	edition = "2024"
492c3466109b3816aaf568bd947b1a01ac452c37 5 5
	
492c3466109b3816aaf568bd947b1a01ac452c37 6 6
	[dependencies]
"#;

        let parser = Parser::new();
        let user_lines: UserLines = parser.parse_blame(Cow::Borrowed(TEST_BLAME_OUTPUT));
        assert_eq!(
            user_lines.len(),
            1,
            "Number of authors in the result is wrong"
        );
        assert!(
            user_lines.contains_key("Hans Ellegård"),
            "Didn't find expected author"
        );
        assert_matches!(
            user_lines.values().next(),
            Some(6_u32),
            "The number of lines for the only author is wrong"
        );
    }
}
