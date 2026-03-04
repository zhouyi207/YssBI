//! 词法分析器：将自由文本解析为 Token 流

use std::iter::Peekable;
use std::str::Chars;

/// 词法单元
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// 标识符（参数名、函数名）
    Ident(String),
    /// 浮点数
    Number(f64),
    /// +
    Plus,
    /// -
    Minus,
    /// *
    Star,
    /// /
    Slash,
    /// (
    LParen,
    /// )
    RParen,
    /// =
    Eq,
    /// <
    Lt,
    /// >
    Gt,
    /// <=
    Le,
    /// >=
    Ge,
    /// < (严格，用于区分解析)
    LtStrict,
    /// > (严格)
    GtStrict,
    /// != (预留)
    Ne,
}

/// 词法分析器
pub struct Lexer<'a> {
    input: &'a str,
    chars: Peekable<Chars<'a>>,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Lexer {
            input,
            chars: input.chars().peekable(),
            pos: 0,
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.chars.next();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_whitespace(&mut self) {
        while self.peek().map_or(false, |c| c.is_whitespace()) {
            self.next_char();
        }
    }

    fn read_ident(&mut self, first: char) -> String {
        let mut s = String::from(first);
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.next_char();
            } else {
                break;
            }
        }
        s
    }

    fn read_number(&mut self, first: char) -> Result<f64, LexError> {
        let mut s = String::from(first);
        let mut has_dot = first == '.';

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.next_char();
            } else if c == '.' && !has_dot {
                has_dot = true;
                s.push(c);
                self.next_char();
            } else if (c == 'e' || c == 'E') && !s.contains(['e', 'E']) {
                s.push(c);
                self.next_char();
                if let Some(sign) = self.peek() {
                    if sign == '+' || sign == '-' {
                        s.push(sign);
                        self.next_char();
                    }
                }
            } else {
                break;
            }
        }

        let s_normalized = if s.starts_with('.') || s.starts_with("-.") {
            "0".to_string() + &s
        } else {
            s.clone()
        };
        s_normalized.parse::<f64>().map_err(|_| LexError::InvalidNumber(s))
    }

    /// 获取下一个 Token
    pub fn next_token(&mut self) -> Result<Option<Token>, LexError> {
        self.skip_whitespace();

        let c = match self.next_char() {
            Some(ch) => ch,
            None => return Ok(None),
        };

        let token = match c {
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '=' => {
                if self.peek() == Some('=') {
                    self.next_char();
                    Token::Eq
                } else {
                    Token::Eq
                }
            }
            '<' => {
                if self.peek() == Some('=') {
                    self.next_char();
                    Token::Le
                } else if self.peek() == Some('>') {
                    self.next_char();
                    Token::Ne
                } else {
                    Token::LtStrict
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.next_char();
                    Token::Ge
                } else {
                    Token::GtStrict
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.next_char();
                    Token::Ne
                } else {
                    return Err(LexError::UnexpectedChar(c));
                }
            }
            '0'..='9' | '.' => {
                let num = self.read_number(c)?;
                return Ok(Some(Token::Number(num)));
            }
            c if c.is_alphabetic() || c == '_' => {
                let ident = self.read_ident(c);
                return Ok(Some(Token::Ident(ident)));
            }
            _ => return Err(LexError::UnexpectedChar(c)),
        };

        Ok(Some(token))
    }

    /// 消费所有 token，返回 token 序列（用于调试或一次性解析）
    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        while let Some(t) = self.next_token()? {
            tokens.push(t);
        }
        Ok(tokens)
    }
}

/// 词法错误
#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    UnexpectedChar(char),
    InvalidNumber(String),
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LexError::UnexpectedChar(c) => write!(f, "unexpected character: {}", c),
            LexError::InvalidNumber(s) => write!(f, "invalid number: {}", s),
        }
    }
}

impl std::error::Error for LexError {}
