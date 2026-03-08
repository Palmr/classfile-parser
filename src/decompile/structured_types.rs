use super::cfg_types::BlockId;
use super::expr::{Expr, LocalVar, Stmt};

/// A structured statement — the result of control flow structuring.
/// Represents Java-level control flow constructs.
#[derive(Clone, Debug)]
pub enum StructuredStmt {
    /// A simple statement (from stack simulation).
    Simple(Stmt),
    /// A sequence of statements.
    Block(Vec<StructuredStmt>),
    /// if / if-else
    If {
        condition: Expr,
        then_body: Box<StructuredStmt>,
        else_body: Option<Box<StructuredStmt>>,
    },
    /// while loop
    While {
        condition: Expr,
        body: Box<StructuredStmt>,
    },
    /// do-while loop
    DoWhile {
        body: Box<StructuredStmt>,
        condition: Expr,
    },
    /// for loop
    For {
        init: Option<Box<StructuredStmt>>,
        condition: Expr,
        update: Option<Box<StructuredStmt>>,
        body: Box<StructuredStmt>,
    },
    /// for-each loop (desugared from iterator or array index pattern)
    ForEach {
        var: LocalVar,
        iterable: Expr,
        body: Box<StructuredStmt>,
    },
    /// switch statement
    Switch {
        expr: Expr,
        cases: Vec<SwitchCase>,
        default: Option<Box<StructuredStmt>>,
    },
    /// try-catch-finally
    TryCatch {
        try_body: Box<StructuredStmt>,
        catches: Vec<CatchClause>,
        finally_body: Option<Box<StructuredStmt>>,
    },
    /// try-with-resources (desugared)
    TryWithResources {
        resources: Vec<(LocalVar, Expr)>,
        body: Box<StructuredStmt>,
        catches: Vec<CatchClause>,
    },
    /// synchronized block
    Synchronized {
        object: Expr,
        body: Box<StructuredStmt>,
    },
    /// Labeled statement (for break/continue targets)
    Labeled {
        label: String,
        body: Box<StructuredStmt>,
    },
    /// break statement
    Break { label: Option<String> },
    /// continue statement
    Continue { label: Option<String> },
    /// assert statement (desugared)
    Assert {
        condition: Expr,
        message: Option<Expr>,
    },
    /// Fallback for irreducible control flow
    UnstructuredGoto { target: BlockId },
    /// Comment (used for error recovery, bytecode fallback, etc.)
    Comment(String),
}

/// A switch case arm.
#[derive(Clone, Debug)]
pub struct SwitchCase {
    pub values: Vec<SwitchValue>,
    pub body: StructuredStmt,
    pub falls_through: bool,
}

/// Value for a switch case label.
#[derive(Clone, Debug)]
pub enum SwitchValue {
    Int(i32),
    String(String),
    Enum {
        type_name: String,
        const_name: String,
    },
}

/// A catch clause in a try-catch.
#[derive(Clone, Debug)]
pub struct CatchClause {
    pub exception_type: Option<String>,
    pub var: LocalVar,
    pub body: StructuredStmt,
}

/// A structured method body: the sequence of structured statements.
#[derive(Clone, Debug)]
pub struct StructuredBody {
    pub statements: Vec<StructuredStmt>,
}

impl StructuredBody {
    pub fn new(statements: Vec<StructuredStmt>) -> Self {
        Self { statements }
    }
}
