//! `novo`: see the crate documentation in `lib.rs`.

fn main() {
    let code = novo_cli::run(
        std::env::args_os(),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    std::process::exit(code);
}
