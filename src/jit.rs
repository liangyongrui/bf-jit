use crate::error::{Result, RuntimeError, VMError};
use crate::ir::{
    self,
    BfIR::{self, *},
};
use dynasmrt::{dynasm, DynasmApi, DynasmLabelApi};
use std::io::{Read, Write};
use std::path::Path;

const MEMORY_SIZE: usize = 4 * 1024 * 1024;

pub struct BfVM<'io> {
    code: dynasmrt::ExecutableBuffer,
    start: dynasmrt::AssemblyOffset,
    memory: Box<[u8]>,
    input: Box<dyn Read + 'io>,
    output: Box<dyn Write + 'io>,
}

fn vm_error(re: RuntimeError) -> *mut VMError {
    let e = Box::new(VMError::from(re));
    Box::into_raw(e)
}

impl<'io> BfVM<'io> {
    unsafe extern "sysv64" fn getbyte(this: *mut Self, ptr: *mut u8) -> *mut VMError {
        let mut buf = [0_u8];
        match (*this).input.read(&mut buf) {
            Ok(0) => {}
            Ok(1) => *ptr = buf[0],
            Err(e) => return vm_error(RuntimeError::IO(e)),
            _ => unreachable!(),
        }
        std::ptr::null_mut()
    }

    unsafe extern "sysv64" fn putbyte(this: *mut Self, ptr: *const u8) -> *mut VMError {
        let buf = std::slice::from_ref(&*ptr);
        match (*this).output.write_all(buf) {
            Ok(()) => std::ptr::null_mut(),
            Err(e) => vm_error(RuntimeError::IO(e)),
        }
    }

    unsafe extern "sysv64" fn overflow_error() -> *mut VMError {
        vm_error(RuntimeError::PointerOverflow)
    }
}

impl<'io> BfVM<'io> {
    pub fn new(
        file_path: &Path,
        input: Box<dyn Read + 'io>,
        output: Box<dyn Write + 'io>,
    ) -> Result<Self> {
        let src = std::fs::read_to_string(file_path)?;
        let ir = ir::compile(&src)?;
        let (code, start) = Self::compile(&ir)?;
        let memory = vec![0; MEMORY_SIZE].into_boxed_slice();
        Ok(Self {
            code,
            start,
            input,
            output,
            memory,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        type RawFn = unsafe extern "sysv64" fn(
            this: *mut BfVM,
            memory_start: *mut u8,
            memory_end: *const u8,
        ) -> *mut VMError;

        let raw_fn: RawFn = unsafe { std::mem::transmute(self.code.ptr(self.start)) };

        let memory_start = self.memory.as_mut_ptr();
        let memory_end = unsafe { memory_start.add(MEMORY_SIZE) };

        let ret = unsafe { raw_fn(self, memory_start, memory_end) };

        if ret.is_null() {
            Ok(())
        } else {
            Err(*unsafe { Box::from_raw(ret) })
        }
    }
}

impl<'io> BfVM<'io> {
    fn compile(code: &[BfIR]) -> Result<(dynasmrt::ExecutableBuffer, dynasmrt::AssemblyOffset)> {
        let mut ops = dynasmrt::x64::Assembler::new()?;
        let start = ops.offset();

        let mut loops = vec![];

        dynasm!(ops
            ; push rax
            ; mov r12, rdi   // save this
            ; mov r13, rsi   // save memory_start
            ; mov r14, rdx   // save memory_end
            ; mov rcx, rsi   // ptr = memory_start
        );

        for ir in code {
            match *ir {
                AddPtr(x) => dynasm!(ops
                    ; add rcx, x as _        // ptr += x
                    ; jc  -> overflow        // jmp if overflow
                    ; cmp rcx, r14           // ptr - memory_end
                    ; jnb -> overflow        // jmp if ptr >= memory_end
                ),
                SubPtr(x) => dynasm!(ops
                    ; sub rcx, x as _       // ptr -= x
                    ; jc  -> overflow       // jmp if overflow
                    ; cmp rcx, r13          // ptr - memory_start
                    ; jb  -> overflow       // jmp if ptr < memory_start
                ),
                AddVal(x) => dynasm!(ops
                    ; add BYTE [rcx], x as _  // *ptr += x
                ),
                SubVal(x) => dynasm!(ops
                    ; sub BYTE [rcx], x as _  // *ptr -= x
                ),
                GetByte => dynasm!(ops
                    ; mov  r15, rcx         // save ptr
                    ; mov  rdi, r12
                    ; mov  rsi, rcx         // arg0: this, arg1: ptr
                    ; mov  rax, QWORD BfVM::getbyte as _
                    ; call rax              // getbyte(this, ptr)
                    ; cmp rax, 0
                    ; jnz  -> io_error      // jmp if rax != 0
                    ; mov  rcx, r15         // recover ptr
                ),
                PutByte => dynasm!(ops
                    ; mov  r15, rcx         // save ptr
                    ; mov  rdi, r12
                    ; mov  rsi, rcx         // arg0: this, arg1: ptr
                    ; mov  rax, QWORD BfVM::putbyte as _
                    ; call rax              // putbyte(this, ptr)
                    ; cmp rax, 0
                    ; jnz  -> io_error      // jmp if rax != 0
                    ; mov  rcx, r15         // recover ptr
                ),
                Jz => {
                    let left = ops.new_dynamic_label();
                    let right = ops.new_dynamic_label();
                    loops.push((left, right));

                    dynasm!(ops
                        ; cmp BYTE [rcx], 0
                        ; jz => right       // jmp if *ptr == 0
                        ; => left
                    )
                }
                Jnz => {
                    let (left, right) = loops.pop().unwrap();
                    dynasm!(ops
                        ; cmp BYTE [rcx], 0
                        ; jnz => left       // jmp if *ptr != 0
                        ; => right
                    )
                }
            }
        }

        dynasm!(ops
            ; xor rax, rax
            ; jmp > exit
            ; -> overflow:
            ; mov rax, QWORD BfVM::overflow_error as _
            ; call rax
            ; jmp > exit
            ; -> io_error:
            ; exit:
            ; pop rdx
            ; ret
        );

        let code = ops.finalize().unwrap();

        Ok((code, start))
    }
}
