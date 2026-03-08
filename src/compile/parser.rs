use super::CompileError;
use super::ast::*;
use super::lexer::{SpannedToken, Token};

pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos].token
    }

    fn at(&self, token: &Token) -> bool {
        self.peek() == token
    }

    fn advance(&mut self) -> &SpannedToken {
        let t = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, expected: &Token) -> Result<(), CompileError> {
        if self.peek() == expected {
            self.advance();
            Ok(())
        } else {
            Err(self.error(format!("expected {:?}, got {:?}", expected, self.peek())))
        }
    }

    fn error(&self, message: impl Into<String>) -> CompileError {
        let span = &self.tokens[self.pos];
        CompileError::ParseError {
            line: span.line,
            column: span.column,
            message: message.into(),
        }
    }

    fn expect_ident(&mut self) -> Result<String, CompileError> {
        if let Token::Ident(name) = self.peek().clone() {
            self.advance();
            Ok(name)
        } else {
            Err(self.error(format!("expected identifier, got {:?}", self.peek())))
        }
    }

    /// Parse a method body: "{" statement* "}"
    pub fn parse_method_body(&mut self) -> Result<Vec<CStmt>, CompileError> {
        self.expect(&Token::LBrace)?;
        let mut stmts = Vec::new();
        while !self.at(&Token::RBrace) && !self.at(&Token::Eof) {
            stmts.push(self.parse_statement()?);
        }
        self.expect(&Token::RBrace)?;
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<CStmt, CompileError> {
        match self.peek() {
            Token::LBrace => {
                self.advance();
                let mut stmts = Vec::new();
                while !self.at(&Token::RBrace) && !self.at(&Token::Eof) {
                    stmts.push(self.parse_statement()?);
                }
                self.expect(&Token::RBrace)?;
                Ok(CStmt::Block(stmts))
            }
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::For => self.parse_for(),
            Token::Switch => self.parse_switch(),
            Token::Synchronized => self.parse_synchronized(),
            Token::Try => self.parse_try_catch(),
            Token::Return => self.parse_return(),
            Token::Throw => self.parse_throw(),
            Token::Break => {
                self.advance();
                self.expect(&Token::Semicolon)?;
                Ok(CStmt::Break)
            }
            Token::Continue => {
                self.advance();
                self.expect(&Token::Semicolon)?;
                Ok(CStmt::Continue)
            }
            Token::Var => {
                self.advance();
                let name = self.expect_ident()?;
                self.expect(&Token::Eq)?;
                let init = self.parse_expression()?;
                self.expect(&Token::Semicolon)?;
                Ok(CStmt::LocalDecl {
                    ty: TypeName::Class("__var__".into()),
                    name,
                    init: Some(init),
                })
            }
            // Attempt type name for local declaration
            Token::KwInt
            | Token::KwLong
            | Token::KwFloat
            | Token::KwDouble
            | Token::KwBoolean
            | Token::KwByte
            | Token::KwChar
            | Token::KwShort
            | Token::KwVoid => self.parse_local_decl(),
            // Could be a local decl with class type or an expression statement
            Token::Ident(_) => {
                // Lookahead to distinguish type declaration from expression
                if self.is_local_decl_start() {
                    self.parse_local_decl()
                } else {
                    self.parse_expr_statement()
                }
            }
            _ => self.parse_expr_statement(),
        }
    }

    /// Lookahead to determine if current position starts a local declaration.
    /// Pattern: Ident ("." Ident)* ("<" ... ">")? ("[" "]")* Ident
    /// vs expression: Ident followed by operator/dot-method/etc.
    fn is_local_decl_start(&self) -> bool {
        let mut i = self.pos;
        // Must start with Ident
        if !matches!(&self.tokens[i].token, Token::Ident(_)) {
            return false;
        }
        i += 1;
        // Skip dotted name: "." Ident
        while i < self.tokens.len() {
            if self.tokens[i].token == Token::Dot {
                i += 1;
                if i < self.tokens.len() && matches!(&self.tokens[i].token, Token::Ident(_)) {
                    i += 1;
                } else {
                    return false;
                }
            } else {
                break;
            }
        }
        // Skip generic type parameters: "<" ... ">"
        if i < self.tokens.len() && self.tokens[i].token == Token::Lt {
            i += 1;
            let mut depth: i32 = 1;
            while i < self.tokens.len() && depth > 0 {
                match &self.tokens[i].token {
                    Token::Lt => {
                        depth += 1;
                        i += 1;
                    }
                    Token::Gt => {
                        depth -= 1;
                        i += 1;
                    }
                    Token::GtGt => {
                        depth -= 2;
                        i += 1;
                    }
                    Token::Eof => break,
                    _ => {
                        i += 1;
                    }
                }
            }
        }
        // Skip array brackets: "[" "]"
        while i + 1 < self.tokens.len()
            && self.tokens[i].token == Token::LBracket
            && self.tokens[i + 1].token == Token::RBracket
        {
            i += 2;
        }
        // Must be followed by an identifier (the variable name)
        i < self.tokens.len() && matches!(&self.tokens[i].token, Token::Ident(_))
    }

    fn parse_if(&mut self) -> Result<CStmt, CompileError> {
        self.expect(&Token::If)?;
        self.expect(&Token::LParen)?;
        let condition = self.parse_expression()?;
        self.expect(&Token::RParen)?;
        let then_body = self.parse_block_or_single()?;
        let else_body = if self.at(&Token::Else) {
            self.advance();
            Some(self.parse_block_or_single()?)
        } else {
            None
        };
        Ok(CStmt::If {
            condition,
            then_body,
            else_body,
        })
    }

    fn parse_while(&mut self) -> Result<CStmt, CompileError> {
        self.expect(&Token::While)?;
        self.expect(&Token::LParen)?;
        let condition = self.parse_expression()?;
        self.expect(&Token::RParen)?;
        let body = self.parse_block_or_single()?;
        Ok(CStmt::While { condition, body })
    }

    fn parse_for(&mut self) -> Result<CStmt, CompileError> {
        self.expect(&Token::For)?;
        self.expect(&Token::LParen)?;

        // Try for-each: for (Type name : expr)
        if self.is_type_start() {
            let saved_pos = self.pos;
            if let Ok(ty) = self.parse_type_name()
                && let Ok(name) = self.expect_ident()
                && self.at(&Token::Colon)
            {
                self.advance(); // consume ':'
                let iterable = self.parse_expression()?;
                self.expect(&Token::RParen)?;
                let body = self.parse_block_or_single()?;
                return Ok(CStmt::ForEach {
                    element_type: ty,
                    var_name: name,
                    iterable,
                    body,
                });
            }
            // Not a for-each, restore position and parse as traditional for
            self.pos = saved_pos;
        }

        // Init
        let init = if self.at(&Token::Semicolon) {
            self.advance();
            None
        } else {
            let stmt = if self.is_type_start() {
                self.parse_local_decl_no_semi()?
            } else {
                let expr = self.parse_expression()?;
                CStmt::ExprStmt(expr)
            };
            self.expect(&Token::Semicolon)?;
            Some(Box::new(stmt))
        };

        // Condition
        let condition = if self.at(&Token::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(&Token::Semicolon)?;

        // Update
        let update = if self.at(&Token::RParen) {
            None
        } else {
            let expr = self.parse_expression()?;
            Some(Box::new(CStmt::ExprStmt(expr)))
        };
        self.expect(&Token::RParen)?;

        let body = self.parse_block_or_single()?;
        Ok(CStmt::For {
            init,
            condition,
            update,
            body,
        })
    }

    fn parse_return(&mut self) -> Result<CStmt, CompileError> {
        self.expect(&Token::Return)?;
        if self.at(&Token::Semicolon) {
            self.advance();
            Ok(CStmt::Return(None))
        } else {
            let expr = self.parse_expression()?;
            self.expect(&Token::Semicolon)?;
            Ok(CStmt::Return(Some(expr)))
        }
    }

    fn parse_throw(&mut self) -> Result<CStmt, CompileError> {
        self.expect(&Token::Throw)?;
        let expr = self.parse_expression()?;
        self.expect(&Token::Semicolon)?;
        Ok(CStmt::Throw(expr))
    }

    fn parse_switch(&mut self) -> Result<CStmt, CompileError> {
        self.expect(&Token::Switch)?;
        self.expect(&Token::LParen)?;
        let expr = self.parse_expression()?;
        self.expect(&Token::RParen)?;
        self.expect(&Token::LBrace)?;

        let mut cases: Vec<SwitchCase> = Vec::new();
        let mut default_body: Option<Vec<CStmt>> = None;
        // Accumulate consecutive case labels before a body
        let mut pending_values: Vec<i64> = Vec::new();

        while !self.at(&Token::RBrace) && !self.at(&Token::Eof) {
            if self.at(&Token::Case) {
                self.advance();
                let value = self.parse_case_value()?;
                self.expect(&Token::Colon)?;
                pending_values.push(value);
            } else if self.at(&Token::Default) {
                self.advance();
                self.expect(&Token::Colon)?;
                // Collect the body for default
                let mut body = Vec::new();
                while !self.at(&Token::RBrace)
                    && !self.at(&Token::Case)
                    && !self.at(&Token::Default)
                    && !self.at(&Token::Eof)
                {
                    body.push(self.parse_statement()?);
                }
                default_body = Some(body);
            } else {
                // This is a statement that belongs to the most recent case/default label(s)
                let mut body = Vec::new();
                while !self.at(&Token::RBrace)
                    && !self.at(&Token::Case)
                    && !self.at(&Token::Default)
                    && !self.at(&Token::Eof)
                {
                    body.push(self.parse_statement()?);
                }
                if !pending_values.is_empty() {
                    cases.push(SwitchCase {
                        values: std::mem::take(&mut pending_values),
                        body,
                    });
                } else {
                    return Err(self.error("unexpected statement outside case/default in switch"));
                }
            }
        }
        // If there are pending values without a body (fall-through to end), add empty case
        if !pending_values.is_empty() {
            cases.push(SwitchCase {
                values: pending_values,
                body: Vec::new(),
            });
        }
        self.expect(&Token::RBrace)?;
        Ok(CStmt::Switch {
            expr,
            cases,
            default_body,
        })
    }

    fn parse_case_value(&mut self) -> Result<i64, CompileError> {
        let negative = if self.at(&Token::Minus) {
            self.advance();
            true
        } else {
            false
        };
        match self.peek().clone() {
            Token::IntLiteral(v) => {
                self.advance();
                Ok(if negative { -v } else { v })
            }
            Token::LongLiteral(v) => {
                self.advance();
                Ok(if negative { -v } else { v })
            }
            _ => Err(self.error(format!(
                "expected integer case value, got {:?}",
                self.peek()
            ))),
        }
    }

    fn parse_try_catch(&mut self) -> Result<CStmt, CompileError> {
        self.expect(&Token::Try)?;
        let try_body = self.parse_block_or_single()?;

        let mut catches = Vec::new();
        let mut finally_body = None;

        while self.at(&Token::Catch) {
            self.advance();
            self.expect(&Token::LParen)?;
            let mut exception_types = vec![self.parse_type_name()?];
            while self.at(&Token::Pipe) {
                self.advance();
                exception_types.push(self.parse_type_name()?);
            }
            let var_name = self.expect_ident()?;
            self.expect(&Token::RParen)?;
            let body = self.parse_block_or_single()?;
            catches.push(CatchClause {
                exception_types,
                var_name,
                body,
            });
        }

        if self.at(&Token::Finally) {
            self.advance();
            finally_body = Some(self.parse_block_or_single()?);
        }

        if catches.is_empty() && finally_body.is_none() {
            return Err(self.error("try requires at least one catch or finally block"));
        }

        Ok(CStmt::TryCatch {
            try_body,
            catches,
            finally_body,
        })
    }

    fn parse_synchronized(&mut self) -> Result<CStmt, CompileError> {
        self.expect(&Token::Synchronized)?;
        self.expect(&Token::LParen)?;
        let lock_expr = self.parse_expression()?;
        self.expect(&Token::RParen)?;
        let body = self.parse_block_or_single()?;
        Ok(CStmt::Synchronized { lock_expr, body })
    }

    fn parse_local_decl(&mut self) -> Result<CStmt, CompileError> {
        let stmt = self.parse_local_decl_no_semi()?;
        self.expect(&Token::Semicolon)?;
        Ok(stmt)
    }

    fn parse_local_decl_no_semi(&mut self) -> Result<CStmt, CompileError> {
        let ty = self.parse_type_name()?;
        let name = self.expect_ident()?;
        let init = if self.at(&Token::Eq) {
            self.advance();
            Some(self.parse_expression()?)
        } else {
            None
        };
        Ok(CStmt::LocalDecl { ty, name, init })
    }

    fn parse_expr_statement(&mut self) -> Result<CStmt, CompileError> {
        let expr = self.parse_expression()?;
        self.expect(&Token::Semicolon)?;
        Ok(CStmt::ExprStmt(expr))
    }

    fn parse_block_or_single(&mut self) -> Result<Vec<CStmt>, CompileError> {
        if self.at(&Token::LBrace) {
            self.advance();
            let mut stmts = Vec::new();
            while !self.at(&Token::RBrace) && !self.at(&Token::Eof) {
                stmts.push(self.parse_statement()?);
            }
            self.expect(&Token::RBrace)?;
            Ok(stmts)
        } else {
            Ok(vec![self.parse_statement()?])
        }
    }

    fn is_type_start(&self) -> bool {
        matches!(
            self.peek(),
            Token::KwInt
                | Token::KwLong
                | Token::KwFloat
                | Token::KwDouble
                | Token::KwBoolean
                | Token::KwByte
                | Token::KwChar
                | Token::KwShort
                | Token::KwVoid
        ) || (matches!(self.peek(), Token::Ident(_)) && self.is_local_decl_start())
    }

    fn parse_type_name(&mut self) -> Result<TypeName, CompileError> {
        let base = match self.peek() {
            Token::KwInt => {
                self.advance();
                TypeName::Primitive(PrimitiveKind::Int)
            }
            Token::KwLong => {
                self.advance();
                TypeName::Primitive(PrimitiveKind::Long)
            }
            Token::KwFloat => {
                self.advance();
                TypeName::Primitive(PrimitiveKind::Float)
            }
            Token::KwDouble => {
                self.advance();
                TypeName::Primitive(PrimitiveKind::Double)
            }
            Token::KwBoolean => {
                self.advance();
                TypeName::Primitive(PrimitiveKind::Boolean)
            }
            Token::KwByte => {
                self.advance();
                TypeName::Primitive(PrimitiveKind::Byte)
            }
            Token::KwChar => {
                self.advance();
                TypeName::Primitive(PrimitiveKind::Char)
            }
            Token::KwShort => {
                self.advance();
                TypeName::Primitive(PrimitiveKind::Short)
            }
            Token::KwVoid => {
                self.advance();
                TypeName::Primitive(PrimitiveKind::Void)
            }
            Token::Ident(_) => {
                let mut name = self.expect_ident()?;
                while self.at(&Token::Dot) {
                    // Peek ahead: if next is ident followed by another dot or bracket or ident (type context), continue
                    if let Token::Ident(_) = &self.tokens[self.pos + 1].token {
                        // Check if this is part of a dotted class name
                        // "Ident.Ident" in type position
                        self.advance(); // consume dot
                        let next = self.expect_ident()?;
                        name = format!("{}.{}", name, next);
                    } else {
                        break;
                    }
                }
                // Skip generic type parameters: List<String>, Map<K,V>, etc. (erased at compile time)
                if self.at(&Token::Lt) {
                    self.skip_type_parameters()?;
                }
                TypeName::Class(name)
            }
            _ => return Err(self.error(format!("expected type name, got {:?}", self.peek()))),
        };

        // Handle array brackets
        let mut ty = base;
        while self.at(&Token::LBracket)
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.token == Token::RBracket)
        {
            self.advance(); // [
            self.advance(); // ]
            ty = TypeName::Array(Box::new(ty));
        }

        Ok(ty)
    }

    // --- Expression parsing with Java operator precedence ---

    fn parse_expression(&mut self) -> Result<CExpr, CompileError> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<CExpr, CompileError> {
        let expr = self.parse_ternary()?;

        match self.peek() {
            Token::Eq => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(CExpr::Assign {
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            Token::PlusEq => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(CExpr::CompoundAssign {
                    op: BinOp::Add,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            Token::MinusEq => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(CExpr::CompoundAssign {
                    op: BinOp::Sub,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            Token::StarEq => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(CExpr::CompoundAssign {
                    op: BinOp::Mul,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            Token::SlashEq => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(CExpr::CompoundAssign {
                    op: BinOp::Div,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            Token::PercentEq => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(CExpr::CompoundAssign {
                    op: BinOp::Rem,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            Token::AmpEq => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(CExpr::CompoundAssign {
                    op: BinOp::BitAnd,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            Token::PipeEq => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(CExpr::CompoundAssign {
                    op: BinOp::BitOr,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            Token::CaretEq => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(CExpr::CompoundAssign {
                    op: BinOp::BitXor,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            Token::LtLtEq => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(CExpr::CompoundAssign {
                    op: BinOp::Shl,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            Token::GtGtEq => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(CExpr::CompoundAssign {
                    op: BinOp::Shr,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            Token::GtGtGtEq => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(CExpr::CompoundAssign {
                    op: BinOp::Ushr,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            _ => Ok(expr),
        }
    }

    fn parse_ternary(&mut self) -> Result<CExpr, CompileError> {
        let expr = self.parse_logical_or()?;
        if self.at(&Token::Question) {
            self.advance();
            let then_expr = self.parse_expression()?;
            self.expect(&Token::Colon)?;
            let else_expr = self.parse_ternary()?;
            Ok(CExpr::Ternary {
                condition: Box::new(expr),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            })
        } else {
            Ok(expr)
        }
    }

    fn parse_logical_or(&mut self) -> Result<CExpr, CompileError> {
        let mut left = self.parse_logical_and()?;
        while self.at(&Token::PipePipe) {
            self.advance();
            let right = self.parse_logical_and()?;
            left = CExpr::LogicalOr(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<CExpr, CompileError> {
        let mut left = self.parse_bitwise_or()?;
        while self.at(&Token::AmpAmp) {
            self.advance();
            let right = self.parse_bitwise_or()?;
            left = CExpr::LogicalAnd(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitwise_or(&mut self) -> Result<CExpr, CompileError> {
        let mut left = self.parse_bitwise_xor()?;
        while self.at(&Token::Pipe) {
            self.advance();
            let right = self.parse_bitwise_xor()?;
            left = CExpr::BinaryOp {
                op: BinOp::BitOr,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_bitwise_xor(&mut self) -> Result<CExpr, CompileError> {
        let mut left = self.parse_bitwise_and()?;
        while self.at(&Token::Caret) {
            self.advance();
            let right = self.parse_bitwise_and()?;
            left = CExpr::BinaryOp {
                op: BinOp::BitXor,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> Result<CExpr, CompileError> {
        let mut left = self.parse_equality()?;
        while self.at(&Token::Amp) {
            self.advance();
            let right = self.parse_equality()?;
            left = CExpr::BinaryOp {
                op: BinOp::BitAnd,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<CExpr, CompileError> {
        let mut left = self.parse_relational()?;
        loop {
            let op = match self.peek() {
                Token::EqEq => CompareOp::Eq,
                Token::BangEq => CompareOp::Ne,
                _ => break,
            };
            self.advance();
            let right = self.parse_relational()?;
            left = CExpr::Comparison {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_relational(&mut self) -> Result<CExpr, CompileError> {
        let mut left = self.parse_shift()?;
        loop {
            match self.peek() {
                Token::Lt => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = CExpr::Comparison {
                        op: CompareOp::Lt,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Token::LtEq => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = CExpr::Comparison {
                        op: CompareOp::Le,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Token::Gt => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = CExpr::Comparison {
                        op: CompareOp::Gt,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Token::GtEq => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = CExpr::Comparison {
                        op: CompareOp::Ge,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Token::Instanceof => {
                    self.advance();
                    let ty = self.parse_type_name()?;
                    left = CExpr::Instanceof {
                        operand: Box::new(left),
                        ty,
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<CExpr, CompileError> {
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                Token::LtLt => BinOp::Shl,
                Token::GtGt => BinOp::Shr,
                Token::GtGtGt => BinOp::Ushr,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = CExpr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<CExpr, CompileError> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = CExpr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<CExpr, CompileError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Rem,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = CExpr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<CExpr, CompileError> {
        match self.peek() {
            Token::Minus => {
                self.advance();
                // Handle negative literals directly
                match self.peek() {
                    Token::IntLiteral(v) => {
                        let v = *v;
                        self.advance();
                        Ok(CExpr::IntLiteral(-v))
                    }
                    Token::LongLiteral(v) => {
                        let v = *v;
                        self.advance();
                        Ok(CExpr::LongLiteral(-v))
                    }
                    Token::FloatLiteral(v) => {
                        let v = *v;
                        self.advance();
                        Ok(CExpr::FloatLiteral(-v))
                    }
                    Token::DoubleLiteral(v) => {
                        let v = *v;
                        self.advance();
                        Ok(CExpr::DoubleLiteral(-v))
                    }
                    _ => {
                        let operand = self.parse_unary()?;
                        Ok(CExpr::UnaryOp {
                            op: UnaryOp::Neg,
                            operand: Box::new(operand),
                        })
                    }
                }
            }
            Token::Bang => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(CExpr::LogicalNot(Box::new(operand)))
            }
            Token::Tilde => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(CExpr::UnaryOp {
                    op: UnaryOp::BitNot,
                    operand: Box::new(operand),
                })
            }
            Token::PlusPlus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(CExpr::PreIncrement(Box::new(operand)))
            }
            Token::MinusMinus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(CExpr::PreDecrement(Box::new(operand)))
            }
            Token::LParen => {
                // Could be cast or parenthesized expression
                if self.is_cast() {
                    self.advance(); // (
                    let ty = self.parse_type_name()?;
                    self.expect(&Token::RParen)?;
                    let operand = self.parse_unary()?;
                    Ok(CExpr::Cast {
                        ty,
                        operand: Box::new(operand),
                    })
                } else {
                    self.parse_postfix()
                }
            }
            _ => self.parse_postfix(),
        }
    }

    /// Distinguish cast from parenthesized expression.
    /// Cast: "(" type_name ")" unary_expr
    fn is_cast(&self) -> bool {
        // Check if ( is followed by a type name and then )
        let mut i = self.pos + 1; // skip (
        if i >= self.tokens.len() {
            return false;
        }
        // Check for primitive type
        if matches!(
            &self.tokens[i].token,
            Token::KwInt
                | Token::KwLong
                | Token::KwFloat
                | Token::KwDouble
                | Token::KwBoolean
                | Token::KwByte
                | Token::KwChar
                | Token::KwShort
        ) {
            i += 1;
            // skip array brackets
            while i + 1 < self.tokens.len()
                && self.tokens[i].token == Token::LBracket
                && self.tokens[i + 1].token == Token::RBracket
            {
                i += 2;
            }
            return i < self.tokens.len() && self.tokens[i].token == Token::RParen;
        }
        // Check for class type cast: (ClassName) expr
        // Only if the ident is likely a type (starts with uppercase) and is followed by )
        if let Token::Ident(name) = &self.tokens[i].token
            && name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        {
            i += 1;
            // Skip dotted name
            while i + 1 < self.tokens.len()
                && self.tokens[i].token == Token::Dot
                && matches!(&self.tokens[i + 1].token, Token::Ident(_))
            {
                i += 2;
            }
            // skip array brackets
            while i + 1 < self.tokens.len()
                && self.tokens[i].token == Token::LBracket
                && self.tokens[i + 1].token == Token::RBracket
            {
                i += 2;
            }
            return i < self.tokens.len() && self.tokens[i].token == Token::RParen;
        }
        false
    }

    fn parse_postfix(&mut self) -> Result<CExpr, CompileError> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.peek() {
                Token::Dot => {
                    self.advance();
                    // Handle generic type parameters before method name: obj.<String>method()
                    if self.at(&Token::Lt) {
                        self.skip_type_parameters()?;
                    }
                    let name = self.expect_ident()?;
                    // Also handle generics after method name: obj.method<String>()
                    if self.at(&Token::Lt) {
                        self.skip_type_parameters()?;
                    }
                    if self.at(&Token::LParen) {
                        // Method call
                        let args = self.parse_args()?;
                        expr = CExpr::MethodCall {
                            object: Some(Box::new(expr)),
                            name,
                            args,
                        };
                    } else {
                        // Field access
                        expr = CExpr::FieldAccess {
                            object: Box::new(expr),
                            name,
                        };
                    }
                }
                Token::LBracket => {
                    self.advance();
                    let index = self.parse_expression()?;
                    self.expect(&Token::RBracket)?;
                    expr = CExpr::ArrayAccess {
                        array: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                Token::PlusPlus => {
                    self.advance();
                    expr = CExpr::PostIncrement(Box::new(expr));
                }
                Token::MinusMinus => {
                    self.advance();
                    expr = CExpr::PostDecrement(Box::new(expr));
                }
                Token::ColonColon => {
                    // Method reference: expr::methodName
                    self.advance();
                    let method_name = self.expect_ident()?;
                    // Extract class name from the expression
                    let class_name = match &expr {
                        CExpr::Ident(name) => name.clone(),
                        _ => return Err(self.error("method reference requires a class name")),
                    };
                    expr = CExpr::MethodRef {
                        class_name,
                        method_name,
                    };
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<CExpr, CompileError> {
        match self.peek().clone() {
            Token::IntLiteral(v) => {
                let v = v;
                self.advance();
                Ok(CExpr::IntLiteral(v))
            }
            Token::LongLiteral(v) => {
                let v = v;
                self.advance();
                Ok(CExpr::LongLiteral(v))
            }
            Token::FloatLiteral(v) => {
                let v = v;
                self.advance();
                Ok(CExpr::FloatLiteral(v))
            }
            Token::DoubleLiteral(v) => {
                let v = v;
                self.advance();
                Ok(CExpr::DoubleLiteral(v))
            }
            Token::StringLiteral(s) => {
                let s = s;
                self.advance();
                Ok(CExpr::StringLiteral(s))
            }
            Token::CharLiteral(c) => {
                self.advance();
                Ok(CExpr::CharLiteral(c))
            }
            Token::True => {
                self.advance();
                Ok(CExpr::BoolLiteral(true))
            }
            Token::False => {
                self.advance();
                Ok(CExpr::BoolLiteral(false))
            }
            Token::Null => {
                self.advance();
                Ok(CExpr::NullLiteral)
            }
            Token::This => {
                self.advance();
                Ok(CExpr::This)
            }
            Token::New => {
                self.advance();
                let ty = self.parse_type_name()?;
                if self.at(&Token::LBracket) {
                    // new Type[size] or new Type[size1][size2]...
                    self.advance();
                    let first_size = self.parse_expression()?;
                    self.expect(&Token::RBracket)?;

                    // Check for multi-dimensional: new Type[expr][expr]...
                    if self.at(&Token::LBracket)
                        && self
                            .tokens
                            .get(self.pos + 1)
                            .is_some_and(|t| t.token != Token::RBracket)
                    {
                        let mut dimensions = vec![first_size];
                        while self.at(&Token::LBracket)
                            && self
                                .tokens
                                .get(self.pos + 1)
                                .is_some_and(|t| t.token != Token::RBracket)
                        {
                            self.advance(); // [
                            dimensions.push(self.parse_expression()?);
                            self.expect(&Token::RBracket)?;
                        }
                        Ok(CExpr::NewMultiArray {
                            element_type: ty,
                            dimensions,
                        })
                    } else {
                        Ok(CExpr::NewArray {
                            element_type: ty,
                            size: Box::new(first_size),
                        })
                    }
                } else if self.at(&Token::LParen) {
                    // new ClassName(args)
                    let class_name = match ty {
                        TypeName::Class(name) => name,
                        _ => {
                            return Err(
                                self.error("cannot use 'new' with primitive type constructor")
                            );
                        }
                    };
                    let args = self.parse_args()?;
                    Ok(CExpr::NewObject { class_name, args })
                } else {
                    Err(self.error("expected '(' or '[' after 'new Type'"))
                }
            }
            Token::Switch => {
                self.advance();
                self.parse_switch_expr()
            }
            Token::LParen => {
                // Check for lambda: () -> or (Type name, ...) ->
                if self.is_lambda_start() {
                    return self.parse_lambda();
                }
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Token::Ident(name) => {
                let name = name;
                self.advance();
                if self.at(&Token::LParen) {
                    // Bare method call (unqualified)
                    let args = self.parse_args()?;
                    Ok(CExpr::MethodCall {
                        object: None,
                        name,
                        args,
                    })
                } else {
                    Ok(CExpr::Ident(name))
                }
            }
            _ => Err(self.error(format!("unexpected token: {:?}", self.peek()))),
        }
    }

    /// Skip generic type parameters `<Type, Type, ...>` in postfix position.
    /// In postfix (after `.name`), `<` is unambiguous — it's always generics, not comparison.
    fn skip_type_parameters(&mut self) -> Result<(), CompileError> {
        self.expect(&Token::Lt)?;
        let mut depth: i32 = 1;
        while depth > 0 && !self.at(&Token::Eof) {
            match self.peek() {
                Token::Lt => {
                    depth += 1;
                    self.advance();
                }
                Token::Gt => {
                    depth -= 1;
                    self.advance();
                }
                Token::GtGt => {
                    depth -= 2;
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
        Ok(())
    }

    fn parse_switch_expr(&mut self) -> Result<CExpr, CompileError> {
        self.expect(&Token::LParen)?;
        let expr = self.parse_expression()?;
        self.expect(&Token::RParen)?;
        self.expect(&Token::LBrace)?;

        let mut cases = Vec::new();
        let mut default_expr = None;

        while !self.at(&Token::RBrace) && !self.at(&Token::Eof) {
            if self.at(&Token::Default) {
                self.advance();
                self.expect(&Token::Arrow)?;
                let e = self.parse_expression()?;
                self.expect(&Token::Semicolon)?;
                default_expr = Some(e);
            } else {
                self.expect(&Token::Case)?;
                let mut values = Vec::new();
                values.push(self.parse_case_value()?);
                while self.at(&Token::Comma) {
                    self.advance();
                    values.push(self.parse_case_value()?);
                }
                self.expect(&Token::Arrow)?;
                let case_expr = self.parse_expression()?;
                self.expect(&Token::Semicolon)?;
                cases.push(SwitchExprCase {
                    values,
                    expr: case_expr,
                });
            }
        }
        self.expect(&Token::RBrace)?;

        let default =
            default_expr.ok_or_else(|| self.error("switch expression requires a default case"))?;

        Ok(CExpr::SwitchExpr {
            expr: Box::new(expr),
            cases,
            default_expr: Box::new(default),
        })
    }

    /// Lookahead to determine if `(` starts a lambda expression.
    /// Patterns: `() ->`, `(Type name) ->`, `(Type name, Type name) ->`
    fn is_lambda_start(&self) -> bool {
        let mut i = self.pos + 1; // skip `(`

        // () -> is a zero-arg lambda
        if i < self.tokens.len() && self.tokens[i].token == Token::RParen {
            return i + 1 < self.tokens.len() && self.tokens[i + 1].token == Token::Arrow;
        }

        // Scan for matching `)` then check for `->`
        let mut depth = 1;
        while i < self.tokens.len() && depth > 0 {
            match &self.tokens[i].token {
                Token::LParen => depth += 1,
                Token::RParen => depth -= 1,
                Token::Eof => return false,
                _ => {}
            }
            i += 1;
        }
        // After `)`, check for `->`
        if depth == 0 && i < self.tokens.len() {
            return self.tokens[i].token == Token::Arrow
                // Fallback: also check i-1 was `)` and next is Arrow
                || (i > 0 && self.tokens[i - 1].token == Token::RParen
                    && i < self.tokens.len()
                    && self.tokens[i].token == Token::Arrow);
        }
        false
    }

    fn parse_lambda(&mut self) -> Result<CExpr, CompileError> {
        self.expect(&Token::LParen)?;
        let mut params = Vec::new();

        if !self.at(&Token::RParen) {
            loop {
                // Try to parse a typed parameter: Type name
                // Or just an identifier (inferred type)
                if self.is_type_start() {
                    let saved = self.pos;
                    if let Ok(ty) = self.parse_type_name() {
                        if let Token::Ident(_) = self.peek() {
                            let name = self.expect_ident()?;
                            params.push(LambdaParam { ty: Some(ty), name });
                        } else {
                            // Not Type name pattern, restore and try ident
                            self.pos = saved;
                            let name = self.expect_ident()?;
                            params.push(LambdaParam { ty: None, name });
                        }
                    } else {
                        self.pos = saved;
                        let name = self.expect_ident()?;
                        params.push(LambdaParam { ty: None, name });
                    }
                } else {
                    let name = self.expect_ident()?;
                    params.push(LambdaParam { ty: None, name });
                }
                if self.at(&Token::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(&Token::RParen)?;
        self.expect(&Token::Arrow)?;

        let body = if self.at(&Token::LBrace) {
            let stmts = self.parse_block_or_single()?;
            LambdaBody::Block(stmts)
        } else {
            let expr = self.parse_expression()?;
            LambdaBody::Expr(Box::new(expr))
        };

        Ok(CExpr::Lambda { params, body })
    }

    fn parse_args(&mut self) -> Result<Vec<CExpr>, CompileError> {
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();
        if !self.at(&Token::RParen) {
            args.push(self.parse_expression()?);
            while self.at(&Token::Comma) {
                self.advance();
                args.push(self.parse_expression()?);
            }
        }
        self.expect(&Token::RParen)?;
        Ok(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::lexer::Lexer;

    fn parse(src: &str) -> Vec<CStmt> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse_method_body().unwrap()
    }

    #[test]
    fn test_return_literal() {
        let stmts = parse("{ return 42; }");
        assert_eq!(stmts, vec![CStmt::Return(Some(CExpr::IntLiteral(42)))]);
    }

    #[test]
    fn test_local_decl() {
        let stmts = parse("{ int x = 10; }");
        assert_eq!(
            stmts,
            vec![CStmt::LocalDecl {
                ty: TypeName::Primitive(PrimitiveKind::Int),
                name: "x".into(),
                init: Some(CExpr::IntLiteral(10)),
            }]
        );
    }

    #[test]
    fn test_if_else() {
        let stmts = parse("{ if (x > 0) { return 1; } else { return 0; } }");
        assert!(matches!(
            &stmts[0],
            CStmt::If {
                else_body: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn test_while_loop() {
        let stmts = parse("{ while (i < 10) { i = i + 1; } }");
        assert!(matches!(&stmts[0], CStmt::While { .. }));
    }

    #[test]
    fn test_for_loop() {
        let stmts = parse("{ for (int i = 0; i < 10; i++) { x = x + i; } }");
        assert!(matches!(&stmts[0], CStmt::For { .. }));
    }

    #[test]
    fn test_method_call() {
        let stmts = parse("{ System.out.println(\"hello\"); }");
        assert!(matches!(
            &stmts[0],
            CStmt::ExprStmt(CExpr::MethodCall { .. })
        ));
    }

    #[test]
    fn test_new_object() {
        let stmts = parse("{ StringBuilder sb = new StringBuilder(); }");
        assert!(matches!(
            &stmts[0],
            CStmt::LocalDecl {
                init: Some(CExpr::NewObject { .. }),
                ..
            }
        ));
    }

    #[test]
    fn test_arithmetic_precedence() {
        let stmts = parse("{ return a + b * c; }");
        match &stmts[0] {
            CStmt::Return(Some(CExpr::BinaryOp {
                op: BinOp::Add,
                right,
                ..
            })) => {
                assert!(matches!(
                    right.as_ref(),
                    CExpr::BinaryOp { op: BinOp::Mul, .. }
                ));
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn test_ternary() {
        let stmts = parse("{ return x > 0 ? 1 : -1; }");
        match &stmts[0] {
            CStmt::Return(Some(CExpr::Ternary { .. })) => {}
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn test_cast() {
        let stmts = parse("{ long x = (long) y; }");
        match &stmts[0] {
            CStmt::LocalDecl {
                init: Some(CExpr::Cast { ty, .. }),
                ..
            } => {
                assert_eq!(*ty, TypeName::Primitive(PrimitiveKind::Long));
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn test_array_access() {
        let stmts = parse("{ return arr[i]; }");
        match &stmts[0] {
            CStmt::Return(Some(CExpr::ArrayAccess { .. })) => {}
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn test_class_type_decl() {
        let stmts = parse("{ String s = \"hello\"; }");
        assert_eq!(
            stmts,
            vec![CStmt::LocalDecl {
                ty: TypeName::Class("String".into()),
                name: "s".into(),
                init: Some(CExpr::StringLiteral("hello".into())),
            }]
        );
    }

    #[test]
    fn test_compound_assign() {
        let stmts = parse("{ x += 1; }");
        match &stmts[0] {
            CStmt::ExprStmt(CExpr::CompoundAssign { op: BinOp::Add, .. }) => {}
            other => panic!("unexpected: {:?}", other),
        }
    }
}
