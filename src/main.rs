#![cfg(unix)]
mod process;
mod watch;
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::os::unix::process::CommandExt;
use std::{
    io::{self, BufRead, IsTerminal, Write},
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

#[derive(Parser, Debug)]
#[command(
    version,
    about = "TrpHot — watch, build, restart. No flags opens the OpenTUI spectator."
)]
struct Args {
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Enable automatic builds (otherwise spectator mode)
    #[arg(long)]
    run: bool,
    /// Make target; repeat for multiple targets
    #[arg(long)]
    target: Vec<String>,
    #[arg(long)]
    makefile: Option<String>,
    /// Shell command to restart after successful builds
    #[arg(long = "exec")]
    exec_cmd: Option<String>,
    #[arg(long)]
    ignore: Vec<String>,
    #[arg(long)]
    interval: Option<u64>,
    #[arg(long)]
    debounce: Option<u64>,
    /// Use the TUI even with --run
    #[arg(long)]
    tui: bool,
    /// Watch without the TUI
    #[arg(long)]
    headless: bool,
    /// Build once; optionally run the executable until it exits
    #[arg(long)]
    once: bool,
    #[arg(long, hide = true)]
    engine: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    targets: Vec<String>,
    makefile: String,
    exec: String,
    ignores: Vec<String>,
    interval: u64,
    debounce: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            targets: vec!["all".into()],
            makefile: "Makefile".into(),
            exec: String::new(),
            ignores: vec![
                "**/*.o".into(),
                "**/*.a".into(),
                "**/*.so".into(),
                "**/*.swp".into(),
                "**/*~".into(),
                "build/**".into(),
                "bin/**".into(),
            ],
            interval: 300,
            debounce: 200,
        }
    }
}
impl Config {
    fn validate(&self) -> Result<(), String> {
        if !(50..=60000).contains(&self.interval) {
            return Err("interval must be 50..60000 ms".into());
        }
        if self.debounce > 60000 {
            return Err("debounce must be <=60000 ms".into());
        }
        if self.targets.is_empty()
            || self
                .targets
                .iter()
                .any(|x| x.is_empty() || x.starts_with('-') || x.contains('='))
        {
            return Err("use nonempty Make target names, not options or assignments".into());
        }
        watch::matcher(&self.ignores)?;
        Ok(())
    }
}
fn emit(protocol: bool, value: Value) {
    if protocol {
        println!("{value}");
    } else {
        match value["type"].as_str().unwrap_or("") {
            "log" => println!(
                "[{}] {}",
                value["source"].as_str().unwrap_or(""),
                value["text"].as_str().unwrap_or("")
            ),
            "change" => println!(
                "[{}] {}",
                value["kind"].as_str().unwrap_or(""),
                value["path"].as_str().unwrap_or("")
            ),
            "status" => println!("[trphot] {}", value["message"].as_str().unwrap_or("")),
            _ => {}
        }
    }
    let _ = io::stdout().flush();
}
fn main() {
    if let Err(e) = start() {
        eprintln!("trphot: {e}");
        std::process::exit(1);
    }
}
fn start() -> Result<(), String> {
    let args = Args::parse();
    if !args.engine && !args.headless && !args.once && (!args.run || args.tui) {
        if !io::stdin().is_terminal() {
            return Err("TUI needs a terminal. Use --headless for spectator output or --run for automatic builds.".into());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let installed = exe.parent().unwrap().join("../share/trphot");
        let local = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let base = std::env::var_os("TRPHOT_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                if installed.join("ui/main.ts").exists() {
                    installed
                } else {
                    local
                }
            });
        let bun = if base.join("node_modules/.bin/bun").exists() {
            base.join("node_modules/.bin/bun")
        } else {
            PathBuf::from("bun")
        };
        let err = Command::new(bun)
            .arg("run")
            .arg(base.join("ui/main.ts"))
            .args(std::env::args_os().skip(1))
            .env("TRPHOT_ENGINE", exe)
            .exec();
        return Err(format!(
            "cannot launch OpenTUI: {err}. Run ./install.sh first, or use --headless."
        ));
    }
    let root = args
        .dir
        .canonicalize()
        .map_err(|e| format!("project directory: {e}"))?;
    if !root.is_dir() {
        return Err("project path is not a directory".into());
    }
    let cfg_path = root.join(".trphot.json");
    let mut cfg: Config = if cfg_path.exists() {
        serde_json::from_slice(&std::fs::read(&cfg_path).map_err(|e| e.to_string())?)
            .map_err(|e| format!(".trphot.json: {e}"))?
    } else {
        Config::default()
    };
    if !args.target.is_empty() {
        cfg.targets = args.target;
    }
    if let Some(v) = args.makefile {
        cfg.makefile = v;
    }
    if let Some(v) = args.exec_cmd {
        cfg.exec = v;
    }
    cfg.ignores.extend(args.ignore);
    if let Some(v) = args.interval {
        cfg.interval = v;
    }
    if let Some(v) = args.debounce {
        cfg.debounce = v;
    }
    cfg.validate()?;
    let stopped = Arc::new(AtomicBool::new(false));
    for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        signal_hook::flag::register(sig, stopped.clone()).map_err(|e| e.to_string())?;
    }
    let (tx, rx) = mpsc::sync_channel::<Value>(1024);
    if args.engine {
        let t = tx.clone();
        std::thread::spawn(move || {
            for line in io::stdin().lock().lines() {
                match line {
                    Ok(s) => {
                        let v = serde_json::from_str(&s).unwrap_or(json!({"cmd":"invalid"}));
                        if t.send(v).is_err() {
                            return;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = t.send(json!({"cmd":"quit"}));
        });
    }
    let protocol = args.engine;
    let mut active = args.run || args.once;
    let mut paused = false;
    let mut previous = watch::scan(&root, &cfg)?;
    emit(
        protocol,
        json!({"type":"ready","root":root,"config":cfg,"active":active,"files":watch::files(&previous)}),
    );
    let mut build: Option<process::Job> = None;
    let mut app: Option<process::Job> = None;
    let mut pending = active;
    let mut due = Instant::now();
    let mut scan_at = Instant::now();
    emit(
        protocol,
        json!({"type":"status","message":if active{"Automatic builds enabled"}else{"Spectating — no commands execute until armed"},"active":active,"paused":paused}),
    );
    while !stopped.load(Ordering::Relaxed) {
        for _ in 0..512 {
            let v = match rx.try_recv() {
                Ok(v) => v,
                Err(_) => break,
            };
            if v["type"] == "log" {
                emit(protocol, v);
                continue;
            }
            match v["cmd"].as_str().unwrap_or("") {
                "quit" => {
                    stopped.store(true, Ordering::Relaxed);
                }
                "arm" => {
                    active = !active;
                    pending = active;
                    due = Instant::now();
                    emit(
                        protocol,
                        json!({"type":"status","message":if active{"Automatic builds enabled"}else{"Spectator mode enabled"},"active":active,"paused":paused}),
                    );
                }
                "pause" => {
                    paused = !paused;
                    emit(
                        protocol,
                        json!({"type":"status","message":if paused{"Build scheduling paused"}else{"Build scheduling resumed"},"active":active,"paused":paused}),
                    );
                }
                "build" => {
                    pending = true;
                    due = Instant::now();
                }
                "stop" => {
                    if let Some(mut j) = app.take() {
                        j.stop();
                    }
                    emit(
                        protocol,
                        json!({"type":"status","message":"Application stopped"}),
                    );
                }
                "config" => {
                    let result = serde_json::from_value::<Config>(v["config"].clone())
                        .map_err(|e| e.to_string())
                        .and_then(|c| {
                            c.validate()?;
                            Ok(c)
                        });
                    match result {
                        Ok(c) => match watch::scan(&root, &c) {
                            Ok(s) => {
                                cfg = c;
                                previous = s;
                                emit(
                                    protocol,
                                    json!({"type":"config","config":cfg,"files":watch::files(&previous)}),
                                );
                            }
                            Err(e) => emit(protocol, json!({"type":"status","message":e})),
                        },
                        Err(e) => emit(protocol, json!({"type":"status","message":e})),
                    }
                }
                "save" => {
                    let result = (|| -> Result<(), String> {
                        let data = serde_json::to_vec_pretty(&cfg).map_err(|e| e.to_string())?;
                        let temp = root.join(".trphot.json.tmp");
                        std::fs::write(&temp, data).map_err(|e| e.to_string())?;
                        std::fs::rename(temp, &cfg_path).map_err(|e| e.to_string())
                    })();
                    emit(
                        protocol,
                        json!({"type":"status","message":match result{Ok(_)=>"Saved .trphot.json".to_owned(),Err(e)=>format!("Save failed: {e}")}}),
                    );
                }
                _ => emit(
                    protocol,
                    json!({"type":"status","message":"Unknown engine command"}),
                ),
            }
        }
        if stopped.load(Ordering::Relaxed) {
            break;
        }
        if !args.once && scan_at.elapsed() >= Duration::from_millis(cfg.interval) {
            scan_at = Instant::now();
            match watch::scan(&root, &cfg) {
                Ok(next) => {
                    let changes = watch::changes(&previous, &next);
                    if !changes.is_empty() {
                        for (kind, path) in changes {
                            emit(protocol, json!({"type":"change","kind":kind,"path":path}));
                        }
                        if active {
                            pending = true;
                            due = Instant::now() + Duration::from_millis(cfg.debounce);
                        }
                    }
                    if next != previous {
                        emit(
                            protocol,
                            json!({"type":"files","files":watch::files(&next)}),
                        );
                    }
                    previous = next;
                }
                Err(e) => emit(protocol, json!({"type":"status","message":e})),
            }
        }
        if let Some(j) = build.as_mut() {
            if let Some(status) = j.poll()? {
                build = None;
                emit(
                    protocol,
                    json!({"type":"status","message":format!("Build {}",status),"building":false,"success":status.success()}),
                );
                if status.success() {
                    if !cfg.exec.trim().is_empty() {
                        if let Some(mut j) = app.take() {
                            j.stop();
                        }
                        app = Some(process::Job::app(&root, &cfg.exec, &tx)?);
                        emit(
                            protocol,
                            json!({"type":"status","message":"Application started"}),
                        );
                    } else if args.once {
                        break;
                    }
                } else if args.once {
                    return Err(format!("build failed: {status}"));
                }
            }
        }
        if let Some(j) = app.as_mut() {
            if let Some(status) = j.poll()? {
                app = None;
                emit(
                    protocol,
                    json!({"type":"status","message":format!("Application {status}")}),
                );
                if args.once {
                    if !status.success() {
                        return Err(format!("application failed: {status}"));
                    }
                    break;
                }
            }
        }
        if pending && !paused && build.is_none() && Instant::now() >= due {
            pending = false;
            emit(
                protocol,
                json!({"type":"status","message":format!("Building: make -f {} {}",cfg.makefile,cfg.targets.join(" ")),"building":true}),
            );
            match process::Job::build(&root, &cfg, &tx) {
                Ok(j) => build = Some(j),
                Err(e) => {
                    emit(
                        protocol,
                        json!({"type":"status","message":e,"building":false,"success":false}),
                    );
                    if args.once {
                        return Err(e);
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(build);
    drop(app);
    Ok(())
}
