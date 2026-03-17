//! Token definitions for the Sage lexer.

use logos::Logos;

/// All tokens in the Sage language.
#[derive(Logos, Debug, Clone, PartialEq, Eq, Hash)]
#[logos(skip r"[ \t\r\n]+")]
#[logos(skip r"//[^\n]*")]
pub enum Token {
    // =========================================================================
    // Keywords
    // =========================================================================
    #[token("agent")]
    KwAgent,

    #[token("belief")]
    KwBelief,

    #[token("on")]
    KwOn,

    #[token("start")]
    KwStart,

    #[token("stop")]
    KwStop,

    /// v2 lifecycle: runs before start, after persistent state loaded.
    #[token("waking")]
    KwWaking,

    /// v2 lifecycle: runs when supervisor signals graceful pause.
    #[token("pause")]
    KwPause,

    /// v2 lifecycle: runs when agent is unpaused.
    #[token("resume")]
    KwResume,

    /// v2 lifecycle: alias for stop (v2 terminology).
    #[token("resting")]
    KwResting,

    #[token("message")]
    KwMessage,

    #[token("divine")]
    KwDivine,

    #[token("summon")]
    KwSummon,

    #[token("await")]
    KwAwait,

    #[token("send")]
    KwSend,

    #[token("yield")]
    KwYield,

    #[token("run")]
    KwRun,

    #[token("fn")]
    KwFn,

    #[token("let")]
    KwLet,

    #[token("return")]
    KwReturn,

    #[token("if")]
    KwIf,

    #[token("else")]
    KwElse,

    #[token("for")]
    KwFor,

    #[token("while")]
    KwWhile,

    #[token("loop")]
    KwLoop,

    #[token("break")]
    KwBreak,

    #[token("in")]
    KwIn,

    #[token("self")]
    KwSelf,

    #[token("true")]
    KwTrue,

    #[token("false")]
    KwFalse,

    #[token("mod")]
    KwMod,

    #[token("use")]
    KwUse,

    #[token("pub")]
    KwPub,

    #[token("as")]
    KwAs,

    #[token("super")]
    KwSuper,

    #[token("record")]
    KwRecord,

    #[token("enum")]
    KwEnum,

    #[token("match")]
    KwMatch,

    #[token("const")]
    KwConst,

    #[token("receives")]
    KwReceives,

    #[token("receive")]
    KwReceive,

    #[token("fail")]
    KwFail,

    #[token("fails")]
    KwFails,

    #[token("timeout")]
    KwTimeout,

    #[token("retry")]
    KwRetry,

    #[token("delay")]
    KwDelay,

    #[token("try")]
    KwTry,

    #[token("catch")]
    KwCatch,

    #[token("error")]
    KwError,

    #[token("tool")]
    KwTool,

    /// RFC-0012: Test declaration keyword.
    #[token("test")]
    KwTest,

    /// RFC-0012: Mock keyword for LLM mocking.
    #[token("mock")]
    KwMock,

    /// Trace keyword for emitting trace events.
    #[token("trace")]
    KwTrace,

    // =========================================================================
    // Supervision tree keywords (v2)
    // =========================================================================
    /// Supervisor declaration keyword.
    #[token("supervisor")]
    KwSupervisor,

    /// Children block in supervisor.
    #[token("children")]
    KwChildren,

    /// Supervision strategy keyword.
    #[token("strategy")]
    KwStrategy,

    /// Restart policy keyword.
    #[token("restart")]
    KwRestart,

    // =========================================================================
    // Type keywords
    // =========================================================================
    #[token("Int")]
    TyInt,

    #[token("Float")]
    TyFloat,

    #[token("Bool")]
    TyBool,

    #[token("String")]
    TyString,

    #[token("Unit")]
    TyUnit,

    #[token("List")]
    TyList,

    #[token("Option")]
    TyOption,

    #[token("Oracle")]
    TyOracle,

    #[token("Agent")]
    TyAgent,

