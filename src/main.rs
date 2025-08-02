use bk::backup::run_backup;

// TODO : add basic ctrl+c support for ending bk tasks instead of everything and ensure cleanups

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    if let Some(conf) = args.get(1) {
        let conf = toml::from_str(&std::fs::read_to_string(conf).unwrap()).unwrap();
        run_backup(conf);
    } else {
        println!("Usage: bk <config>");
    }
}
