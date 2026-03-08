use std::{collections::HashMap, env::args, error::Error, process::Command};

type User = String;
type UserLines = HashMap<User, u32>;
const TOP_USER_COUNT: usize = 5;

fn blame(file_name: String) -> Result<UserLines, Box<dyn Error>> {
    let blame_result = Command::new("git")
        .arg("blame")
        .arg("--porcelain")
        .arg(file_name)
        .output()?.stdout;
    let mut result = UserLines::new();
    let mut commit_to_user = HashMap::<String, String>::new();
    let mut lines = str::from_utf8(&blame_result)?.lines();
    for line in lines {
        // Lines starting with tab are real lines, while we are only interested in metadata.
        if line.starts_with('\t') {
            continue;
        }
    }
    Ok(result)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut user_lines = UserLines::new();
    for file_name in args() {
        for (user, lines) in blame(file_name)? {
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
