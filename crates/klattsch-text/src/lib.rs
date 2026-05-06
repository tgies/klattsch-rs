//! Parser and schedule compiler for klattsch phoneme strings.
//!
//! Compiles a phoneme string (e.g. `"HH AH L OW"`) into a
//! [`klattsch_core::Schedule`].

pub mod compile;
pub mod normalize;
pub mod tokenize;

pub use compile::{
    compile, compile_string, CompileError, CompileOptions, CompileResult, Phrase, PhraseKind,
};
pub use tokenize::{tokenize, DirectiveKey, ParseError, Span, Token, Tokenized};

/// Combined error type for [`compile_string`], which can fail at either the
/// tokenize or compile stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileOrParseError {
    Parse(ParseError),
    Compile(CompileError),
}

impl core::fmt::Display for CompileOrParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "{e}"),
            Self::Compile(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CompileOrParseError {}
