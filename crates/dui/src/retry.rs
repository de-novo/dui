use crate::{
    analysis::store,
    chromium::{Browser, executable},
    replay::{Error, Job, io, read},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

pub(crate) fn eligible(job: &Job, first: &Value) -> bool {
    let observation = &first["observation"];
    let slots = &observation["slots"];
    observation["ready"] == true
        && observation["path"] == job.spec["path"]
        && observation["width"] == job.width
        && job.inputs.keys().all(|k| slots.get(k).is_some())
        && job.inputs.iter().any(|(k, v)| slots.get(k) != Some(v))
}

fn batch(
    browser: &mut Browser,
    jobs: &[Job],
    indices: &[usize],
    out: &Path,
    scratch: &Path,
) -> Result<(), Error> {
    let mut page = None;
    for &i in indices {
        let job = &jobs[i];
        if page.is_none() {
            let mut spec = job.spec.clone();
            spec["assertions"] = json!([]);
            page = Some(
                browser
                    .open_page(&spec, &json!({}), job.width, scratch)
                    .map_err(io)?,
            );
        }
        let (ctx, session, base) = page.as_ref().ok_or_else(|| io("missing retry context"))?;
        let img = tempfile::NamedTempFile::new_in(out).map_err(io)?;
        let captured = browser.capture_after_mount(
            session,
            &json!(job.inputs),
            base,
            &job.spec,
            job.width,
            img.path(),
        );
        let mut record = match captured {
            Ok((observation, restored)) => {
                let hash = Sha256::digest(fs::read(img.path()).map_err(io)?);
                let name = format!("{hash:x}-{}.png", job.width);
                img.persist(out.join(&name)).map_err(io)?;
                json!({"observation":observation,"restored":restored,"image":name})
            }
            Err(e) => json!({"error":e.to_string(),"restored":false}),
        };
        record["state"] = job.cell["state"].clone();
        record["width"] = json!(job.width);
        store(&out.join("after-mount").join(format!("{i}.json")), &record)?;
        if record["restored"] != true {
            browser.close_control(ctx).map_err(io)?;
            page = None;
        }
    }
    if let Some((ctx, ..)) = page {
        browser.close_control(&ctx).map_err(io)?;
    }
    Ok(())
}

pub(crate) fn run(jobs: &[Job], out: &Path, port: u16) -> Result<(), Error> {
    let mut partitions: BTreeMap<(String, u32), Vec<usize>> = BTreeMap::new();
    let dir = out.join("after-mount");
    fs::create_dir_all(&dir).map_err(io)?;
    for (i, job) in jobs.iter().enumerate() {
        let readback = out.join("captures").join(format!("{i}.json"));
        if !readback.is_file() {
            continue;
        }
        if let Some(record) = crate::reference::load(&dir.join(format!("{i}.json")))? {
            if record["state"] != job.cell["state"] || record["width"] != job.width {
                return Err(io("retry job identity mismatch"));
            }
            continue;
        }
        let original = read(&readback)?;
        if eligible(job, &original) {
            partitions
                .entry((job.scenario().into(), job.width))
                .or_default()
                .push(i);
        }
    }
    let work: Vec<_> = partitions.values().flat_map(|v| v.chunks(16)).collect();
    let cursor = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let mut workers = Vec::new();
        for _ in 0..work.len().min(2) {
            workers.push(scope.spawn(|| -> Result<(), Error> {
                let profile = tempfile::tempdir().map_err(io)?;
                let mut browser =
                    Browser::local(&executable(), profile.path(), port).map_err(io)?;
                let img = profile.path().join("control.png");
                while let Some(ids) = work.get(cursor.fetch_add(1, Ordering::Relaxed)) {
                    batch(&mut browser, jobs, ids, out, &img)?;
                }
                Ok(())
            }));
        }
        for w in workers {
            w.join().map_err(|_| io("post-mount worker panic"))??;
        }
        Ok(())
    })
}
