use crate::*;

pub struct ModulePassManager<'a>(NonNull<llvm::LLVMPassManager>, PhantomData<&'a ()>);
pub struct FuncPassManager<'a>(NonNull<llvm::LLVMPassManager>, PhantomData<&'a ()>);

pub trait PassManager: LLVM<llvm::LLVMPassManager> {
    type Kind;

    fn run(&self, f: &Self::Kind) -> bool;

    fn add(&self, transforms: impl AsRef<[Transform]>) {
        for transform in transforms.as_ref().iter() {
            unsafe { transform(self.llvm()) }
        }
    }
}

llvm_inner_impl!(ModulePassManager<'a>, llvm::LLVMPassManager);
llvm_inner_impl!(FuncPassManager<'a>, llvm::LLVMPassManager);

pub type Transform = unsafe extern "C" fn(_: *mut llvm::LLVMPassManager);

pub use llvm::transforms;

impl<'a> Drop for ModulePassManager<'a> {
    fn drop(&mut self) {
        unsafe { llvm::core::LLVMDisposePassManager(self.llvm()) }
    }
}

impl<'a> Drop for FuncPassManager<'a> {
    fn drop(&mut self) {
        unsafe { llvm::core::LLVMDisposePassManager(self.llvm()) }
    }
}

impl<'a> FuncPassManager<'a> {
    pub fn new() -> Result<FuncPassManager<'a>, Error> {
        let ptr = unsafe { llvm::core::LLVMCreatePassManager() };

        Ok(FuncPassManager(wrap_inner(ptr)?, PhantomData))
    }
}

impl<'a> ModulePassManager<'a> {
    pub fn new(module: &Module<'a>) -> Result<ModulePassManager<'a>, Error> {
        let ptr = unsafe { llvm::core::LLVMCreateFunctionPassManagerForModule(module.llvm()) };

        Ok(ModulePassManager(wrap_inner(ptr)?, PhantomData))
    }
}

impl<'a> PassManager for FuncPassManager<'a> {
    type Kind = Func<'a>;

    fn run(&self, f: &Func<'a>) -> bool {
        unsafe { llvm::core::LLVMRunFunctionPassManager(self.llvm(), f.as_ref().llvm()) == 1 }
    }
}

impl<'a> PassManager for ModulePassManager<'a> {
    type Kind = Module<'a>;

    fn run(&self, module: &Module<'a>) -> bool {
        unsafe { llvm::core::LLVMRunPassManager(self.llvm(), module.llvm()) == 1 }
    }
}
