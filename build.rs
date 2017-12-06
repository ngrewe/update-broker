extern crate gcc;

const SRC: &str = "c-wrap/lib.cpp";

fn main() {
    println!("cargo:rerun-if-changed={}", SRC);

    gcc::Build::new()
        .file(SRC)
        .cpp(true)
        .flag("-std=gnu++11")
        .compile("libue-apt-c.a");
}