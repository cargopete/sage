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

    #[token("message")]
    KwMessage,

    #[token("infer")]
    KwInfer,

    #[token("spawn")]
    KwSpawn,

    #[token("await")]
    KwAwait,

    #[token("send")]
    KwSend,

    #[token("emit")]
    KwEmit,

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

    #[token("in")]
    KwIn,

    #[token("self")]
    KwSelf,

    #[token("true")]
    KwTrue,

    #[token("false")]
    KwFalse,

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

    #[token("Inferred")]
    TyInferred,

    #[token("Agent")]
    TyAgent,

    // =========================================================================
    // Literals
    // =========================================================================
    /// Integer literal (e.g., `42`, `-7`).
    #[regex(r"-?[0-9]+", priority = 2)]
    IntLit,

    /// Float literal (e.g., `3.14`, `-0.5`).
    #[regex(r"-?[0-9]+\.[0-9]+")]
    FloatLit,

    /// String literal (e.g., `"hello"`).
    /// Supports escape sequences: \n, \t, \r, \\, \"
    #[regex(r#""([^"\\]|\\.)*""#)]
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

    #[token(":")]
    Colon,

    #[token(".")]
    Dot,

    #[token("->")]
    Arrow,

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

    /// String concatenation operator.
    #[token("++")]
    PlusPlus,
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
                | Token::KwMessage
                | Token::KwInfer
                | Token::KwSpawn
                | Token::KwAwait
                | Token::KwSend
                | Token::KwEmit
                | Token::KwRun
                | Token::KwFn
                | Token::KwLet
                | Token::KwReturn
                | Token::KwIf
                | Token::KwElse
                | Token::KwFor
                | Token::KwIn
                | Token::KwSelf
                | Token::KwTrue
                | Token::KwFalse
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
                | Token::TyInferred
                | Token::TyAgent
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
            Token::KwMessage => write!(f, "message"),
            Token::KwInfer => write!(f, "infer"),
            Token::KwSpawn => write!(f, "spawn"),
            Token::KwAwait => write!(f, "await"),
            Token::KwSend => write!(f, "send"),
            Token::KwEmit => write!(f, "emit"),
            Token::KwRun => write!(f, "run"),
            Token::KwFn => write!(f, "fn"),
            Token::KwLet => write!(f, "let"),
            Token::KwReturn => write!(f, "return"),
            Token::KwIf => write!(f, "if"),
            Token::KwElse => write!(f, "else"),
            Token::KwFor => write!(f, "for"),
            Token::KwIn => write!(f, "in"),
            Token::KwSelf => write!(f, "self"),
            Token::KwTrue => write!(f, "true"),
            Token::KwFalse => write!(f, "false"),

            // Type keywords
            Token::TyInt => write!(f, "Int"),
            Token::TyFloat => write!(f, "Float"),
            Token::TyBool => write!(f, "Bool"),
            Token::TyString => write!(f, "String"),
            Token::TyUnit => write!(f, "Unit"),
            Token::TyList => write!(f, "List"),
            Token::TyOption => write!(f, "Option"),
            Token::TyInferred => write!(f, "Inferred"),
            Token::TyAgent => write!(f, "Agent"),

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
            Token::Colon => write!(f, ":"),
            Token::Dot => write!(f, "."),
            Token::Arrow => write!(f, "->"),

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
            Token::PlusPlus => write!(f, "++"),
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
            "infer spawn await send emit run fn let return if else for in self true false",
        );
        assert_eq!(lexer.next(), Some(Ok(Token::KwInfer)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwSpawn)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwAwait)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwSend)));
        assert_eq!(lexer.next(), Some(Ok(Token::KwEmit)));
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
        let mut lexer = Token::lexer("Int Float Bool String Unit List Option Inferred Agent");
        assert_eq!(lexer.next(), Some(Ok(Token::TyInt)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyFloat)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyBool)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyString)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyUnit)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyList)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyOption)));
        assert_eq!(lexer.next(), Some(Ok(Token::TyInferred)));
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
        let mut lexer = Token::lexer("= == != < > <= >= + - * / ! && || ++");
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
                    let result: Inferred<String> = infer("test")
                    emit(result)
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
    fn token_display() {
        assert_eq!(format!("{}", Token::KwAgent), "agent");
        assert_eq!(format!("{}", Token::TyInt), "Int");
        assert_eq!(format!("{}", Token::IntLit), "<int>");
        assert_eq!(format!("{}", Token::Ident), "<ident>");
        assert_eq!(format!("{}", Token::LBrace), "{");
        assert_eq!(format!("{}", Token::PlusPlus), "++");
    }
}
