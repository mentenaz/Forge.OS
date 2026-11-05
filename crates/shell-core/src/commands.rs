//! The built-in commands.

use core::fmt::Write;

use crate::terminal::Terminal;

/// What the shell should do after a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Keep going.
    Continue,
    /// Leave the shell.
    Exit,
}

/// Every built-in command with a one-line description, in the order `help`
/// lists them.
pub const COMMANDS: [(&str, &str); 5] = [
    ("help", "list the commands"),
    ("echo", "print the arguments"),
    ("clear", "clear the screen"),
    ("uptime", "time since the machine started"),
    ("exit", "leave the shell"),
];

/// Run one line. Every command leaves the cursor at the start of a row.
pub(crate) fn execute<const C: usize, const R: usize>(
    line: &str,
    term: &mut Terminal<C, R>,
    now_ns: u64,
) -> Flow {
    let mut words = line.split_ascii_whitespace();
    let Some(name) = words.next() else {
        return Flow::Continue;
    };
    match name {
        "help" => {
            for (name, about) in COMMANDS {
                let _ = writeln!(term, "{name:<8}{about}");
            }
        }
        "echo" => {
            for (index, word) in words.enumerate() {
                if index > 0 {
                    term.put_char(' ');
                }
                let _ = term.write_str(word);
            }
            term.newline();
        }
        "clear" => term.clear(),
        "uptime" => {
            let seconds = now_ns / 1_000_000_000;
            let _ = writeln!(
                term,
                "up {}h {}m {}s",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            );
        }
        "exit" => return Flow::Exit,
        other => {
            let _ = writeln!(term, "{other}: command not found");
        }
    }
    Flow::Continue
}