    #[token("Error")]
    TyError,

    #[token("ErrorKind")]
    TyErrorKind,

    /// Function type keyword: `Fn`
    #[token("Fn")]
    TyFn,

    /// Map type keyword: `Map`
    #[token("Map")]
    TyMap,

    /// Result type keyword: `Result`
    #[token("Result")]
    TyResult,

    // =========================================================================
    // Literals
    // =========================================================================
    /// Integer literal (e.g., `42`, `-7`).
    #[regex(r"-?[0-9]+", priority = 2)]
    IntLit,

    /// Float literal (e.g., `3.14`, `-0.5`).
    #[regex(r"-?[0-9]+\.[0-9]+")]
    FloatLit,

    /// String literal (e.g., `"hello"` or `'hello'`).
    /// Supports escape sequences: \n, \t, \r, \\, \", \'
    /// Both double and single quotes are allowed. Use single quotes inside
    /// interpolations when needing string literals: `"Result: {len('hello')}"`
    #[regex(r#""([^"\\]|\\.)*""#)]
    #[regex(r#"'([^'\\]|\\.)*'"#)]
    StringLit,

    // =========================================================================
    // Identifiers
    // =========================================================================
    /// Identifier (e.g., `foo`, `myAgent`, `_private`).
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,

    // =========================================================================
    // Punctuation
    // =========================================================================
    #[token("{")]
    LBrace,

    #[token("}")]
    RBrace,

    #[token("(")]
    LParen,

    #[token(")")]
    RParen,

    #[token("[")]
    LBracket,

    #[token("]")]
    RBracket,

    #[token(",")]
    Comma,

    #[token("::")]
    ColonColon,

    #[token(":")]
    Colon,

    #[token(".")]
    Dot,

    #[token("->")]
    Arrow,

    #[token("=>")]
    FatArrow,

    /// Annotation marker for test attributes.
    #[token("@")]
    At,

    // =========================================================================
    // Operators
    // =========================================================================
    #[token("=")]
    Eq,

    #[token("==")]
    EqEq,

    #[token("!=")]
    Ne,

    #[token("<")]
    Lt,

    #[token(">")]
    Gt,

    #[token("<=")]
    Le,

    #[token(">=")]
    Ge,

    #[token("+")]
    Plus,

    #[token("-")]
    Minus,

    #[token("*")]
    Star,

    #[token("/")]
    Slash,

    #[token("!")]
    Bang,

    #[token("&&")]
    And,

    #[token("||")]
    Or,

    /// Single pipe for closure parameters: `|`
    #[token("|")]
    Pipe,

    /// String concatenation operator.
    #[token("++")]
    PlusPlus,

    /// Modulo/remainder operator.
    #[token("%")]
    Percent,

    /// Statement terminator.
    #[token(";")]
    Semicolon,
}

