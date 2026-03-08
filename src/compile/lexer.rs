use super::CompileError;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    // Literals
    IntLiteral(i64),
    LongLiteral(i64),
    FloatLiteral(f64),
    DoubleLiteral(f64),
    StringLiteral(String),
    CharLiteral(char),

    // Identifiers and keywords
    Ident(String),
    If,
    Else,
    While,
    For,
    Return,
    New,
    This,
    Throw,
    Break,
    Continue,
    Instanceof,
    Switch,
    Case,
    Default,
    Try,
    Catch,
    Finally,
    Null,
    True,
    False,
    Synchronized,
    Var,

    // Primitive type keywords
    KwInt,
    KwLong,
    KwFloat,
    KwDouble,
    KwBoolean,
    KwByte,
    KwChar,
    KwShort,
    KwVoid,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Amp,
    Pipe,
    Caret,
    Tilde,
    Bang,
    AmpAmp,
    PipePipe,
    Eq,
    EqEq,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    LtLt,
    GtGt,
    GtGtGt,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    AmpEq,
    PipeEq,
    CaretEq,
    LtLtEq,
    GtGtEq,
    GtGtGtEq,
    PlusPlus,
    MinusMinus,

    Arrow,
    ColonColon,

    // Delimiters
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Semicolon,
    Comma,
    Dot,
    Question,
    Colon,

    // End of input
    Eof,
}

