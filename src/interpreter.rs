//! Execute a validated program with an explicit output destination.

use std::io::{self, Write};

use crate::ast::{Program, Statement};

pub fn execute(program: &Program, output: &mut impl Write) -> io::Result<()> {
    for statement in &program.statements {
        match statement {
            Statement::Println { value, .. } => writeln!(output, "{value}")?,
        }
    }
    Ok(())
}
