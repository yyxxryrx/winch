use colored::{Colorize, control};
use std::fmt::Display;
use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);
static QUIET: AtomicBool = AtomicBool::new(false);

pub fn init(verbose: bool, quiet: bool) {
    VERBOSE.store(verbose, Ordering::Relaxed);
    QUIET.store(quiet, Ordering::Relaxed);

    let color = if std::env::var_os("NO_COLOR").is_some()
        || std::env::var("CLICOLOR").ok().as_deref() == Some("0")
    {
        false
    } else {
        match std::env::var("FORCE_COLOR").ok().as_deref() {
            Some("0") => false,
            Some(_) => true,
            None => std::io::stderr().is_terminal(),
        }
    };
    control::set_override(color);
}

pub fn loaded(message: impl Display) {
    emit("Loaded", Style::Green, message, false);
}

pub fn found(message: impl Display) {
    emit("Found", Style::Green, message, false);
}

pub fn saved(message: impl Display) {
    emit("Saved", Style::Green, message, false);
}

pub fn cached(message: impl Display) {
    emit("Cached", Style::Cyan, message, false);
}

pub fn finished(message: impl Display) {
    emit("Finished", Style::Green, message, false);
}

pub fn warning(message: impl Display) {
    emit("Warning", Style::Yellow, message, false);
}

pub fn error(message: impl Display) {
    emit("Error", Style::Red, message, true);
}

pub fn debug(message: impl Display) {
    if VERBOSE.load(Ordering::Relaxed) {
        emit("Debug", Style::Dim, message, false);
    }
}

#[derive(Clone, Copy)]
enum Style {
    Green,
    Cyan,
    Yellow,
    Red,
    Dim,
}

fn emit(label: &str, style: Style, message: impl Display, force: bool) {
    if QUIET.load(Ordering::Relaxed) && !force {
        return;
    }

    let label = format!("{label:>8}");
    let label = match style {
        Style::Green => label.green().bold(),
        Style::Cyan => label.cyan().bold(),
        Style::Yellow => label.yellow().bold(),
        Style::Red => label.red().bold(),
        Style::Dim => label.bright_black(),
    };
    eprintln!("{label} {message}");
}
