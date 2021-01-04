use std::fmt::{self, Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BfIR {
    AddVal(u8),  // +
    SubVal(u8),  // -
    AddPtr(u32), // >
    SubPtr(u32), // <
    GetByte,     // ,
    PutByte,     // .
    Jz,          // [
    Jnz,         // ]
}

#[derive(Debug, thiserror::Error)]
pub enum CompileErrorKind {
    #[error("Unclosed left bracket")]
    UnclosedLeftBracket,
    #[error("Unexpected right bracket")]
    UnexpectedRightBracket,
}

#[derive(Debug)]
pub struct CompileError {
    line: u32,
    col: u32,
    kind: CompileErrorKind,
}

impl Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at line {}:{}", self.kind, self.line, self.col)
    }
}

impl std::error::Error for CompileError {}

pub fn compile(src: &str) -> Result<Vec<BfIR>, CompileError> {
    let mut code = vec![];
    let mut stk = vec![];
    let mut line = 1;
    let mut col = 0;
    for ch in src.chars() {
        col += 1;
        match ch {
            '\n' => {
                line += 1;
                col = 0;
            }
            '+' => {
                if let Some(BfIR::AddVal(n)) = code.last_mut() {
                    *n += 1;
                } else {
                    code.push(BfIR::AddVal(1));
                }
            }
            '-' => {
                if let Some(BfIR::SubVal(n)) = code.last_mut() {
                    *n += 1;
                } else {
                    code.push(BfIR::SubVal(1));
                }
            }
            '>' => {
                if let Some(BfIR::AddPtr(n)) = code.last_mut() {
                    *n += 1;
                } else {
                    code.push(BfIR::AddPtr(1));
                }
            }
            '<' => {
                if let Some(BfIR::SubPtr(n)) = code.last_mut() {
                    *n += 1;
                } else {
                    code.push(BfIR::SubPtr(1));
                }
            }
            ',' => code.push(BfIR::GetByte),
            '.' => code.push(BfIR::PutByte),
            '[' => {
                stk.push((line, col));
                code.push(BfIR::Jz)
            }
            ']' => {
                if stk.pop().is_none() {
                    return Err(CompileError {
                        line,
                        col,
                        kind: CompileErrorKind::UnexpectedRightBracket,
                    });
                }
                code.push(BfIR::Jnz)
            }
            _ => {}
        }
    }
    if let Some((line, col)) = stk.pop() {
        return Err(CompileError {
            line,
            col,
            kind: CompileErrorKind::UnclosedLeftBracket,
        });
    }
    Ok(code)
}
