use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, stdout, BufRead, BufReader, Write};

/// Loads shell command history from the appropriate file.
fn load_history() -> io::Result<Vec<String>> {
    let home_dir = env::var("HOME").expect("Could not determine HOME directory");
    let shell = env::var("SHELL").expect("Could not determine SHELL");
    let history_path = match shell.as_str() {
        "/bin/bash" | "/usr/bin/bash" => format!("{}/.bash_history", home_dir),
        "/bin/zsh" | "/usr/bin/zsh" => format!("{}/.zsh_history", home_dir),
        "/usr/bin/fish" | "/bin/fish" => format!("{}/.local/share/fish/fish_history", home_dir),
        _ => panic!("Unsupported shell: {}", shell),
    };

    let file = File::open(&history_path)
        .unwrap_or_else(|_| panic!("Failed to open history file at {}", history_path));
    let reader = BufReader::new(file);

    let mut commands = Vec::new();
    for line in reader.lines() {
        if let Ok(command) = line {
            let trimmed = command.trim();
            if !trimmed.is_empty() {
                commands.push(trimmed.to_string());
            }
        }
    }
    Ok(commands)
}

/// Builds a frequency map for the list of commands.
fn build_frequency_map(commands: &[String]) -> HashMap<String, usize> {
    let mut freq = HashMap::new();
    for cmd in commands {
        *freq.entry(cmd.clone()).or_insert(0) += 1;
    }
    freq
}

/// Truncates a given string to fit within the specified width.
fn truncate_to_width(s: &str, width: u16) -> String {
    s.chars().take(width as usize).collect()
}

/// Runs the interactive command search UI.
fn run_ui() -> io::Result<()> {
    // Load history and compute command frequencies.
    let commands = load_history().expect("Failed to load history");
    let frequency = build_frequency_map(&commands);

    let mut query = String::new();
    let mut selected_index: usize = 0;

    // Set up terminal: enable raw mode, enter alternate screen, and hide cursor.
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, Hide)?;

    loop {
        // Get terminal size.
        let (term_width, _) = crossterm::terminal::size()?;

        // Filter suggestions matching the query (case-insensitive) and sort them.
        let mut suggestions: Vec<(String, usize)> = frequency
            .iter()
            .filter(|(cmd, _)| cmd.to_lowercase().contains(&query.to_lowercase()))
            .map(|(cmd, &count)| (cmd.clone(), count))
            .collect();

        suggestions.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        // Limit the suggestions to a maximum.
        let max_suggestions = 10;
        if suggestions.len() > max_suggestions {
            suggestions.truncate(max_suggestions);
        }

        // Adjust selected index if necessary.
        if selected_index >= suggestions.len() {
            selected_index = suggestions.len().saturating_sub(1);
        }

        // Clear the screen and display the prompt along with suggestions.
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        let header =
            "Type your search query. Use ↑/↓ to select. Press Enter to choose. (Esc to exit)";
        writeln!(stdout, "{}", truncate_to_width(header, term_width))?;
        writeln!(
            stdout,
            "{}",
            truncate_to_width(&format!("Search: {}", query), term_width)
        )?;
        writeln!(stdout)?;

        for (i, (cmd, count)) in suggestions.iter().enumerate() {
            let line = if i == selected_index {
                format!("> {} ({})", cmd, count)
            } else {
                format!("  {} ({})", cmd, count)
            };
            writeln!(stdout, "{}", truncate_to_width(&line, term_width))?;
        }
        stdout.flush()?;

        // Process user input.
        match event::read()? {
            Event::Key(KeyEvent { code, .. }) => match code {
                KeyCode::Char(c) => {
                    query.push(c);
                    selected_index = 0;
                }
                KeyCode::Backspace => {
                    query.pop();
                    selected_index = 0;
                }
                KeyCode::Up => {
                    if selected_index > 0 {
                        selected_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if selected_index + 1 < suggestions.len() {
                        selected_index += 1;
                    }
                }
                KeyCode::Enter => {
                    // Cleanup terminal before exiting.
                    execute!(
                        stdout,
                        Clear(ClearType::All),
                        MoveTo(0, 0),
                        Show,
                        LeaveAlternateScreen
                    )?;
                    disable_raw_mode()?;
                    if !suggestions.is_empty() {
                        println!("Selected command:\n{}", suggestions[selected_index].0);
                    } else {
                        println!("No matching commands found.");
                    }
                    return Ok(());
                }
                KeyCode::Esc => {
                    // Cleanup terminal on exit.
                    execute!(
                        stdout,
                        Clear(ClearType::All),
                        MoveTo(0, 0),
                        Show,
                        LeaveAlternateScreen
                    )?;
                    disable_raw_mode()?;
                    println!("Exited.");
                    return Ok(());
                }
                _ => {}
            },
            Event::Resize(_, _) => {
                // The UI will redraw on the next loop iteration.
            }
            _ => {}
        }
    }
}

fn main() -> io::Result<()> {
    // Run the UI and ensure that the terminal state is restored in case of an error.
    let result = run_ui();
    if result.is_err() {
        let mut stdout = stdout();
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
    result
}
