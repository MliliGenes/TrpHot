use crate::Config;
use serde_json::{json, Value};
use std::os::unix::process::CommandExt;
use std::{
    io::{BufRead, BufReader, Read},
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc::SyncSender,
    time::{Duration, Instant},
};
pub struct Job {
    child: Child,
    group: i32,
}
fn pipe<R: Read + Send + 'static>(r: R, source: String, tx: SyncSender<Value>) {
    std::thread::spawn(move || {
        let mut r = BufReader::new(r);
        loop {
            let mut b = vec![];
            match r.by_ref().take(16384).read_until(b'\n', &mut b) {
                Ok(0) | Err(_) => break,
                _ => {
                    let text = String::from_utf8_lossy(&b).trim_end().to_string();
                    if tx
                        .send(json!({"type":"log","source":source,"text":text}))
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    });
}
impl Job {
    fn spawn(mut c: Command, source: &str, tx: &SyncSender<Value>) -> Result<Self, String> {
        c.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = c
            .spawn()
            .map_err(|e| format!("Cannot start {source}: {e}"))?;
        let group = child.id() as i32;
        pipe(child.stdout.take().unwrap(), source.into(), tx.clone());
        pipe(
            child.stderr.take().unwrap(),
            format!("{source}:err"),
            tx.clone(),
        );
        Ok(Self { child, group })
    }
    pub fn build(root: &Path, cfg: &Config, tx: &SyncSender<Value>) -> Result<Self, String> {
        let mut c = Command::new("make");
        c.current_dir(root)
            .arg("-f")
            .arg(&cfg.makefile)
            .arg("--")
            .args(&cfg.targets);
        Self::spawn(c, "make", tx)
    }
    pub fn app(root: &Path, cmd: &str, tx: &SyncSender<Value>) -> Result<Self, String> {
        let mut c = Command::new("sh");
        c.current_dir(root).arg("-c").arg(cmd);
        Self::spawn(c, "app", tx)
    }
    pub fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
        self.child.try_wait().map_err(|e| e.to_string())
    }
    pub fn stop(&mut self) {
        if self.group == 0 {
            return;
        }
        unsafe {
            libc::kill(-self.group, libc::SIGTERM);
        }
        let end = Instant::now() + Duration::from_millis(500);
        while Instant::now() < end {
            if unsafe { libc::kill(-self.group, 0) } != 0 {
                break;
            }
            let _ = self.child.try_wait();
            std::thread::sleep(Duration::from_millis(10));
        }
        unsafe {
            libc::kill(-self.group, libc::SIGKILL);
        }
        let _ = self.child.wait();
        self.group = 0;
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.stop();
    }
}
