use crate::attribute_info::AttributeInfo;

#[derive(Clone, Debug)]
pub struct MethodInfo {
    pub access_flags: MethodAccessFlags,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub attributes_count: u16,
    pub attributes: Vec<AttributeInfo>,
}

bitflags! {
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
    pub struct MethodAccessFlags: u16 {
        const PUBLIC = 0x0001;       // 	Declared public; may be accessed from outside its package.
        const PRIVATE = 0x0002;      // 	Declared private; accessible only within the defining class.
        const PROTECTED = 0x0004;    // 	Declared protected; may be accessed within subclasses.
        const STATIC = 0x0008;       // 	Declared static.
        const FINAL = 0x0010;        // 	Declared final; must not be overridden.
        const SYNCHRONIZED = 0x0020; // 	Declared synchronized; invocation is wrapped by a monitor use.
        const BRIDGE = 0x0040;       // 	A bridge method, generated by the compiler.
        const VARARGS = 0x0080;      // 	Declared with variable number of arguments.
        const NATIVE = 0x0100;       //  Declared native; implemented in a language other than Java
        const ABSTRACT = 0x0400;     // 	Declared abstract; no implementation is provided.
        const STRICT = 0x0800;       // 	Declared strictfp; floating-point mode is FP-strict.
        const SYNTHETIC = 0x1000;    // 	Declared synthetic; not present in the source code.
    }
}

#[cfg(test)]
#[allow(dead_code)]
trait TraitTester:
    Copy + Clone + PartialEq + Eq + PartialOrd + Ord + ::std::hash::Hash + ::std::fmt::Debug
{
}

#[cfg(test)]
impl TraitTester for MethodAccessFlags {}
