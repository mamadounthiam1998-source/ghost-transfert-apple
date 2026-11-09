//! Execution des outils externes (libimobiledevice). Jamais via un shell :
//! les arguments sont passes un par un, ce qui exclut l'injection de commandes.

use crate::{progress::OpCtl, GhostError, Result};
use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct Tools {
    pub dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Tools {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Tools { dirs }
    }

    fn candidates(name: &str) -> Vec<String> {
        if cfg!(windows) {
            vec![format!("{name}.exe"), name.to_string()]
        } else {
            vec![name.to_string()]
        }
    }

    /// Cherche d'abord dans les dossiers fournis (outils embarques), puis dans le PATH.
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        let mut dirs = self.dirs.clone();
        if let Some(path) = std::env::var_os("PATH") {
            dirs.extend(std::env::split_paths(&path));
        }
        for d in dirs {
            for c in Self::candidates(name) {
                let p = d.join(&c);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    }

    fn command(&self, name: &str, args: &[&str]) -> Result<Command> {
        let exe = self
            .find(name)
            .ok_or_else(|| GhostError::Tool(format!("`{name}` est introuvable (voir Diagnostic)")))?;
        let mut c = Command::new(exe);
        c.args(args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        Ok(c)
    }

    pub fn run(&self, name: &str, args: &[&str], timeout: Duration) -> Result<Output> {
        let mut cmd = self.command(name, args)?;
        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut so = child.stdout.take().unwrap();
        let mut se = child.stderr.take().unwrap();
        let h_out = thread::spawn(move || {
            let mut s = String::new();
            let _ = so.read_to_string(&mut s);
            s
        });
        let h_err = thread::spawn(move || {
            let mut s = String::new();
            let _ = se.read_to_string(&mut s);
            s
        });
        let start = Instant::now();
        let status = loop {
            if let Some(st) = child.try_wait()? {
                break st;
            }
            if start.elapsed() > timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(GhostError::Tool(format!("`{name}` a dépassé le délai imparti")));
            }
            thread::sleep(Duration::from_millis(25));
        };
        Ok(Output {
            code: status.code().unwrap_or(-1),
            stdout: h_out.join().unwrap_or_default(),
            stderr: h_err.join().unwrap_or_default(),
        })
    }

    /// Execute un outil en lisant sa sortie au fil de l'eau (separateurs \r ou \n).
    /// L'annulation tue le processus. Retourne (code, stderr).
    pub fn run_streaming(
        &self,
        name: &str,
        args: &[&str],
        ctl: &OpCtl,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<(i32, String)> {
        let mut cmd = self.command(name, args)?;
        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut so = child.stdout.take().unwrap();
        let mut se = child.stderr.take().unwrap();
        let h_err = thread::spawn(move || {
            let mut s = String::new();
            let _ = se.read_to_string(&mut s);
            s
        });
        let child = Arc::new(Mutex::new(child));
        let done = Arc::new(AtomicBool::new(false));
        let watcher = {
            let (child, done, ctl) = (child.clone(), done.clone(), ctl.clone());
            thread::spawn(move || {
                while !done.load(Ordering::SeqCst) {
                    if ctl.is_cancelled() {
                        if let Ok(mut c) = child.lock() {
                            let _ = c.kill();
                        }
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            })
        };

        let mut buf = [0u8; 4096];
        let mut line = Vec::new();
        loop {
            let n = so.read(&mut buf)?;
            if n == 0 {
                break;
            }
            for &b in &buf[..n] {
                if b == b'\n' || b == b'\r' {
                    if !line.is_empty() {
                        on_line(&String::from_utf8_lossy(&line));
                        line.clear();
                    }
                } else {
                    line.push(b);
                }
            }
        }
        if !line.is_empty() {
            on_line(&String::from_utf8_lossy(&line));
        }
        let status = child.lock().unwrap().wait()?;
        done.store(true, Ordering::SeqCst);
        let _ = watcher.join();
        let stderr = h_err.join().unwrap_or_default();
        ctl.check()?;
        Ok((status.code().unwrap_or(-1), stderr))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn run_captures_output_without_shell() {
        let t = Tools::default();
        // Un argument contenant des metacaracteres est passe tel quel, jamais interprete.
        let o = t.run("echo", &["a; touch /tmp/ghost_pwned"], Duration::from_secs(5)).unwrap();
        assert_eq!(o.code, 0);
        assert!(o.stdout.contains("a; touch /tmp/ghost_pwned"));
        assert!(!std::path::Path::new("/tmp/ghost_pwned").exists());
    }

    #[test]
    fn missing_tool_is_an_error() {
        assert!(Tools::default().run("outil_inexistant_xyz", &[], Duration::from_secs(1)).is_err());
    }

    #[test]
    fn timeout_kills_process() {
        let t = Tools::default();
        let r = t.run("sleep", &["5"], Duration::from_millis(200));
        assert!(r.is_err());
    }

    #[test]
    fn streaming_splits_on_cr_and_lf() {
        let t = Tools::default();
        let mut lines = vec![];
        let ctl = OpCtl::new();
        let (code, _) = t
            .run_streaming("printf", &["10%%\\r20%%\\n30%%"], &ctl, &mut |l| lines.push(l.to_string()))
            .unwrap();
        assert_eq!(code, 0);
        assert_eq!(lines, vec!["10%", "20%", "30%"]);
    }

    #[test]
    fn streaming_can_be_cancelled() {
        let t = Tools::default();
        let ctl = OpCtl::new();
        let c2 = ctl.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            c2.cancel();
        });
        let r = t.run_streaming("sleep", &["10"], &ctl, &mut |_| {});
        assert!(matches!(r, Err(GhostError::Cancelled)));
    }
}
