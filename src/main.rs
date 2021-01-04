mod error;
mod ir;
mod jit;

use jit::BfVM;
use std::{
    env,
    io::{stdin, stdout},
    path::Path,
};

// target/release/bf-jit hello-world.bf
fn main() {
    let stdin = stdin();
    let stdout = stdout();

    let ret = BfVM::new(
        Path::new(&env::args().nth(1).unwrap()),
        Box::new(stdin.lock()),
        Box::new(stdout.lock()),
    )
    .and_then(|mut vm| vm.run());

    if let Err(e) = &ret {
        eprintln!("bfjit: {}", e);
    }

    std::process::exit(ret.is_err() as i32)
}
