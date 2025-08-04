use bk::backup::run_backup;

// TODO : add basic ctrl+c support for ending bk tasks instead of everything and ensure cleanups

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }
    env_logger::init();
    let args = std::env::args().collect::<Vec<_>>();

    if let Some(conf) = args.get(1) {
        let conf = toml::from_str(&std::fs::read_to_string(conf).unwrap()).unwrap();
        let state = run_backup(conf);
        std::process::exit(state);
    } else {
        println!("Usage: bk <config>");
    }
}
