use bk::cli::BkArgs;

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }
    env_logger::init();

    let args: BkArgs = argh::from_env();
    args.run();
}
