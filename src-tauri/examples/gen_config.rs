//! Parse a share link and print the sing-box config our app would generate.
//! Usage: cargo run --example gen_config -- "vless://..." [global|rule|direct]

use rocket_win_lib::model::Settings;
use rocket_win_lib::{parser, singbox};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let link = args.get(1).expect("pass a share link as arg 1");
    let mode = args.get(2).map(|s| s.as_str()).unwrap_or("global");

    let server = parser::parse_link(link).expect("failed to parse link");
    eprintln!("parsed: {server:#?}");

    let mut settings = Settings::default();
    settings.listen_port = 11080;
    settings.clash_api_port = 19191;

    let cfg = singbox::build_config(&server, mode, &settings, "cache.db", &[]);
    println!("{}", serde_json::to_string_pretty(&cfg).unwrap());
}
