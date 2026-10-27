use std::env;
fn main() {
    let envvar = env::var("_JOURNAL_LD_LIBRARY_PATH");
    println!("cargo:warning={:?}", envvar);
    match envvar {
        Err(_) => {}
        Ok(val) => {
            println!("cargo:rustc-env=LD_LIBRARY_PATH={val}");
            println!("cargo:warning=LD_LIBRARY_PATH={val}");
        }
    };
}
