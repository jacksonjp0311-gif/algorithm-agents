use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct SourceDoc {
    pub title: String,
    pub authors: Vec<String>,
    pub year: String,
    pub source_type: String,
    pub url: String,
    pub repository: String,
    pub license: String,
    pub domain: String,
    pub tags: Vec<String>,
    pub abstract_text: String,
    pub sections: Vec<(String, String)>,
    pub equations: Vec<String>,
    pub variables: Vec<String>,
    pub assumptions: Vec<String>,
    pub constraints: Vec<String>,
    pub failures: Vec<String>,
    pub complexity_time: String,
    pub complexity_space: String,
    pub complexity_origin: String,
    pub pseudocode: String,
    pub reference_code: String,
    pub reference_language: String,
    pub known_uses: Vec<String>,
    pub potential_uses: Vec<String>,
    pub algorithm_present: Option<bool>,
    pub ambiguous: bool,
    pub ambiguity_reason: String,
    pub citations: Vec<String>,
    pub motifs: Vec<String>,
    pub role: String,
    pub body: String,
    pub fields: BTreeMap<String, String>,
}

pub fn parse_source_document(text: &str) -> SourceDoc {
    if looks_labeled(text) {
        parse_labeled_document(text)
    } else {
        infer_document(text)
    }
}

pub fn looks_labeled(text: &str) -> bool {
    text.lines().take(40).any(|line| {
        matches!(
            line.split_once(':').map(|(key, _)| key.trim()),
            Some("TITLE" | "ALGORITHM" | "PSEUDOCODE" | "ABSTRACT" | "DOMAIN" | "EQUATION")
        )
    })
}

pub fn strip_markup(raw: &str) -> String {
    if looks_html(raw) {
        strip_html(raw)
    } else {
        raw.to_owned()
    }
}

pub fn clean_wikitext(raw: &str) -> String {
    let mut out = String::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("{{")
            || trimmed.starts_with("|")
            || trimmed.starts_with("}}")
            || trimmed.starts_with("[[File:")
            || trimmed.starts_with("[[Image:")
            || trimmed.starts_with("[[Category:")
        {
            continue;
        }
        let mut line = trimmed.to_owned();
        line = line.replace("'''", "").replace("''", "");
        while let Some(start) = line.find("[[") {
            if let Some(end) = line[start..].find("]]") {
                let inner = &line[start + 2..start + end];
                let shown = inner.split('|').next_back().unwrap_or(inner);
                line = format!("{}{}{}", &line[..start], shown, &line[start + end + 2..]);
            } else {
                break;
            }
        }
        if !line.is_empty() {
            out.push_str(&line);
            out.push('\n');
        }
    }
    clean_unstructured(&out)
}

