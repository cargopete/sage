//! Rust code emitter.
//!
//! This module provides utilities for generating well-formatted Rust code.

/// A code emitter that handles indentation and formatting.
pub struct Emitter {
    /// The output buffer.
    output: String,
    /// Current indentation level.
    indent: usize,
    /// Whether we're at the start of a line.
    at_line_start: bool,
}

impl Emitter {
    /// Create a new emitter.
    pub fn new() -> Self {
        Self {
            output: String::with_capacity(4096),
            indent: 0,
            at_line_start: true,
        }
    }

    /// Get the generated output.
    pub fn finish(self) -> String {
        self.output
    }

    /// Increase indentation.
    pub fn indent(&mut self) {
        self.indent += 1;
    }

    /// Decrease indentation.
    pub fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    /// Write raw text (no indentation handling).
    pub fn write_raw(&mut self, s: &str) {
        self.output.push_str(s);
        self.at_line_start = false;
    }

    /// Write text, adding indentation if at line start.
    pub fn write(&mut self, s: &str) {
        if self.at_line_start && !s.is_empty() {
            self.write_indent();
        }
        self.output.push_str(s);
        self.at_line_start = false;
    }

    /// Write text followed by a newline.
    pub fn writeln(&mut self, s: &str) {
        self.write(s);
        self.newline();
    }

    /// Write just a newline.
    pub fn newline(&mut self) {
        self.output.push('\n');
        self.at_line_start = true;
    }

    /// Write the current indentation.
    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        self.at_line_start = false;
    }

    /// Write an opening brace and increase indent.
    pub fn open_brace(&mut self) {
        self.writeln("{");
        self.indent();
    }

    /// Decrease indent and write a closing brace.
    pub fn close_brace(&mut self) {
        self.dedent();
        self.writeln("}");
    }

    /// Decrease indent and write a closing brace without newline.
    pub fn close_brace_inline(&mut self) {
        self.dedent();
        self.write("}");
    }

    /// Write a blank line.
    pub fn blank_line(&mut self) {
        self.newline();
    }
}

impl Default for Emitter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_emit() {
        let mut e = Emitter::new();
        e.writeln("fn main() {");
        e.indent();
        e.writeln("println!(\"hello\");");
        e.dedent();
        e.writeln("}");

        let output = e.finish();
        assert_eq!(
            output,
            "fn main() {\n    println!(\"hello\");\n}\n"
        );
    }

    #[test]
    fn nested_braces() {
        let mut e = Emitter::new();
        e.write("if true ");
        e.open_brace();
        e.write("if false ");
        e.open_brace();
        e.writeln("inner();");
        e.close_brace();
        e.close_brace();

        let output = e.finish();
        assert!(output.contains("        inner();"));
    }
}
