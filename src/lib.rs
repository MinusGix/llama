#[allow(non_snake_case)]
macro_rules! cstr {
    ($x:expr) => {
        std::ffi::CString::new($x).expect("Invalid C string")
    };
}

macro_rules! llvm_inner_impl {
    ($t:ty, $u:ty) => {
        impl<'a> LLVMInner<$u> for $t {
            fn llvm_inner(&self) -> *mut $u {
                self.0.as_ptr()
            }
        }
    };
}

macro_rules! instr {
    ($x:ident($(&$amp:ident,)? $($n:ident : $t:ty),*$(,)?) $b:block) => {
        pub fn $x($(& $amp,)? $($n : $t),*) -> Result<Value<'a>, Error> {
            unsafe {
                Value::from_inner($b)
            }
        }
    }
}

extern "C" {
    fn strlen(_: *const std::os::raw::c_char) -> usize;
}

mod attribute;
mod basic_block;
mod binary;
mod builder;
mod codegen;
mod context;
mod error;
mod execution_engine;
mod module;
mod pass_manager;
mod typ;
mod value;

pub(crate) use std::ffi::c_void;
pub(crate) use std::marker::PhantomData;
pub(crate) use std::os::raw::{c_char, c_int, c_uint};
pub(crate) use std::ptr::NonNull;

pub(crate) use llvm_sys as llvm;

pub use crate::attribute::Attribute;
pub use crate::basic_block::BasicBlock;
pub use crate::binary::Binary;
pub use crate::builder::Builder;
pub use crate::codegen::Codegen;
pub use crate::context::Context;
pub use crate::error::Error;
pub use crate::execution_engine::ExecutionEngine;
pub use crate::module::Module;
pub use crate::pass_manager::PassManager;
pub use crate::typ::{FunctionType, StructType, Type, TypeKind};
pub use crate::value::{Const, Function, Value, ValueKind};

pub use llvm::{
    object::LLVMBinaryType as BinaryType, LLVMAtomicOrdering as AtomicOrdering,
    LLVMCallConv as CallConv, LLVMDiagnosticSeverity as DiagnosticSeverity,
    LLVMInlineAsmDialect as InlineAsmDialect, LLVMIntPredicate as IntPredicate,
    LLVMLinkage as Linkage, LLVMModuleFlagBehavior as ModuleFlagBehavior, LLVMOpcode as OpCode,
    LLVMRealPredicate as RealPredicate, LLVMThreadLocalMode as ThreadLocalMode,
    LLVMUnnamedAddr as UnnamedAddr, LLVMVisibility as Visibility,
};

/// Allows for llama types to be converted into LLVM pointers
pub trait LLVMInner<T> {
    /// Return a LLVM pointer
    fn llvm_inner(&self) -> *mut T;
}

pub(crate) fn wrap_inner<T>(x: *mut T) -> Result<NonNull<T>, Error> {
    match NonNull::new(x) {
        Some(x) => Ok(x),
        None => Err(Error::NullPointer),
    }
}

/// Wraps LLVM messages, these are strings that should be freed using LLVMDisposeMessage
pub struct Message(*mut c_char);
impl Message {
    pub(crate) fn from_raw(c: *mut c_char) -> Message {
        Message(c)
    }

    /// Message length
    pub fn len(&self) -> usize {
        if self.0.is_null() {
            return 0;
        }

        unsafe { strlen(self.0) }
    }
}

impl AsRef<str> for Message {
    fn as_ref(&self) -> &str {
        if self.0.is_null() {
            return "<NULL>";
        }

        unsafe {
            let st = std::slice::from_raw_parts(self.0 as *const u8, self.len());
            std::str::from_utf8_unchecked(st)
        }
    }
}

impl From<Message> for String {
    fn from(m: Message) -> String {
        m.as_ref().into()
    }
}

impl Drop for Message {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { llvm::core::LLVMDisposeMessage(self.0) }
        }
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{}", self.as_ref())
    }
}

impl std::fmt::Debug for Message {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{}", self)
    }
}

/// Memory buffer wraps LLVMMemoryBufferRef
pub struct MemoryBuffer(NonNull<llvm::LLVMMemoryBuffer>);

llvm_inner_impl!(MemoryBuffer, llvm::LLVMMemoryBuffer);

impl MemoryBuffer {
    pub(crate) fn from_raw(ptr: *mut llvm::LLVMMemoryBuffer) -> Result<Self, Error> {
        Ok(MemoryBuffer(wrap_inner(ptr)?))
    }

    /// Create new memory buffer from file
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<MemoryBuffer, Error> {
        let path = match path.as_ref().to_str() {
            Some(p) => cstr!(p),
            None => return Err(Error::InvalidPath),
        };

        let mut mem = std::ptr::null_mut();
        let mut message = std::ptr::null_mut();

        let ok = unsafe {
            llvm::core::LLVMCreateMemoryBufferWithContentsOfFile(
                path.as_ptr(),
                &mut mem,
                &mut message,
            ) == 1
        };

        let message = Message::from_raw(message);
        if !ok {
            return Err(Error::Message(message));
        }

        Self::from_raw(mem)
    }

    /// Create new memory buffer from slice
    pub fn from_slice(name: impl AsRef<str>, s: impl AsRef<[u8]>) -> Result<MemoryBuffer, Error> {
        let name = cstr!(name.as_ref());
        let s = s.as_ref();
        let mem = unsafe {
            llvm::core::LLVMCreateMemoryBufferWithMemoryRangeCopy(
                s.as_ptr() as *const c_char,
                s.len(),
                name.as_ptr(),
            )
        };

        Self::from_raw(mem)
    }

    /// Number of bytes in buffer
    pub fn len(&self) -> usize {
        unsafe { llvm::core::LLVMGetBufferSize(self.0.as_ptr()) }
    }

    /// Write buffer to the specified file
    pub fn write_to_file(&self, path: impl AsRef<std::path::Path>) -> Result<(), Error> {
        let mut f = std::fs::File::create(path)?;
        std::io::Write::write_all(&mut f, self.as_ref())?;
        Ok(())
    }
}

impl AsRef<[u8]> for MemoryBuffer {
    fn as_ref(&self) -> &[u8] {
        let size = self.len();
        unsafe {
            let data = llvm::core::LLVMGetBufferStart(self.0.as_ptr());
            std::slice::from_raw_parts(data as *const u8, size)
        }
    }
}

impl Drop for MemoryBuffer {
    fn drop(&mut self) {
        unsafe { llvm::core::LLVMDisposeMemoryBuffer(self.0.as_ptr()) }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn it_works() {
        let context = Context::new().unwrap();
        let module = Module::new(&context, "testing").unwrap();
        let i32 = Type::int(&context, 32);
        assert_eq!(module.identifier().unwrap(), "testing");
    }
}