pub fn clean_unstructured(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_chrome(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_chrome(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("skip to main")
        || lower.starts_with("search submit")
        || lower.contains("press enter to search")
        || lower == "log in"
        || lower == "donate"
        || lower.starts_with("advanced search")
}

pub fn infer_document(raw: &str) -> SourceDoc {
    let text = if looks_html(raw) {
        clean_unstructured(&strip_html(raw))
    } else if raw.contains("{{") || raw.contains("'''") {
        clean_wikitext(raw)
    } else {
        clean_unstructured(raw)
    };
    let mut doc = SourceDoc {
        body: text.clone(),
        complexity_origin: "UNKNOWN".into(),
        source_type: if looks_html(raw) { "html".into() } else { "document".into() },
        role: "primary".into(),
        ..SourceDoc::default()
    };
    doc.title = first_heading(&text).unwrap_or_else(|| "Untitled source".into());
    doc.abstract_text = first_paragraph(&text);
    doc.reference_code = collect_fences(&text);
    if doc.reference_code.is_empty() {
        doc.pseudocode = collect_indented_code(&text);
    } else {
        doc.pseudocode = doc.reference_code.clone();
        doc.reference_language = infer_language(&doc.reference_code);
    }
    doc.equations = text
        .lines()
        .map(str::trim)
        .filter(|line| {
            (line.contains('=') && line.len() < 160)
                && (line.contains("O(")
                    || line.contains("min")
                    || line.contains("max")
                    || line.contains("sum")
                    || line.starts_with("d[")
                    || line.contains("←")
                    || line.contains(":="))
        })
        .map(|line| line.to_owned())
        .take(12)
        .collect();
    let hay = text.to_ascii_lowercase();
    if doc.reference_code.is_empty() && doc.pseudocode.is_empty() {
        let hinted = text
            .lines()
            .filter(|line| {
                let t = line.trim_start();
                t.starts_with("def ")
                    || t.starts_with("fn ")
                    || t.starts_with("function ")
                    || t.starts_with("proc ")
                    || t.contains("while ")
                    || t.contains("for each")
            })
            .take(24)
            .collect::<Vec<_>>()
            .join("\n");
        if !hinted.is_empty() {
            if hinted.contains("def ") {
                doc.reference_language = "python".into();
            }
            doc.pseudocode = hinted;
        }
    }
    let looks_algorithm = hay.contains("algorithm")
        || hay.contains("pseudocode")
        || hay.contains("complexity")
        || hay.contains("def ")
        || hay.contains("fn ")
        || !doc.reference_code.is_empty()
        || !doc.pseudocode.is_empty();
    doc.algorithm_present = Some(looks_algorithm);
    doc.ambiguous = !looks_labeled(&text) && looks_algorithm;
    if doc.ambiguous {
        doc.ambiguity_reason =
            "inferred from unstructured source; fields are heuristic, not author-labeled".into();
    }
    if hay.contains("graph") {
        doc.domain = "Graph Algorithms".into();
    } else if hay.contains("sort") {
        doc.domain = "Sorting".into();
    } else if hay.contains("filter") || hay.contains("kalman") || hay.contains("particle") {
        doc.domain = "State Estimation".into();
    }
    doc
}

fn looks_html(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("<html") || lower.contains("<article") || lower.contains("<p>")
}

fn strip_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    let mut in_tag = false;
    let mut skip_until: Option<&str> = None;
    while !rest.is_empty() {
        let lower = rest.to_ascii_lowercase();
        if let Some(end) = skip_until {
            if let Some(idx) = lower.find(end) {
                rest = &rest[idx + end.len()..];
                skip_until = None;
                in_tag = false;
                continue;
            }
            break;
        }
        if !in_tag && (lower.starts_with("<script") || lower.starts_with("<style")) {
            skip_until = if lower.starts_with("<script") {
                Some("</script")
            } else {
                Some("</style")
            };
            continue;
        }
        let ch = rest.chars().next().unwrap();
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
            out.push('\n');
        } else if !in_tag {
            out.push(ch);
        }
        rest = &rest[ch.len_utf8()..];
    }
    decode_entities(&out)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn first_heading(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim().trim_start_matches('#').trim();
        if trimmed.starts_with("{{")
            || trimmed.starts_with("|")
            || trimmed.starts_with("}")
            || trimmed.starts_with("[[File:")
            || trimmed.starts_with("[[Image:")
        {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("'''") {
            let name = rest.split("'''").next().unwrap_or(rest).trim();
            if name.len() > 2 && name.len() < 160 {
                return Some(name.to_owned());
            }
        }
        if trimmed.len() > 3 && trimmed.len() < 160 && !trimmed.starts_with('{') && !is_chrome(trimmed)
        {
            return Some(trimmed.to_owned());
        }
    }
    None
}

fn first_paragraph(text: &str) -> String {
    let mut block = String::new();
    for line in text.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() && !block.is_empty() {
            break;
        }
        if trimmed.starts_with('#') || trimmed.starts_with("```") {
            if !block.is_empty() {
                break;
            }
            continue;
        }
        if !block.is_empty() {
            block.push(' ');
        }
        block.push_str(trimmed);
        if block.len() > 480 {
            break;
        }
    }
    block
}

fn collect_fences(text: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            if in_fence {
                break;
            }
            in_fence = true;
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_owned()
}

fn collect_indented_code(text: &str) -> String {
    text.lines()
        .filter(|line| line.starts_with("    ") || line.starts_with('\t'))
        .take(40)
        .collect::<Vec<_>>()
        .join("\n")
}

fn infer_language(code: &str) -> String {
    if code.contains("def ") || code.contains("import ") {
        "python".into()
    } else if code.contains("fn ") && code.contains("let ") {
        "rust".into()
    } else if code.contains("function ") || code.contains("const ") {
        "javascript".into()
    } else {
        String::new()
    }
}

pub fn parse_all_documents(text: &str) -> Vec<SourceDoc> {
    text.split("\n---EMERGENT---")
        .enumerate()
        .filter_map(|(index, chunk)| {
            let trimmed = chunk.trim();
            if trimmed.is_empty() {
                return None;
            }
            let mut doc = parse_source_document(trimmed);
            if index > 0 {
                doc.role = "emergent".into();
                if doc.algorithm_present.is_none() {
                    doc.algorithm_present = Some(true);
                }
            } else if doc.role.is_empty() {
                doc.role = "primary".into();
            }
            Some(doc)
        })
        .collect()
}

