use crate::Config;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path, time::SystemTime};
#[derive(Clone, PartialEq, Eq)]
pub struct Entry {
    pub size: u64,
    pub modified: SystemTime,
    pub ignored: bool,
}
pub type Snapshot = BTreeMap<String, Entry>;
pub fn matcher(patterns: &[String]) -> Result<GlobSet, String> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        b.add(Glob::new(p).map_err(|e| format!("Invalid ignore glob {p}: {e}"))?);
    }
    b.build().map_err(|e| e.to_string())
}
pub fn scan(root: &Path, cfg: &Config) -> Result<Snapshot, String> {
    let mut out = Snapshot::new();
    let mut patterns = cfg.ignores.clone();
    // A directly executed project binary must not trigger itself after linking.
    if let Some(word) = cfg.exec.split_whitespace().next() {
        let word = word.trim_matches(['\'', '"']).trim_start_matches("./");
        if !word.is_empty() && !word.starts_with('/') {
            patterns.push(globset::escape(word));
        }
    }
    let rules = matcher(&patterns)?;
    fn walk(root: &Path, dir: &Path, rules: &GlobSet, out: &mut Snapshot) -> Result<(), String> {
        for item in fs::read_dir(dir).map_err(|e| format!("Cannot scan {}: {e}", dir.display()))? {
            let item = item.map_err(|e| e.to_string())?;
            let p = item.path();
            let name = item.file_name();
            let name = name.to_string_lossy();
            if [
                ".git",
                "node_modules",
                "target",
                ".trphot.json",
                ".trphot.json.tmp",
            ]
            .contains(&name.as_ref())
            {
                continue;
            }
            let ty = match item.file_type() {
                Ok(t) => t,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.to_string()),
            };
            if ty.is_symlink() {
                continue;
            }
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            if ty.is_dir() {
                if !rules.is_match(&rel) && !rules.is_match(format!("{rel}/")) {
                    walk(root, &p, rules, out)?;
                }
            } else if ty.is_file() {
                let m = match item.metadata() {
                    Ok(m) => m,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(e.to_string()),
                };
                out.insert(
                    rel.clone(),
                    Entry {
                        size: m.len(),
                        modified: m.modified().map_err(|e| e.to_string())?,
                        ignored: rules.is_match(rel),
                    },
                );
                if out.len() > 50000 {
                    return Err("More than 50,000 files: exclude large directories with --ignore 'directory/**'".into());
                }
            }
        }
        Ok(())
    }
    walk(root, root, &rules, &mut out)?;
    Ok(out)
}
pub fn files(s: &Snapshot) -> Vec<Value> {
    s.iter()
        .map(|(p, e)| json!({"path":p,"size":e.size,"ignored":e.ignored}))
        .collect()
}
pub fn changes(old: &Snapshot, new: &Snapshot) -> Vec<(&'static str, String)> {
    let mut v = vec![];
    for (p, e) in new {
        if e.ignored {
            continue;
        }
        match old.get(p) {
            None => v.push(("added", p.clone())),
            Some(prev) if prev != e => v.push(("modified", p.clone())),
            _ => {}
        }
    }
    for (p, e) in old {
        if !e.ignored && !new.contains_key(p) {
            v.push(("deleted", p.clone()));
        }
    }
    v
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diffs_include_delete_but_skip_ignored() {
        let mut a = Snapshot::new();
        a.insert(
            "gone.c".into(),
            Entry {
                size: 1,
                modified: SystemTime::UNIX_EPOCH,
                ignored: false,
            },
        );
        a.insert(
            "ignored.o".into(),
            Entry {
                size: 1,
                modified: SystemTime::UNIX_EPOCH,
                ignored: true,
            },
        );
        let mut b = Snapshot::new();
        b.insert(
            "new.c".into(),
            Entry {
                size: 2,
                modified: SystemTime::UNIX_EPOCH,
                ignored: false,
            },
        );
        assert_eq!(
            changes(&a, &b),
            vec![("added", "new.c".into()), ("deleted", "gone.c".into())]
        );
    }
    #[test]
    fn globs() {
        let m = matcher(&vec!["**/*.o".into(), "build/**".into()]).unwrap();
        assert!(m.is_match("main.o"));
        assert!(m.is_match("build/"));
        assert!(!m.is_match("src/main.c"));
    }
}
