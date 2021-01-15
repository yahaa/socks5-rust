use chrono::Local;
use std::io::Write;

pub fn init() {
    std::env::set_var("RUST_LOG", "info,socks=trace");
    env_logger::Builder::from_env(env_logger::Env::default())
        .format(|buf, record| {
            let level = { buf.default_styled_level(record.level()) };
            writeln!(
                buf,
                "[{} {} {}:{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                level,
                record.module_path().unwrap_or("<unnamed>"),
                record.line().unwrap_or(0),
                &record.args()
            )
        })
        .init();
}