pub fn parse_labeled_document(text: &str) -> SourceDoc {
    let mut doc = SourceDoc {
        body: text.to_owned(),
        complexity_origin: "UNKNOWN".into(),
        source_type: "document".into(),
        role: "primary".into(),
        ..SourceDoc::default()
    };
    let mut current: Option<String> = None;
    let mut block = String::new();

    let flush = |doc: &mut SourceDoc, key: &str, block: &str| {
        let value = block.trim();
        if value.is_empty() {
            return;
        }
        doc.fields.insert(key.to_owned(), value.to_owned());
        match key {
            "TITLE" => doc.title = value.to_owned(),
            "AUTHORS" => doc.authors = lines(value),
            "YEAR" => doc.year = value.to_owned(),
            "TYPE" => doc.source_type = value.to_owned(),
            "URL" => doc.url = value.to_owned(),
            "REPOSITORY" => doc.repository = value.to_owned(),
            "LICENSE" => doc.license = value.to_owned(),
            "DOMAIN" => doc.domain = value.to_owned(),
            "TAGS" => doc.tags = lines(value),
            "ABSTRACT" => doc.abstract_text = value.to_owned(),
            "EQUATION" | "EQUATIONS" => doc.equations = lines(value),
            "VARIABLES" => doc.variables = lines(value),
            "ASSUMPTIONS" => doc.assumptions = lines(value),
            "CONSTRAINTS" => doc.constraints = lines(value),
            "FAILURES" | "FAILURE_MODES" => doc.failures = lines(value),
            "COMPLEXITY_TIME" => doc.complexity_time = value.to_owned(),
            "COMPLEXITY_SPACE" => doc.complexity_space = value.to_owned(),
            "COMPLEXITY_ORIGIN" => doc.complexity_origin = value.to_uppercase(),
            "PSEUDOCODE" => doc.pseudocode = value.to_owned(),
            "REFERENCE_CODE" => doc.reference_code = value.to_owned(),
            "REFERENCE_LANGUAGE" => doc.reference_language = value.to_owned(),
            "KNOWN_USES" => doc.known_uses = lines(value),
            "POTENTIAL_USES" => doc.potential_uses = lines(value),
            "ALGORITHM" => {
                doc.algorithm_present = Some(matches!(
                    value.to_ascii_lowercase().as_str(),
                    "yes" | "true" | "1"
                ));
            }
            "AMBIGUOUS" => {
                doc.ambiguous = matches!(value.to_ascii_lowercase().as_str(), "yes" | "true" | "1");
            }
            "AMBIGUITY" => doc.ambiguity_reason = value.to_owned(),
            "CITATIONS" => doc.citations = lines(value),
            "MOTIFS" => doc.motifs = lines(value),
            other if other.starts_with("SECTION_") => {
                let name = other.trim_start_matches("SECTION_").replace('_', " ");
                doc.sections.push((name.to_lowercase(), value.to_owned()));
            }
            _ => {}
        }
    };

    for line in text.lines() {
        if let Some((key, rest)) = line.split_once(':') {
            let key = key.trim();
            if key
                .chars()
                .all(|ch| ch.is_ascii_uppercase() || ch == '_' || ch.is_ascii_digit())
                && key.len() > 1
            {
                if let Some(prev) = current.take() {
                    flush(&mut doc, &prev, &block);
                    block.clear();
                }
                current = Some(key.to_owned());
                block.push_str(rest.trim());
                block.push('\n');
                continue;
            }
        }
        if current.is_some() {
            block.push_str(line);
            block.push('\n');
        }
    }
    if let Some(prev) = current.take() {
        flush(&mut doc, &prev, &block);
    }
    if doc.complexity_origin.is_empty() {
        doc.complexity_origin = "UNKNOWN".into();
    }
    doc
}

fn lines(value: &str) -> Vec<String> {
    value
        .lines()
        .map(|line| line.trim().trim_start_matches(['-', '*']).trim())
        .filter(|line| !line.is_empty())
        .map(|line| line.to_owned())
        .collect()
}

pub fn contains_claim(source: &str, claim: &str) -> bool {
    let hay = normalize(source);
    let needle = normalize(claim);
    !needle.is_empty() && hay.contains(&needle)
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ch.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

pub fn keyword_score(objective: &str, doc: &SourceDoc) -> u32 {
    let hay = format!(
        "{} {} {} {}",
        doc.title, doc.abstract_text, doc.domain, doc.body
    )
    .to_ascii_lowercase();
    objective
        .split_whitespace()
        .map(|word| word.trim_matches(|ch: char| !ch.is_alphanumeric()).to_ascii_lowercase())
        .filter(|word| word.len() > 3 && hay.contains(word.as_str()))
        .count() as u32
}
