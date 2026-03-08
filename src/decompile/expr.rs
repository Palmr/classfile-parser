use super::cfg_types::BlockId;
use super::descriptor::JvmType;

/// Binary operators.
#[derive(Clone, Debug, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    Ushr,
    And,
    Or,
    Xor,
}

/// Unary operators.
#[derive(Clone, Debug, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not, // bitwise not (for boolean negation in conditions)
}

/// Comparison operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Ge,
    Gt,
    Le,
}

impl CompareOp {
    /// Returns the negated comparison.
    pub fn negate(self) -> Self {
        match self {
            CompareOp::Eq => CompareOp::Ne,
            CompareOp::Ne => CompareOp::Eq,
            CompareOp::Lt => CompareOp::Ge,
            CompareOp::Ge => CompareOp::Lt,
            CompareOp::Gt => CompareOp::Le,
            CompareOp::Le => CompareOp::Gt,
        }
    }

    /// Java source token for this operator.
    pub fn as_str(&self) -> &'static str {
        match self {
            CompareOp::Eq => "==",
            CompareOp::Ne => "!=",
            CompareOp::Lt => "<",
            CompareOp::Ge => ">=",
            CompareOp::Gt => ">",
            CompareOp::Le => "<=",
        }
    }
}

/// Method invocation kind.
#[derive(Clone, Debug, PartialEq)]
pub enum InvokeKind {
    Virtual,
    Special,
    Static,
    Interface,
}

/// Local variable reference.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalVar {
    pub index: u16,
    pub name: Option<String>,
    pub ty: JvmType,
}

/// Expression tree node -- represents a value-producing computation.
#[derive(Clone, Debug)]
pub enum Expr {
    // --- Literals ---
    IntLiteral(i32),
    LongLiteral(i64),
    FloatLiteral(f32),
    DoubleLiteral(f64),
    StringLiteral(String),
    ClassLiteral(String),
    NullLiteral,

    // --- Variables ---
    LocalLoad(LocalVar),
    This,

    // --- Operations ---
    BinaryOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Cast {
        target_type: JvmType,
        operand: Box<Expr>,
    },
    Instanceof {
        operand: Box<Expr>,
        check_type: String,
    },

    // --- Field access ---
    FieldGet {
        object: Option<Box<Expr>>,
        class_name: String,
        field_name: String,
        field_type: JvmType,
    },

    // --- Method invocation ---
    MethodCall {
        kind: InvokeKind,
        object: Option<Box<Expr>>,
        class_name: String,
        method_name: String,
        descriptor: String,
        args: Vec<Expr>,
        return_type: JvmType,
    },

    // --- Object creation ---
    New {
        class_name: String,
        constructor_descriptor: String,
        args: Vec<Expr>,
    },
    NewArray {
        element_type: JvmType,
        length: Box<Expr>,
    },
    NewMultiArray {
        element_type: JvmType,
        dimensions: Vec<Expr>,
    },
    ArrayLength {
        array: Box<Expr>,
    },
    ArrayLoad {
        array: Box<Expr>,
        index: Box<Expr>,
        element_type: JvmType,
    },

    // --- Comparison ---
    Compare {
        op: CompareOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// Result of lcmp/fcmpl/fcmpg/dcmpl/dcmpg: -1, 0, or 1
    CmpResult {
        kind: CmpKind,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    // --- invokedynamic (lambdas) ---
    InvokeDynamic {
        bootstrap_index: u16,
        method_name: String,
        descriptor: String,
        captures: Vec<Expr>,
    },

    // --- Ternary (synthesized during structuring) ---
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },

    // --- Fallback ---
    Unresolved(String),

    // --- Stack bookkeeping (used during simulation, cleaned up after) ---
    Dup(Box<Expr>),
    /// Marker for an uninitialized `new` before <init> is called
    UninitNew {
        class_name: String,
    },
}

/// Compare instruction kinds (for lcmp, fcmpl, etc.)
#[derive(Clone, Debug, PartialEq)]
pub enum CmpKind {
    LCmp,
    FCmpL,
    FCmpG,
    DCmpL,
    DCmpG,
}

/// Statement -- represents a side-effecting operation.
#[derive(Clone, Debug)]
pub enum Stmt {
    LocalStore {
        var: LocalVar,
        value: Expr,
    },
    FieldStore {
        object: Option<Expr>,
        class_name: String,
        field_name: String,
        field_type: JvmType,
        value: Expr,
    },
    ArrayStore {
        array: Expr,
        index: Expr,
        value: Expr,
    },
    ExprStmt(Expr),
    Iinc {
        var: LocalVar,
        amount: i32,
    },
    Return(Option<Expr>),
    Throw(Expr),
    Monitor {
        enter: bool,
        object: Expr,
    },
}

/// A simulated basic block: the result of stack-simulating one BasicBlock.
#[derive(Clone, Debug)]
pub struct SimulatedBlock {
    pub id: BlockId,
    pub statements: Vec<Stmt>,
    pub exit_stack: Vec<Expr>,
    pub terminator: super::cfg_types::Terminator,
    /// Branch condition expression (populated for ConditionalBranch terminators)
    pub branch_condition: Option<Expr>,
}
