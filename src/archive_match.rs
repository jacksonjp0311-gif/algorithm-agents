use serde_json::{Value, json};

use crate::host::PublishedAlgorithm;
use crate::parse::contains_claim;

pub fn verdict_against_archive(
    title: &str,
    short_name: &str,
    core_idea: &str,
    published: &[PublishedAlgorithm],
) -> (String, Vec<Value>) {
    let mut verdict = "NEW";
    let mut matches = Vec::new();
    for item in published {
        if titles_match(title, &item.title)
            || titles_match(title, &item.short_name)
            || (!short_name.is_empty() && titles_match(short_name, &item.title))
        {
            verdict = "DUPLICATE";
            matches.push(json!({ "id": item.id, "archive_id": item.archive_id, "title": item.title, "kind": "title" }));
        } else if !title.is_empty()
            && (contains_claim(&item.summary, title) || contains_claim(&item.core_idea, title))
        {
            if verdict == "NEW" {
                verdict = "POSSIBLE_DUPLICATE";
            }
            matches.push(json!({ "id": item.id, "title": item.title, "kind": "summary" }));
        } else if !item.core_idea.is_empty()
            && !core_idea.is_empty()
            && contains_claim(&item.core_idea, core_idea)
        {
            if verdict == "NEW" {
                verdict = "VARIANT";
            }
            matches.push(json!({ "id": item.id, "title": item.title, "kind": "structure" }));
        }
    }
    (verdict.to_owned(), matches)
}

fn titles_match(a: &str, b: &str) -> bool {
    let a = a.trim().to_ascii_lowercase();
    let b = b.trim().to_ascii_lowercase();
    !a.is_empty() && a == b
}
