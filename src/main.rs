use bk::cli::BkArgs;

// TODO : add basic ctrl+c support for ending bk tasks instead of everything and ensure cleanups

fn main() {
    // Enable log output by default
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }
    env_logger::init();

    let args: BkArgs = argh::from_env();
    args.run();
}