impl Token {
    /// Returns true if this token is a keyword.
    #[must_use]
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            Token::KwAgent
                | Token::KwBelief
                | Token::KwOn
                | Token::KwStart
                | Token::KwStop
                | Token::KwWaking
                | Token::KwPause
                | Token::KwResume
                | Token::KwResting
                | Token::KwMessage
                | Token::KwDivine
                | Token::KwSummon
                | Token::KwAwait
                | Token::KwSend
                | Token::KwYield
                | Token::KwRun
                | Token::KwFn
                | Token::KwLet
                | Token::KwReturn
                | Token::KwIf
                | Token::KwElse
                | Token::KwFor
                | Token::KwWhile
                | Token::KwLoop
                | Token::KwBreak
                | Token::KwIn
                | Token::KwSelf
                | Token::KwTrue
                | Token::KwFalse
                | Token::KwMod
                | Token::KwUse
                | Token::KwPub
                | Token::KwAs
                | Token::KwSuper
                | Token::KwRecord
                | Token::KwEnum
                | Token::KwMatch
                | Token::KwConst
                | Token::KwReceives
                | Token::KwReceive
                | Token::KwFail
                | Token::KwFails
                | Token::KwTimeout
                | Token::KwRetry
                | Token::KwDelay
                | Token::KwTry
                | Token::KwCatch
                | Token::KwError
                | Token::KwTool
                | Token::KwTrace
                | Token::KwSupervisor
                | Token::KwChildren
                | Token::KwStrategy
                | Token::KwRestart
        )
    }

    /// Returns true if this token is a type keyword.
    #[must_use]
    pub fn is_type_keyword(&self) -> bool {
        matches!(
            self,
            Token::TyInt
                | Token::TyFloat
                | Token::TyBool
                | Token::TyString
                | Token::TyUnit
                | Token::TyList
                | Token::TyOption
                | Token::TyOracle
                | Token::TyAgent
                | Token::TyError
                | Token::TyErrorKind
                | Token::TyFn
                | Token::TyMap
                | Token::TyResult
        )
    }

    /// Returns true if this token is a literal.
    #[must_use]
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            Token::IntLit | Token::FloatLit | Token::StringLit | Token::KwTrue | Token::KwFalse
        )
    }

    /// Returns true if this token is an operator.
    #[must_use]
    pub fn is_operator(&self) -> bool {
        matches!(
            self,
            Token::Eq
                | Token::EqEq
                | Token::Ne
                | Token::Lt
                | Token::Gt
                | Token::Le
                | Token::Ge
                | Token::Plus
                | Token::Minus
                | Token::Star
                | Token::Slash
                | Token::Percent
                | Token::Bang
                | Token::And
                | Token::Or
                | Token::PlusPlus
        )
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Keywords
            Token::KwAgent => write!(f, "agent"),
            Token::KwBelief => write!(f, "belief"),
            Token::KwOn => write!(f, "on"),
            Token::KwStart => write!(f, "start"),
            Token::KwStop => write!(f, "stop"),
            Token::KwWaking => write!(f, "waking"),
            Token::KwPause => write!(f, "pause"),
            Token::KwResume => write!(f, "resume"),
            Token::KwResting => write!(f, "resting"),
            Token::KwMessage => write!(f, "message"),
            Token::KwDivine => write!(f, "divine"),
            Token::KwSummon => write!(f, "summon"),
            Token::KwAwait => write!(f, "await"),
            Token::KwSend => write!(f, "send"),
            Token::KwYield => write!(f, "yield"),
            Token::KwRun => write!(f, "run"),
            Token::KwFn => write!(f, "fn"),
            Token::KwLet => write!(f, "let"),
            Token::KwReturn => write!(f, "return"),
            Token::KwIf => write!(f, "if"),
            Token::KwElse => write!(f, "else"),
            Token::KwFor => write!(f, "for"),
            Token::KwWhile => write!(f, "while"),
            Token::KwLoop => write!(f, "loop"),
            Token::KwBreak => write!(f, "break"),
            Token::KwIn => write!(f, "in"),
            Token::KwSelf => write!(f, "self"),
            Token::KwTrue => write!(f, "true"),
            Token::KwFalse => write!(f, "false"),
            Token::KwMod => write!(f, "mod"),
            Token::KwUse => write!(f, "use"),
            Token::KwPub => write!(f, "pub"),
            Token::KwAs => write!(f, "as"),
            Token::KwSuper => write!(f, "super"),
            Token::KwRecord => write!(f, "record"),
            Token::KwEnum => write!(f, "enum"),
            Token::KwMatch => write!(f, "match"),
            Token::KwConst => write!(f, "const"),
            Token::KwReceives => write!(f, "receives"),
            Token::KwReceive => write!(f, "receive"),
            Token::KwFail => write!(f, "fail"),
            Token::KwFails => write!(f, "fails"),
            Token::KwTimeout => write!(f, "timeout"),
            Token::KwRetry => write!(f, "retry"),
            Token::KwDelay => write!(f, "delay"),
            Token::KwTry => write!(f, "try"),
            Token::KwCatch => write!(f, "catch"),
            Token::KwError => write!(f, "error"),
            Token::KwTool => write!(f, "tool"),
            Token::KwTest => write!(f, "test"),
            Token::KwMock => write!(f, "mock"),
            Token::KwTrace => write!(f, "trace"),
            Token::KwSupervisor => write!(f, "supervisor"),
            Token::KwChildren => write!(f, "children"),
            Token::KwStrategy => write!(f, "strategy"),
            Token::KwRestart => write!(f, "restart"),

            // Type keywords
            Token::TyInt => write!(f, "Int"),
            Token::TyFloat => write!(f, "Float"),
            Token::TyBool => write!(f, "Bool"),
            Token::TyString => write!(f, "String"),
            Token::TyUnit => write!(f, "Unit"),
            Token::TyList => write!(f, "List"),
            Token::TyOption => write!(f, "Option"),
            Token::TyOracle => write!(f, "Oracle"),
            Token::TyAgent => write!(f, "Agent"),
            Token::TyError => write!(f, "Error"),
            Token::TyErrorKind => write!(f, "ErrorKind"),
            Token::TyFn => write!(f, "Fn"),
            Token::TyMap => write!(f, "Map"),
            Token::TyResult => write!(f, "Result"),

            // Literals
            Token::IntLit => write!(f, "<int>"),
            Token::FloatLit => write!(f, "<float>"),
            Token::StringLit => write!(f, "<string>"),

            // Identifier
            Token::Ident => write!(f, "<ident>"),

            // Punctuation
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::Comma => write!(f, ","),
            Token::ColonColon => write!(f, "::"),
            Token::Colon => write!(f, ":"),
            Token::Dot => write!(f, "."),
            Token::Arrow => write!(f, "->"),
            Token::FatArrow => write!(f, "=>"),
            Token::At => write!(f, "@"),

            // Operators
            Token::Eq => write!(f, "="),
            Token::EqEq => write!(f, "=="),
            Token::Ne => write!(f, "!="),
            Token::Lt => write!(f, "<"),
            Token::Gt => write!(f, ">"),
            Token::Le => write!(f, "<="),
            Token::Ge => write!(f, ">="),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Bang => write!(f, "!"),
            Token::And => write!(f, "&&"),
            Token::Or => write!(f, "||"),
            Token::Pipe => write!(f, "|"),
            Token::PlusPlus => write!(f, "++"),
            Token::Percent => write!(f, "%"),
            Token::Semicolon => write!(f, ";"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_keywords() {
        let mut lexer = Token::lexer("agent belief on start stop message");
        assert_eq!(lexer.next(), Some(Ok(Token::KwAgent)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwBelief)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwOn)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwStart)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwStop)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwMessage)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_more_keywords() {
        let mut lexer = Token::lexer(
            "divine summon await send yield run fn let return if else for in self true false",
        );
        assert_eq!(lexer.next(), Some(Ok(Token::KwDivine)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwSummon)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwAwait)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwSend)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwYield)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwRun)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwFn)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwLet)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwReturn)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwIf)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwElse)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwFor)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwIn)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwSelf)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwTrue)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwFalse)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_type_keywords() {
        let mut lexer = Token::lexer("Int Float Bool String Unit List Option Oracle Agent");
        assert_eq!(lexer.next(), Some(Ok(Token::TyInt)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyFloat)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyBool)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyString)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyUnit)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyList)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyOption)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyOracle)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyAgent)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_integer_literals() {
        let mut lexer = Token::lexer("42 -7 0 123456");
        assert_eq!(lexer.next(), Some(Ok(Token::IntLit)));
        assert_eq!(lexer.slice(), "42");
        assert_eq!(lexer.next(), Some(Ok(Token::IntLit)));
        assert_eq!(lexer.slice(), "-7");
        assert_eq!(lexer.next(), Some(Ok(Token::IntLit)));
        assert_eq!(lexer.slice(), "0");
        assert_eq!(lexer.next(), Some(Ok(Token::IntLit)));
        assert_eq!(lexer.slice(), "123456");
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_float_literals() {
        let mut lexer = Token::lexer("3.14 -0.5 0.0 123.456");
        assert_eq!(lexer.next(), Some(Ok(Token::FloatLit)));
        assert_eq!(lexer.slice(), "3.14");
        assert_eq!(lexer.next(), Some(Ok(Token::FloatLit)));
        assert_eq!(lexer.slice(), "-0.5");
        assert_eq!(lexer.next(), Some(Ok(Token::FloatLit)));
        assert_eq!(lexer.slice(), "0.0");
        assert_eq!(lexer.next(), Some(Ok(Token::FloatLit)));
        assert_eq!(lexer.slice(), "123.456");
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_string_literals() {
        let mut lexer = Token::lexer(r#""hello" "world" "with spaces""#);
        assert_eq!(lexer.next(), Some(Ok(Token::StringLit)));
        assert_eq!(lexer.slice(), r#""hello""#);
        assert_eq!(lexer.next(), Some(Ok(Token::StringLit)));
        assert_eq!(lexer.slice(), r#""world""#);
        assert_eq!(lexer.next(), Some(Ok(Token::StringLit)));
        assert_eq!(lexer.slice(), r#""with spaces""#);
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_string_with_escapes() {
        let mut lexer = Token::lexer(r#""hello\nworld" "tab\there" "quote\"here""#);
        assert_eq!(lexer.next(), Some(Ok(Token::StringLit)));
        assert_eq!(lexer.slice(), r#""hello\nworld""#);
        assert_eq!(lexer.next(), Some(Ok(Token::StringLit)));
        assert_eq!(lexer.slice(), r#""tab\there""#);
        assert_eq!(lexer.next(), Some(Ok(Token::StringLit)));
        assert_eq!(lexer.slice(), r#""quote\"here""#);
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_identifiers() {
        let mut lexer = Token::lexer("foo bar _private myAgent agent2");
        assert_eq!(lexer.next(), Some(Ok(Token::Ident)));
        assert_eq!(lexer.slice(), "foo");
        assert_eq!(lexer.next(), Some(Ok(Token::Ident)));
        assert_eq!(lexer.slice(), "bar");
        assert_eq!(lexer.next(), Some(Ok(Token::Ident)));
        assert_eq!(lexer.slice(), "_private");
        assert_eq!(lexer.next(), Some(Ok(Token::Ident)));
        assert_eq!(lexer.slice(), "myAgent");
        assert_eq!(lexer.next(), Some(Ok(Token::Ident)));
        assert_eq!(lexer.slice(), "agent2");
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn keyword_vs_identifier() {
        // "agent" is a keyword, "agent_name" is an identifier
        let mut lexer = Token::lexer("agent agent_name agents");
        assert_eq!(lexer.next(), Some(Ok(Token::KwAgent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident)));
        assert_eq!(lexer.slice(), "agent_name");
        assert_eq!(lexer.next(), Some(Ok(Token::Ident)));
        assert_eq!(lexer.slice(), "agents");
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_punctuation() {
        let mut lexer = Token::lexer("{ } ( ) [ ] , : . ->");
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::LBracket)));
        assert_eq!(lexer.next(), Some(Ok(Token::RBracket)));
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Dot)));
        assert_eq!(lexer.next(), Some(Ok(Token::Arrow)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_operators() {
        let mut lexer = Token::lexer("= == != < > <= >= + - * / % ! && || ++");
        assert_eq!(lexer.next(), Some(Ok(Token::Eq)));
        assert_eq!(lexer.next(), Some(Ok(Token::EqEq)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ne)));
        assert_eq!(lexer.next(), Some(Ok(Token::Lt)));
        assert_eq!(lexer.next(), Some(Ok(Token::Gt)));
        assert_eq!(lexer.next(), Some(Ok(Token::Le)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ge)));
        assert_eq!(lexer.next(), Some(Ok(Token::Plus)));
        assert_eq!(lexer.next(), Some(Ok(Token::Minus)));
        assert_eq!(lexer.next(), Some(Ok(Token::Star)));
        assert_eq!(lexer.next(), Some(Ok(Token::Slash)));
        assert_eq!(lexer.next(), Some(Ok(Token::Percent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Bang)));
        assert_eq!(lexer.next(), Some(Ok(Token::And)));
        assert_eq!(lexer.next(), Some(Ok(Token::Or)));
        assert_eq!(lexer.next(), Some(Ok(Token::PlusPlus)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn skip_whitespace() {
        let mut lexer = Token::lexer("  agent   belief\n\ttrue  ");
        assert_eq!(lexer.next(), Some(Ok(Token::KwAgent)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwBelief)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwTrue)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn skip_comments() {
        let mut lexer = Token::lexer("agent // this is a comment\nbelief");
        assert_eq!(lexer.next(), Some(Ok(Token::KwAgent)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwBelief)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn comment_at_end() {
        let mut lexer = Token::lexer("agent // comment at end");
        assert_eq!(lexer.next(), Some(Ok(Token::KwAgent)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_agent_declaration() {
        let source = r#"
            agent Researcher {
                belief topic: String

                on start {
                    let result: Oracle<String> = divine("test")
                    yield(result)
                }
            }
        "#;
        let tokens: Vec<_> = Token::lexer(source)
            .map(|r| r.expect("valid token"))
            .collect();

        assert_eq!(tokens[0], Token::KwAgent);
        assert_eq!(tokens[1], Token::Ident); // Researcher
        assert_eq!(tokens[2], Token::LBrace);
        assert_eq!(tokens[3], Token::KwBelief);
        assert_eq!(tokens[4], Token::Ident); // topic
        assert_eq!(tokens[5], Token::Colon);
        assert_eq!(tokens[6], Token::TyString);
        assert_eq!(tokens[7], Token::KwOn);
        assert_eq!(tokens[8], Token::KwStart);
        assert_eq!(tokens[9], Token::LBrace);
        assert_eq!(tokens[10], Token::KwLet);
    }

    #[test]
    fn is_keyword_helper() {
        assert!(Token::KwAgent.is_keyword());
        assert!(Token::KwLet.is_keyword());
        assert!(!Token::TyInt.is_keyword());
        assert!(!Token::Ident.is_keyword());
    }

    #[test]
    fn is_type_keyword_helper() {
        assert!(Token::TyInt.is_type_keyword());
        assert!(Token::TyAgent.is_type_keyword());
        assert!(!Token::KwAgent.is_type_keyword());
        assert!(!Token::Ident.is_type_keyword());
    }

    #[test]
    fn is_literal_helper() {
        assert!(Token::IntLit.is_literal());
        assert!(Token::FloatLit.is_literal());
        assert!(Token::StringLit.is_literal());
        assert!(Token::KwTrue.is_literal());
        assert!(Token::KwFalse.is_literal());
        assert!(!Token::Ident.is_literal());
    }

    #[test]
    fn is_operator_helper() {
        assert!(Token::Plus.is_operator());
        assert!(Token::EqEq.is_operator());
        assert!(Token::PlusPlus.is_operator());
        assert!(!Token::LBrace.is_operator());
        assert!(!Token::Ident.is_operator());
    }

    #[test]
    fn lex_module_keywords() {
        let mut lexer = Token::lexer("mod use pub as super");
        assert_eq!(lexer.next(), Some(Ok(Token::KwMod)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwUse)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwPub)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwAs)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwSuper)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_path_separator() {
        let mut lexer = Token::lexer("agents::Researcher");
        assert_eq!(lexer.next(), Some(Ok(Token::Ident)));
        assert_eq!(lexer.slice(), "agents");
        assert_eq!(lexer.next(), Some(Ok(Token::ColonColon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident)));
        assert_eq!(lexer.slice(), "Researcher");
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_use_statement() {
        let mut lexer = Token::lexer("use agents::{Researcher, Coordinator as Coord}");
        assert_eq!(lexer.next(), Some(Ok(Token::KwUse)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // agents
        assert_eq!(lexer.next(), Some(Ok(Token::ColonColon)));
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Researcher
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Coordinator
        assert_eq!(lexer.next(), Some(Ok(Token::KwAs)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Coord
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_pub_agent() {
        let mut lexer = Token::lexer("pub agent Researcher");
        assert_eq!(lexer.next(), Some(Ok(Token::KwPub)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwAgent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn token_display() {
        assert_eq!(format!("{}", Token::KwAgent), "agent");
        assert_eq!(format!("{}", Token::TyInt), "Int");
        assert_eq!(format!("{}", Token::IntLit), "<int>");
        assert_eq!(format!("{}", Token::Ident), "<ident>");
        assert_eq!(format!("{}", Token::LBrace), "{");
        assert_eq!(format!("{}", Token::PlusPlus), "++");
    }

    #[test]
    fn lex_type_keywords_record_enum_match_const() {
        let mut lexer = Token::lexer("record enum match const");
        assert_eq!(lexer.next(), Some(Ok(Token::KwRecord)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwEnum)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwMatch)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwConst)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_fat_arrow() {
        let mut lexer = Token::lexer("=> -> =");
        assert_eq!(lexer.next(), Some(Ok(Token::FatArrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::Arrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::Eq)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_match_expression() {
        let mut lexer = Token::lexer("match status { Active => 1, Inactive => 0 }");
        assert_eq!(lexer.next(), Some(Ok(Token::KwMatch)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // status
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Active
        assert_eq!(lexer.next(), Some(Ok(Token::FatArrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::IntLit))); // 1
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Inactive
        assert_eq!(lexer.next(), Some(Ok(Token::FatArrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::IntLit))); // 0
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_record_declaration() {
        let mut lexer = Token::lexer("record Point { x: Int, y: Int }");
        assert_eq!(lexer.next(), Some(Ok(Token::KwRecord)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Point
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // x
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyInt)));
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // y
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyInt)));
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_enum_declaration() {
        let mut lexer = Token::lexer("enum Status { Active, Pending, Done }");
        assert_eq!(lexer.next(), Some(Ok(Token::KwEnum)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Status
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Active
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Pending
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Done
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_const_declaration() {
        let mut lexer = Token::lexer("const MAX_RETRIES: Int = 3");
        assert_eq!(lexer.next(), Some(Ok(Token::KwConst)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // MAX_RETRIES
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyInt)));
        assert_eq!(lexer.next(), Some(Ok(Token::Eq)));
        assert_eq!(lexer.next(), Some(Ok(Token::IntLit))); // 3
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn new_keywords_are_keywords() {
        assert!(Token::KwRecord.is_keyword());
        assert!(Token::KwEnum.is_keyword());
        assert!(Token::KwMatch.is_keyword());
        assert!(Token::KwConst.is_keyword());
    }

    #[test]
    fn lex_loop_break() {
        let mut lexer = Token::lexer("loop { break }");
        assert_eq!(lexer.next(), Some(Ok(Token::KwLoop)));
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwBreak)));
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_receives_receive() {
        let mut lexer = Token::lexer("agent Worker receives WorkerMsg { receive }");
        assert_eq!(lexer.next(), Some(Ok(Token::KwAgent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Worker
        assert_eq!(lexer.next(), Some(Ok(Token::KwReceives)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // WorkerMsg
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwReceive)));
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn rfc6_keywords_are_keywords() {
        assert!(Token::KwLoop.is_keyword());
        assert!(Token::KwBreak.is_keyword());
        assert!(Token::KwReceives.is_keyword());
        assert!(Token::KwReceive.is_keyword());
    }

    #[test]
    fn lex_error_handling_keywords() {
        let mut lexer = Token::lexer("fails try catch error");
        assert_eq!(lexer.next(), Some(Ok(Token::KwFails)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwTry)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwCatch)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwError)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_try_catch_expression() {
        let mut lexer = Token::lexer("let x = try divine(prompt) catch { fallback }");
        assert_eq!(lexer.next(), Some(Ok(Token::KwLet)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // x
        assert_eq!(lexer.next(), Some(Ok(Token::Eq)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwTry)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwDivine)));
        assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // prompt
        assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwCatch)));
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // fallback
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_fails_function() {
        let mut lexer = Token::lexer("fn fetch(url: String) -> String fails { }");
        assert_eq!(lexer.next(), Some(Ok(Token::KwFn)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // fetch
        assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // url
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyString)));
        assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Arrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyString)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwFails)));
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_on_error_handler() {
        let mut lexer = Token::lexer("on error(e) { yield(fallback) }");
        assert_eq!(lexer.next(), Some(Ok(Token::KwOn)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwError)));
        assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // e
        assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwYield)));
        assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // fallback
        assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn rfc7_keywords_are_keywords() {
        assert!(Token::KwFail.is_keyword());
        assert!(Token::KwFails.is_keyword());
        assert!(Token::KwTry.is_keyword());
        assert!(Token::KwCatch.is_keyword());
        assert!(Token::KwError.is_keyword());
    }

    #[test]
    fn lex_fail_expression() {
        let mut lexer = Token::lexer("fail \"error message\"");
        assert_eq!(lexer.next(), Some(Ok(Token::KwFail)));
        assert_eq!(lexer.next(), Some(Ok(Token::StringLit)));
        assert_eq!(lexer.next(), None);
    }

    // =========================================================================
    // RFC-0009: Closures
    // =========================================================================

    #[test]
    fn lex_closure_syntax() {
        // |x: Int| x + 1
        let mut lexer = Token::lexer("|x: Int| x + 1");
        assert_eq!(lexer.next(), Some(Ok(Token::Pipe)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // x
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyInt)));
        assert_eq!(lexer.next(), Some(Ok(Token::Pipe)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // x
        assert_eq!(lexer.next(), Some(Ok(Token::Plus)));
        assert_eq!(lexer.next(), Some(Ok(Token::IntLit)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_empty_closure() {
        // || 42
        let mut lexer = Token::lexer("|| 42");
        assert_eq!(lexer.next(), Some(Ok(Token::Or))); // || lexes as Or
        assert_eq!(lexer.next(), Some(Ok(Token::IntLit)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn lex_fn_type() {
        // Fn(Int, String) -> Bool
        let mut lexer = Token::lexer("Fn(Int, String) -> Bool");
        assert_eq!(lexer.next(), Some(Ok(Token::TyFn)));
        assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyInt)));
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyString)));
        assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Arrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyBool)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn fn_is_type_keyword() {
        assert!(Token::TyFn.is_type_keyword());
    }

    #[test]
    fn pipe_display() {
        assert_eq!(format!("{}", Token::Pipe), "|");
        assert_eq!(format!("{}", Token::TyFn), "Fn");
    }

    // =========================================================================
    // RFC-0011: Tool Support
    // =========================================================================

    #[test]
    fn lex_tool_keyword() {
        let mut lexer = Token::lexer("tool Http { fn get(url: String) -> String }");
        assert_eq!(lexer.next(), Some(Ok(Token::KwTool)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Http
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwFn)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // get
        assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // url
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyString)));
        assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Arrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyString)));
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn tool_is_keyword() {
        assert!(Token::KwTool.is_keyword());
    }

    #[test]
    fn lex_agent_use_tool() {
        let mut lexer = Token::lexer("agent Fetcher { use Http }");
        assert_eq!(lexer.next(), Some(Ok(Token::KwAgent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Fetcher
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwUse)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ident))); // Http
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);
    }
}
