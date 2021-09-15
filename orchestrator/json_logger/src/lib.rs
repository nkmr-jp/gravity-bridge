use once_cell::sync::Lazy;
use slog::{PushFnValue, *};
use std::fs::OpenOptions;
use std::sync::Mutex;
use chrono;

// refs: https://rust.graystorm.com/tag/crate-slog/
// refs: https://github.com/slog-rs/slog/issues/123

#[derive(Debug)]
pub struct Logging {
    pub logger: slog::Logger,
}

pub static LOGGING: Lazy<Logging> = Lazy::new(|| {
    let pid=std::process::id().to_string();
    let ts = chrono::Local::now().timestamp();

    // let logfile = format!("./app-{}-{}.log", ts, pid);
    let logfile = format!("/peggy/data/json_log/app-{}-{}.log", ts, pid);
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(logfile)
        .unwrap();

    let drain = slog_json::Json::new(file)
        .set_pretty(false)
        .add_default_keys()
        .add_key_value(o!(
                "pid" => pid
                ))
        .build()
        .fuse();

    let module = PushFnValue(|r: &Record, ser: PushFnValueSerializer| {
        ser.emit(format_args!("{}", r.module()))
    });
    let location = PushFnValue(|r: &Record, ser: PushFnValueSerializer| {
        ser.emit(format_args!("https://github.com/nkmr-jp/gravity-bridge/blob/mylog/orchestrator/{}#L{}", r.file(), r.line()))
    });

    let applogger = Logger::root(
        Mutex::new(drain).fuse(),
        o!("module" => module,"location" => location,),
    );
    println!("json_logger initialized");
    Logging { logger: applogger }
});
