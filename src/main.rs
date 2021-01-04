mod error;
mod ir;
mod jit;

use jit::BfVM;
use std::{
    io::{stdin, stdout},
    path::PathBuf,
};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(name = "FILE")]
    file_path: PathBuf,
}

// target/release/bf-jit hello-world.bf
fn main() {
    let opt = Opt::from_args();

    let stdin = stdin();
    let stdout = stdout();

    let ret = BfVM::new(
        &opt.file_path,
        Box::new(stdin.lock()),
        Box::new(stdout.lock()),
    )
    .and_then(|mut vm| vm.run());

    if let Err(e) = &ret {
        eprintln!("bfjit: {}", e);
    }

    std::process::exit(ret.is_err() as i32)
}
