//! WASI Entry Point for OutLayer
//!
//! Reads JSON request from stdin, executes, writes response to stdout.

use key_manager::execute;
use std::io::{self, Read, Write};

fn main() {
    // Read input from stdin
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("Failed to read stdin");

    // Execute
    let output = execute(&input);

    // Write output to stdout
    io::stdout()
        .write_all(output.as_bytes())
        .expect("Failed to write stdout");
}
