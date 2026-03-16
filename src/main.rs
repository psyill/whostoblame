use std::{collections::HashMap, env::args, error::Error, process::Command};

use regex_lite::Regex;

type User = String;
type UserLines = HashMap<User, u32>;
const TOP_USER_COUNT: usize = 5;

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

    fn parse_blame(&self, blame: &str) -> UserLines {
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

fn blame(file_name: String, parser: &Parser) -> Result<UserLines, Box<dyn Error>> {
    let blame_result = Command::new("git")
        .arg("blame")
        .arg("--porcelain")
        .arg(file_name)
        .output()?
        .stdout;
    Ok(parser.parse_blame(str::from_utf8(&blame_result)?))
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut user_lines = UserLines::new();
    let parser = Parser::new();
    for file_name in args() {
        for (user, lines) in blame(file_name, &parser)? {
            *user_lines.entry(user).or_default() += lines;
        }
    }
    let mut users_and_lines: Vec<_> = user_lines.into_iter().collect();
    users_and_lines.sort_by_key(|(_, lines)| *lines);
    println!("Changed lines:");
    for (user, lines) in users_and_lines.iter().take(TOP_USER_COUNT) {
        println!("\t{user} ({lines})");
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use std::assert_matches;

    use super::*;

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

    #[test]
    fn test_parse_simple_blame() {
        let parser = Parser::new();
        let user_lines: UserLines = parser.parse_blame(TEST_BLAME_OUTPUT);
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