#[derive(Clone, Debug)]
pub struct SpannedToken {
    pub token: Token,
    pub line: usize,
    pub column: usize,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<SpannedToken>, CompileError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.chars.len() {
                tokens.push(SpannedToken {
                    token: Token::Eof,
                    line: self.line,
                    column: self.column,
                });
                break;
            }
            tokens.push(self.next_token()?);
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_ahead(&self, n: usize) -> Option<char> {
        self.chars.get(self.pos + n).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.peek().is_some_and(|c| c.is_whitespace()) {
                self.advance();
            }
            // Skip line comments
            if self.peek() == Some('/') && self.peek_ahead(1) == Some('/') {
                while self.peek().is_some_and(|c| c != '\n') {
                    self.advance();
                }
                continue;
            }
            // Skip block comments
            if self.peek() == Some('/') && self.peek_ahead(1) == Some('*') {
                self.advance();
                self.advance();
                loop {
                    if self.peek().is_none() {
                        break;
                    }
                    if self.peek() == Some('*') && self.peek_ahead(1) == Some('/') {
                        self.advance();
                        self.advance();
                        break;
                    }
                    self.advance();
                }
                continue;
            }
            break;
        }
    }

    fn error(&self, message: impl Into<String>) -> CompileError {
        CompileError::ParseError {
            line: self.line,
            column: self.column,
            message: message.into(),
        }
    }

    fn next_token(&mut self) -> Result<SpannedToken, CompileError> {
        let line = self.line;
        let column = self.column;
        let c = self.peek().unwrap();

        let token = match c {
            '(' => {
                self.advance();
                Token::LParen
            }
            ')' => {
                self.advance();
                Token::RParen
            }
            '{' => {
                self.advance();
                Token::LBrace
            }
            '}' => {
                self.advance();
                Token::RBrace
            }
            '[' => {
                self.advance();
                Token::LBracket
            }
            ']' => {
                self.advance();
                Token::RBracket
            }
            ';' => {
                self.advance();
                Token::Semicolon
            }
            ',' => {
                self.advance();
                Token::Comma
            }
            '.' => {
                self.advance();
                Token::Dot
            }
            '?' => {
                self.advance();
                Token::Question
            }
            ':' => {
                self.advance();
                if self.peek() == Some(':') {
                    self.advance();
                    Token::ColonColon
                } else {
                    Token::Colon
                }
            }
            '~' => {
                self.advance();
                Token::Tilde
            }
            '+' => {
                self.advance();
                if self.peek() == Some('+') {
                    self.advance();
                    Token::PlusPlus
                } else if self.peek() == Some('=') {
                    self.advance();
                    Token::PlusEq
                } else {
                    Token::Plus
                }
            }
            '-' => {
                self.advance();
                if self.peek() == Some('-') {
                    self.advance();
                    Token::MinusMinus
                } else if self.peek() == Some('=') {
                    self.advance();
                    Token::MinusEq
                } else if self.peek() == Some('>') {
                    self.advance();
                    Token::Arrow
                } else {
                    Token::Minus
                }
            }
            '*' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Token::StarEq
                } else {
                    Token::Star
                }
            }
            '/' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Token::SlashEq
                } else {
                    Token::Slash
                }
            }
            '%' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Token::PercentEq
                } else {
                    Token::Percent
                }
            }
            '&' => {
                self.advance();
                if self.peek() == Some('&') {
                    self.advance();
                    Token::AmpAmp
                } else if self.peek() == Some('=') {
                    self.advance();
                    Token::AmpEq
                } else {
                    Token::Amp
                }
            }
            '|' => {
                self.advance();
                if self.peek() == Some('|') {
                    self.advance();
                    Token::PipePipe
                } else if self.peek() == Some('=') {
                    self.advance();
                    Token::PipeEq
                } else {
                    Token::Pipe
                }
            }
            '^' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Token::CaretEq
                } else {
                    Token::Caret
                }
            }
            '!' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Token::BangEq
                } else {
                    Token::Bang
                }
            }
            '=' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Token::EqEq
                } else {
                    Token::Eq
                }
            }
            '<' => {
                self.advance();
                if self.peek() == Some('<') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        Token::LtLtEq
                    } else {
                        Token::LtLt
                    }
                } else if self.peek() == Some('=') {
                    self.advance();
                    Token::LtEq
                } else {
                    Token::Lt
                }
            }
            '>' => {
                self.advance();
                if self.peek() == Some('>') {
                    self.advance();
                    if self.peek() == Some('>') {
                        self.advance();
                        if self.peek() == Some('=') {
                            self.advance();
                            Token::GtGtGtEq
                        } else {
                            Token::GtGtGt
                        }
                    } else if self.peek() == Some('=') {
                        self.advance();
                        Token::GtGtEq
                    } else {
                        Token::GtGt
                    }
                } else if self.peek() == Some('=') {
                    self.advance();
                    Token::GtEq
                } else {
                    Token::Gt
                }
            }
            '"' => self.read_string()?,
            '\'' => self.read_char()?,
            _ if c.is_ascii_digit() => self.read_number()?,
            _ if c.is_ascii_alphabetic() || c == '_' => self.read_ident_or_keyword(),
            _ => return Err(self.error(format!("unexpected character: '{}'", c))),
        };

        Ok(SpannedToken {
            token,
            line,
            column,
        })
    }

    fn read_string(&mut self) -> Result<Token, CompileError> {
        self.advance(); // consume opening "
        let mut s = String::new();
        loop {
            match self.peek() {
                None => return Err(self.error("unterminated string literal")),
                Some('"') => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    self.advance();
                    match self.peek() {
                        Some('n') => {
                            self.advance();
                            s.push('\n');
                        }
                        Some('t') => {
                            self.advance();
                            s.push('\t');
                        }
                        Some('r') => {
                            self.advance();
                            s.push('\r');
                        }
                        Some('\\') => {
                            self.advance();
                            s.push('\\');
                        }
                        Some('"') => {
                            self.advance();
                            s.push('"');
                        }
                        Some('\'') => {
                            self.advance();
                            s.push('\'');
                        }
                        Some('0') => {
                            self.advance();
                            s.push('\0');
                        }
                        _ => return Err(self.error("invalid escape sequence")),
                    }
                }
                Some(c) => {
                    self.advance();
                    s.push(c);
                }
            }
        }
        Ok(Token::StringLiteral(s))
    }

    fn read_char(&mut self) -> Result<Token, CompileError> {
        self.advance(); // consume opening '
        let c = match self.peek() {
            None => return Err(self.error("unterminated char literal")),
            Some('\\') => {
                self.advance();
                match self.peek() {
                    Some('n') => {
                        self.advance();
                        '\n'
                    }
                    Some('t') => {
                        self.advance();
                        '\t'
                    }
                    Some('r') => {
                        self.advance();
                        '\r'
                    }
                    Some('\\') => {
                        self.advance();
                        '\\'
                    }
                    Some('\'') => {
                        self.advance();
                        '\''
                    }
                    Some('0') => {
                        self.advance();
                        '\0'
                    }
                    _ => return Err(self.error("invalid escape sequence in char literal")),
                }
            }
            Some(c) => {
                self.advance();
                c
            }
        };
        if self.peek() != Some('\'') {
            return Err(self.error("expected closing ' in char literal"));
        }
        self.advance();
        Ok(Token::CharLiteral(c))
    }

    fn read_number(&mut self) -> Result<Token, CompileError> {
        let mut num_str = String::new();
        let mut is_float = false;
        let mut is_long = false;
        let mut is_explicit_float = false;
        let mut is_explicit_double = false;

        // Handle hex
        if self.peek() == Some('0')
            && (self.peek_ahead(1) == Some('x') || self.peek_ahead(1) == Some('X'))
        {
            num_str.push(self.advance().unwrap()); // '0'
            num_str.push(self.advance().unwrap()); // 'x'
            while self
                .peek()
                .is_some_and(|c| c.is_ascii_hexdigit() || c == '_')
            {
                let c = self.advance().unwrap();
                if c != '_' {
                    num_str.push(c);
                }
            }
            if self.peek() == Some('L') || self.peek() == Some('l') {
                self.advance();
                let val = i64::from_str_radix(&num_str[2..], 16)
                    .map_err(|_| self.error("invalid hex long literal"))?;
                return Ok(Token::LongLiteral(val));
            }
            let val = i64::from_str_radix(&num_str[2..], 16)
                .map_err(|_| self.error("invalid hex literal"))?;
            return Ok(Token::IntLiteral(val));
        }

        // Regular decimal
        while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') {
            let c = self.advance().unwrap();
            if c != '_' {
                num_str.push(c);
            }
        }

        if self.peek() == Some('.') && self.peek_ahead(1).is_some_and(|c| c.is_ascii_digit()) {
            is_float = true;
            num_str.push(self.advance().unwrap()); // '.'
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') {
                let c = self.advance().unwrap();
                if c != '_' {
                    num_str.push(c);
                }
            }
        }

        // Exponent
        if self.peek() == Some('e') || self.peek() == Some('E') {
            is_float = true;
            num_str.push(self.advance().unwrap());
            if self.peek() == Some('+') || self.peek() == Some('-') {
                num_str.push(self.advance().unwrap());
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                num_str.push(self.advance().unwrap());
            }
        }

        // Suffix
        if self.peek() == Some('f') || self.peek() == Some('F') {
            self.advance();
            is_float = true;
            is_explicit_float = true;
        } else if self.peek() == Some('d') || self.peek() == Some('D') {
            self.advance();
            is_float = true;
            is_explicit_double = true;
        } else if self.peek() == Some('L') || self.peek() == Some('l') {
            self.advance();
            is_long = true;
        }

        if is_long {
            let val: i64 = num_str
                .parse()
                .map_err(|_| self.error("invalid long literal"))?;
            Ok(Token::LongLiteral(val))
        } else if is_float {
            let val: f64 = num_str
                .parse()
                .map_err(|_| self.error("invalid float literal"))?;
            if is_explicit_float {
                Ok(Token::FloatLiteral(val))
            } else if is_explicit_double {
                Ok(Token::DoubleLiteral(val))
            } else {
                // Unadorned float literal defaults to double in Java
                Ok(Token::DoubleLiteral(val))
            }
        } else {
            let val: i64 = num_str
                .parse()
                .map_err(|_| self.error("invalid integer literal"))?;
            Ok(Token::IntLiteral(val))
        }
    }

    fn read_ident_or_keyword(&mut self) -> Token {
        let mut ident = String::new();
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            ident.push(self.advance().unwrap());
        }
        match ident.as_str() {
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "for" => Token::For,
            "return" => Token::Return,
            "new" => Token::New,
            "this" => Token::This,
            "throw" => Token::Throw,
            "break" => Token::Break,
            "continue" => Token::Continue,
            "instanceof" => Token::Instanceof,
            "switch" => Token::Switch,
            "case" => Token::Case,
            "default" => Token::Default,
            "try" => Token::Try,
            "catch" => Token::Catch,
            "finally" => Token::Finally,
            "null" => Token::Null,
            "true" => Token::True,
            "false" => Token::False,
            "synchronized" => Token::Synchronized,
            "var" => Token::Var,
            "int" => Token::KwInt,
            "long" => Token::KwLong,
            "float" => Token::KwFloat,
            "double" => Token::KwDouble,
            "boolean" => Token::KwBoolean,
            "byte" => Token::KwByte,
            "char" => Token::KwChar,
            "short" => Token::KwShort,
            "void" => Token::KwVoid,
            _ => Token::Ident(ident),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(s: &str) -> Vec<Token> {
        Lexer::new(s)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|st| st.token)
            .collect()
    }

    #[test]
    fn test_simple_tokens() {
        let tokens = lex("int x = 42;");
        assert_eq!(
            tokens,
            vec![
                Token::KwInt,
                Token::Ident("x".into()),
                Token::Eq,
                Token::IntLiteral(42),
                Token::Semicolon,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_string_literal() {
        let tokens = lex("\"hello\\nworld\"");
        assert_eq!(
            tokens,
            vec![Token::StringLiteral("hello\nworld".into()), Token::Eof]
        );
    }

    #[test]
    fn test_operators() {
        let tokens = lex("a++ && b-- || !c");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("a".into()),
                Token::PlusPlus,
                Token::AmpAmp,
                Token::Ident("b".into()),
                Token::MinusMinus,
                Token::PipePipe,
                Token::Bang,
                Token::Ident("c".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_float_literals() {
        let tokens = lex("3.14f 2.0 1L");
        assert_eq!(
            tokens,
            vec![
                Token::FloatLiteral(3.14),
                Token::DoubleLiteral(2.0),
                Token::LongLiteral(1),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_comments() {
        let tokens = lex("a // comment\n+ b /* block */ + c");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("a".into()),
                Token::Plus,
                Token::Ident("b".into()),
                Token::Plus,
                Token::Ident("c".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_shift_operators() {
        let tokens = lex("a << b >> c >>> d");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("a".into()),
                Token::LtLt,
                Token::Ident("b".into()),
                Token::GtGt,
                Token::Ident("c".into()),
                Token::GtGtGt,
                Token::Ident("d".into()),
                Token::Eof,
            ]
        );
    }
}
