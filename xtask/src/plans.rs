use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

pub(crate) fn check(root: &Path) -> Result<()> {
    let mut paths = fs::read_dir(root.join("docs"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.retain(|path| {
        path.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("-plan.md"))
    });
    paths.sort();
    if paths.is_empty() {
        bail!("no docs/*-plan.md files found");
    }
    for path in &paths {
        validate(&fs::read_to_string(path)?)
            .with_context(|| format!("plan lifecycle: {}", path.display()))?;
    }
    println!("plan lifecycle passed: {} plans", paths.len());
    Ok(())
}

fn field<'a>(source: &'a str, prefix: &str) -> Result<&'a str> {
    let values: Vec<_> = source
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(str::trim)
        .collect();
    match values.as_slice() {
        [value] if !value.is_empty() => Ok(value),
        _ => bail!("expected exactly one non-empty {prefix} field"),
    }
}

fn delivery_reference(source: &str) -> Result<()> {
    let delivery = field(source, "Delivery:")?;
    let number = delivery
        .split_once("](https://github.com/robchristie/polyorama/pull/")
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(number, _)| number);
    if !number.is_some_and(|number| {
        !number.is_empty()
            && !number.starts_with('0')
            && number.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        bail!("Delivery must link to the owning Polyorama pull request");
    }
    Ok(())
}

fn validate(source: &str) -> Result<()> {
    match field(source, "Status:")? {
        "active" => {
            let next = field(source, "Next action:")?;
            if next.eq_ignore_ascii_case("none") || next.eq_ignore_ascii_case("none.") {
                bail!("active plan requires an actionable Next action");
            }
            if source
                .lines()
                .any(|line| line.starts_with("Landed commit:"))
            {
                bail!("active plan cannot declare terminal Landed commit");
            }
        }
        "complete" => {
            delivery_reference(source)?;
            if source
                .lines()
                .any(|line| line.starts_with("Landed commit:"))
            {
                let commit = field(source, "Landed commit:")?;
                let commit = commit.trim_matches('`');
                if commit.len() != 40
                    || !commit
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    bail!("Landed commit must be a full lowercase 40-character commit ID");
                }
            }
            let mut status_column = None;
            for line in source.lines() {
                let normalised = line.trim().to_ascii_lowercase();
                let heading = normalised.trim_start_matches('#').trim();
                if heading == "current phase"
                    || heading == "next action"
                    || normalised.starts_with("next action:")
                    || normalised.starts_with("- [ ]")
                    || normalised.starts_with("* [ ]")
                {
                    bail!("complete plan retains active state: {line}");
                }
                if !normalised.starts_with('|') {
                    status_column = None;
                    continue;
                }
                let cells: Vec<_> = normalised
                    .trim_matches('|')
                    .split('|')
                    .map(str::trim)
                    .collect();
                if let Some(column) = cells.iter().position(|cell| *cell == "status") {
                    status_column = Some(column);
                } else if let Some(column) = status_column {
                    let status = cells.get(column).copied().unwrap_or("");
                    let separator = !status.is_empty()
                        && status.bytes().all(|byte| byte == b'-' || byte == b':');
                    if !separator
                        && status != "landed"
                        && status != "complete"
                        && !status.starts_with("complete for ")
                    {
                        bail!("complete plan retains unfinished delivery status: {line}");
                    }
                }
            }
        }
        status => bail!("unknown Status: {status:?}; expected active or complete"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check, validate};

    const COMPLETE: &str = "Status: complete\nDelivery: [PR #28](https://github.com/robchristie/polyorama/pull/28)\nLanded commit: `0123456789abcdef0123456789abcdef01234567`\n";

    #[test]
    fn accepts_explicit_lifecycle_states_and_historical_evidence() {
        validate("Status: active\nNext action: Run the representative probe.\n").unwrap();
        validate(&format!(
            "{COMPLETE}\n## Closeout\nThe earlier review was pending before PR #28 landed.\n\
             | Increment | Status |\n| --- | --- |\n| 1 | Landed |\n\
             | 2 | Complete for one exact native environment |\n"
        ))
        .unwrap();
    }

    #[test]
    fn accepts_completed_candidate_before_its_merge_identity_exists() {
        validate("Status: complete\nDelivery: [PR #29](https://github.com/robchristie/polyorama/pull/29)\n").unwrap();
        for delivery in [
            "",
            "Delivery: PR pending",
            "Delivery: [PR](https://example.com/pull/29)",
            "Delivery: [PR](https://github.com/robchristie/polyorama/pull/pending)",
        ] {
            assert!(validate(&format!("Status: complete\n{delivery}\n")).is_err());
        }
    }

    #[test]
    fn rejects_ambiguous_missing_and_duplicate_lifecycle_fields() {
        for source in [
            "Status: terminal candidate; completes when PR #26 lands\n",
            "Status: deliverable candidate verified; exact-head review and landing remain\n",
            "Status: complete\n",
            "Status: active\nNext action: none.\n",
            "Status: active\n",
            "Next action: Review.\n",
        ] {
            assert!(validate(source).is_err(), "accepted {source}");
        }
        assert!(validate(&format!("{COMPLETE}Status: complete\n")).is_err());
        assert!(validate(&COMPLETE.replace("01234567`", "short`")).is_err());
        assert!(validate(&format!("{COMPLETE}Landed commit: duplicate\n")).is_err());
        assert!(validate(&COMPLETE.replace("complete", "active")).is_err());
    }

    #[test]
    fn rejects_obsolete_completed_plan_actions_and_delivery_rows() {
        for stale in [
            "## Current phase\nLanding remains.",
            "## Next action\nReview and land.",
            "Next action: none.",
            "- [ ] Reconcile final evidence",
            "* [ ] Reconcile final evidence",
            "| Increment | Status |\n| --- | --- |\n| 5 | In progress |",
            "| Increment | Status |\n| --- | --- |\n| 2 | Terminal candidate |",
        ] {
            assert!(
                validate(&format!("{COMPLETE}\n{stale}\n")).is_err(),
                "accepted {stale}"
            );
        }
    }

    #[test]
    fn discovers_repository_plans() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        check(root).unwrap();
    }

    use std::path::Path;
}
