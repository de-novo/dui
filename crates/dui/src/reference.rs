use crate::{
    analysis::store,
    chromium::{Browser, executable},
    replay::{Error, Job, io, read},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Control {
    context: String,
    session: String,
    baseline: Value,
}

impl Control {
    fn open(browser: &mut Browser, job: &Job, image: &Path) -> Result<Self, Error> {
        let mut spec = job.spec.clone();
        spec["assertions"] = json!([]);
        let (context, session, baseline) = browser
            .open_page(&spec, &json!({}), job.width, image)
            .map_err(io)?;
        Ok(Self {
            context,
            session,
            baseline,
        })
    }

    fn measure(&self, browser: &mut Browser, job: &Job) -> Value {
        let result =
            browser.reference(&self.session, &json!(job.inputs), &self.baseline, &job.spec);
        let mut result =
            result.unwrap_or_else(|err| json!({"reason":err.to_string(),"restore":false}));
        let base = &self.baseline;
        result["control"] = json!({
            "semantic":base["semantic"],"settled":base["settled"],"ready":base["ready"],"width":base["width"],
            "path":base["path"],"responseChecks":base["responseChecks"],"responses":base["responses"],
        });
        result["state"] = job.cell["state"].clone();
        result["widthId"] = json!(job.width);
        result
    }
}

fn group(
    browser: &mut Browser,
    jobs: &[Job],
    indices: &[usize],
    folder: &Path,
    image: &Path,
) -> Result<(), Error> {
    let mut page: Option<Control> = None;
    for &index in indices {
        let job = &jobs[index];
        if page.is_none() {
            page = Some(Control::open(browser, job, image)?);
        }
        let control = page.as_ref().ok_or_else(|| io("missing control page"))?;
        let result = control.measure(browser, job);
        store(&folder.join(format!("{index}.json")), &result)?;
        if result["restore"] != true {
            browser.close_control(&control.context).map_err(io)?;
            page = None;
        }
    }
    if let Some(page) = page {
        browser.close_control(&page.context).map_err(io)?;
    }
    Ok(())
}

pub(crate) fn collect(jobs: &[Job], out: &Path, port: u16) -> Result<(), Error> {
    let folder = out.join("reference");
    fs::create_dir(&folder).map_err(io)?;
    let mut groups: BTreeMap<(String, u32), Vec<usize>> = BTreeMap::new();
    for (i, job) in jobs.iter().enumerate() {
        if job
            .inputs
            .keys()
            .any(|id| !job.checks().iter().any(|check| check["slot"] == *id))
        {
            groups
                .entry((job.scenario().into(), job.width))
                .or_default()
                .push(i);
        }
    }
    // Periodically throw away even fully restored controls to bound page state and memory.
    let groups: Vec<_> = groups
        .values()
        .flat_map(|indices| indices.chunks(24))
        .collect();
    let cursor = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let mut tasks = Vec::new();
        for _ in 0..groups.len().min(2) {
            tasks.push(scope.spawn(|| -> Result<(), Error> {
                let profile = tempfile::tempdir().map_err(io)?;
                let image = profile.path().join("control.png");
                let mut browser =
                    Browser::local(&executable(), profile.path(), port).map_err(io)?;
                while let Some(indices) = groups.get(cursor.fetch_add(1, Ordering::Relaxed)) {
                    group(&mut browser, jobs, indices, &folder, &image)?;
                }
                Ok(())
            }));
        }
        for task in tasks {
            task.join().map_err(|_| io("reference worker panic"))??;
        }
        Ok(())
    })
}

pub(crate) fn load(path: &Path) -> Result<Option<Value>, Error> {
    if path.is_file() {
        Ok(Some(read(path)?))
    } else {
        Ok(None)
    }
}
