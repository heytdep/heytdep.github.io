use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::str;

fn extract_numeric_prefix(filename: &str) -> Option<i32> {
    let parts: Vec<&str> = filename.split("--").collect();
    if let Some(first_part) = parts.get(0) {
        let numeric_part = first_part.trim_matches(|c: char| !c.is_numeric());
        numeric_part.parse::<i32>().ok()
    } else {
        None
    }
}

fn make_slug(title: &str) -> String {
    title
        .to_lowercase()
        .replace(" ", "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

fn get_drafts() -> Vec<PathBuf> {
    let mut paths = fs::read_dir("./drafts")
        .expect("Failed to read directory")
        .map(|entry| entry.expect("Failed to read entry").path())
        .collect::<Vec<PathBuf>>();

    paths.sort_by(|a, b| {
        let a_filename = a.file_name().unwrap().to_str().unwrap();
        let b_filename = b.file_name().unwrap().to_str().unwrap();

        if let (Some(a_num), Some(b_num)) = (
            extract_numeric_prefix(a_filename),
            extract_numeric_prefix(b_filename),
        ) {
            a_num.cmp(&b_num)
        } else {
            a_filename.cmp(b_filename)
        }
    });

    paths
}

fn preemptive_remove() {
    for dir in &["./writeup/", "./research/"] {
        let mut child = Command::new("rm")
            .arg("-rf")
            .arg(dir)
            .spawn()
            .expect("failed to remove dir");
        let _result = child.wait().unwrap();

        let mut child1 = Command::new("mkdir")
            .arg("-p")
            .arg(dir)
            .spawn()
            .expect("failed to create dir");
        let _result = child1.wait().unwrap();
    }
}

fn build_handler(out_dir: &str, slug: &str, title: &str, is_research: bool) {
    let label = if is_research {
        r#"<p class="doc-tag">long-form research</p>"#
    } else {
        ""
    };

    let parent = format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.6.0/styles/idea.min.css">
<link href="/index.css" rel="stylesheet">
<script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.6.0/highlight.min.js"></script>
<!-- Cloudflare Web Analytics --><script defer src='https://static.cloudflareinsights.com/beacon.min.js' data-cf-beacon='{{"token": "e9adb517193447a3a9c4d5064ffa2550"}}'></script><!-- End Cloudflare Web Analytics -->
<script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.6.0/languages/rust.min.js"></script>
<script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.6.0/languages/javascript.min.js"></script>
<script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.6.0/languages/python.min.js"></script>
<script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.6.0/languages/bash.min.js"></script>
<script>
window.MathJax = {{
    startup: {{
        typeset: false
    }}
}};
</script>
<script src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js"></script>
<script>
fetch("./content.html").then(resp => resp.text().then(content => {{
    const post = document.createElement("div");
    post.innerHTML = content;
    document.getElementById("post").appendChild(post);
    hljs.highlightAll();
    if (window.MathJax && MathJax.typesetPromise) {{
        MathJax.typesetPromise();
    }}
}}));
</script>
<style>
code {{
    background-color: #e4e4e4;
    padding: 2px;
    border-radius: 5px;
    max-height: 800px;
}}
#post {{
    max-width: 720px;
    font-family: Georgia, 'Times New Roman', serif;
}}
#post p, #post ol, #post li {{
    font-family: Georgia, 'Times New Roman', serif;
    font-size: 14px;
    line-height: 1.65;
    color: #333;
}}
#post h1, #post h2, #post h3, #post h4 {{
    font-family: Georgia, 'Times New Roman', serif;
    font-weight: 700;
}}
#post h1 {{ font-size: 1.4rem; }}
#post h2 {{ font-size: 1.2rem; }}
#post h3 {{ font-size: 1.05rem; }}
#post blockquote p {{
    font-size: 13px;
}}
.doc-tag {{
    font-size: 0.7rem;
    color: #999;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    margin-bottom: 0.25rem;
}}
.title {{
    font-size: 1.5rem;
    font-family: Georgia, 'Times New Roman', serif;
}}
</style>
</head>
<body>
<div id="post">
<h4><a href="/">back</a></h4>
{}
<h1 class="title">{}</h1>
</div>
</body>
</html>"#,
        label, title
    );

    let path = format!("./{}/{}/index.html", out_dir, slug);
    let mut file = File::create(&path).unwrap();
    file.write_all(parent.as_bytes());
}

fn to_html_pd(name: String) {
    let splitted: &Vec<&str> = &name.split(".").collect();

    if splitted.len() != 1 {
        let dir_name = splitted[0];
        let ext_name = splitted[1];
        let post_name_split: Vec<&str> = dir_name.split(")--").collect();

        if post_name_split.len() <= 1 {
            return;
        }

        let is_research = post_name_split[1] == "research";
        let title_raw = post_name_split[2];
        let slug = make_slug(title_raw);
        let out_dir = if is_research { "research" } else { "writeup" };

        let mut child1 = Command::new("mkdir")
            .arg("-p")
            .arg(format!("./{}/{}", out_dir, &slug))
            .spawn()
            .expect("failed to create dir");
        let _result1 = child1.wait().unwrap();

        let input_format = if ext_name == "tex" { "latex" } else { "markdown" };

        let _child = Command::new("pandoc")
            .arg("-f")
            .arg(input_format)
            .arg("-t")
            .arg("html")
            .arg("--mathjax")
            .arg(format!("./drafts/{}", name))
            .arg("-o")
            .arg(format!("./{}/{}/content.html", out_dir, &slug))
            .output()
            .expect("failed to run pandoc");

        let title = title_raw.replace("-", " ");
        build_handler(out_dir, &slug, &title, is_research);
    } else {
        // year marker, no output needed
    }
}

fn gen_index_content(draft_names: Vec<String>) -> String {
    let mut head = String::from(
        r#"<!DOCTYPE html>
<html>
<meta charset="UTF-8">
<head>
<link href="index.css" rel="stylesheet">
<!-- Cloudflare Web Analytics --><script defer src='https://static.cloudflareinsights.com/beacon.min.js' data-cf-beacon='{"token": "e9adb517193447a3a9c4d5064ffa2550"}'></script><!-- End Cloudflare Web Analytics -->
</head>
<style>
    * { margin: 0; padding: 0; box-sizing: border-box; }

    body {
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif;
        font-size: 14px;
        line-height: 1.5;
        color: #333;
        background: #fff;
        padding: 3rem 1.5rem;
    }

    .container {
        max-width: 700px;
        margin: 0 auto;
    }

    header {
        margin-bottom: 2.5rem;
    }

    header h1 {
        font-size: 1.1rem;
        font-weight: 600;
        margin-bottom: 0.2rem;
        border: none;
    }

    header .handle {
        font-size: 0.85rem;
        color: #888;
        margin-bottom: 0.6rem;
    }

    header .handle a { color: #888; }
    header .handle a:hover { color: #333; }

    header .bio {
        font-size: 0.85rem;
        color: #666;
        line-height: 1.5;
    }

    header .bio a { color: #555; text-decoration: underline; }

    .sections {
        display: flex;
        gap: 0.8rem;
        margin-bottom: 2rem;
        border-bottom: 1px solid #e5e5e5;
        padding-bottom: 0;
    }

    .sections a {
        font-size: 0.8rem;
        color: #999;
        text-decoration: none;
        padding-bottom: 0.5rem;
        letter-spacing: 0.03em;
        text-transform: uppercase;
        border-bottom: 1.5px solid transparent;
        transition: color 0.15s;
    }

    .sections a:hover { color: #333; }
    .sections a.active { color: #333; border-bottom-color: #333; }

    .section { display: none; }
    .section.active { display: block; }

    .year-label {
        font-size: 0.8rem;
        color: #aaa;
        font-weight: 600;
        margin: 1.5rem 0 0.5rem;
        letter-spacing: 0.02em;
    }

    .entry {
        padding: 0.4rem 0;
    }

    .entry a {
        font-size: 0.9rem;
        color: #333;
        text-decoration: none;
    }

    .entry a:hover { color: rgb(249, 15, 18); }

    .entry .date {
        font-size: 0.75rem;
        color: #bbb;
        margin-left: 0.4rem;
    }

    .entry .tag {
        font-size: 0.65rem;
        color: #bbb;
        text-transform: uppercase;
        letter-spacing: 0.04em;
        margin-left: 0.3rem;
    }

    .research-entry {
        padding: 0.6rem 0;
        border-bottom: 1px solid #f5f5f5;
    }

    .research-entry a.title-link {
        font-size: 0.9rem;
        color: #333;
        text-decoration: none;
        display: block;
    }

    .research-entry a.title-link:hover { color: rgb(249, 15, 18); }

    .research-entry .snippet {
        font-size: 0.78rem;
        color: #999;
        line-height: 1.4;
        margin-top: 0.15rem;
    }

    .research-note {
        font-size: 0.78rem;
        color: #aaa;
        margin-bottom: 1.5rem;
        line-height: 1.4;
    }

    @media (max-width: 600px) {
        body { padding: 1.5rem 1rem; }
    }
</style>

<script>
function switchTab(tab) {
    document.querySelectorAll('.section').forEach(s => s.classList.remove('active'));
    document.querySelectorAll('.sections a').forEach(a => a.classList.remove('active'));
    document.getElementById(tab).classList.add('active');
    document.querySelector('[data-tab="' + tab + '"]').classList.add('active');
}
</script>

"#,
    );

    let mut writeup_entries = Vec::new();
    let mut research_entries = Vec::new();

    let drafts: Vec<String> = draft_names.into_iter().rev().collect();
    for draft in &drafts {
        let post_name_split: Vec<&str> = draft.split(")--").collect();

        if post_name_split.len() > 1 {
            let is_research = post_name_split[1] == "research";
            let title_raw = post_name_split[2];
            let slug = make_slug(title_raw);
            let dis_name = title_raw.replace("-", " ");

            if is_research {
                let content_bytes = fs::read(format!("./drafts/{}.md", draft)).unwrap_or(vec![]);
                let full_string = String::from_utf8(content_bytes).unwrap_or_default();
                let snippet: String = full_string
                    .lines()
                    .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(160)
                    .collect();

                research_entries.push(format!(
                    r#"<div class="research-entry">
<a class="title-link" href="./research/{slug}/">{dis_name}</a>
<div class="snippet">{snippet}...</div>
</div>"#
                ));
            } else {
                let content = fs::read(format!("./drafts/{}.md", draft)).unwrap_or(vec![]);
                let subtitle = content.split_at_checked(200).unwrap_or((&[], &[])).0;
                let sub_string = String::from_utf8(subtitle.to_vec()).unwrap_or_default();

                let date: &str = {
                    let split = sub_string.split('`').collect::<Vec<&str>>();
                    split.get(1).map_or("", |v| v)
                };

                let tag = if post_name_split[1] == "priv" { "notes" } else { "" };

                writeup_entries.push(format!(
                    r#"<div class="entry"><a href="./writeup/{slug}/">{dis_name}</a><span class="date">{date}</span>{}</div>"#,
                    if !tag.is_empty() { format!(r#"<span class="tag">{tag}</span>"#) } else { String::new() }
                ));
            }
        } else {
            let year = &draft.split("--").collect::<Vec<&str>>()[1];
            writeup_entries.push(format!(
                r#"<div class="year-label">{year}</div>"#
            ));
        }
    }

    let body = format!(
        r#"
<body>
<div class="container">
    <header>
        <h1>Tommaso De Ponti</h1>
        <div class="handle"><a href="https://x.com/heytdep" target="_blank">@heytdep</a></div>
        <div class="bio">founding eng/research <a href="https://x.com/tplus_cx">@tplus_cx</a>, co-founder <a href="https://x.com/xyclooLabs">@xyclooLabs</a>, trusted (?) execution</div>
    </header>

    <nav class="sections">
        <a href="javascript:switchTab('writeups')" data-tab="writeups" class="active">writeups</a>
        <a href="javascript:switchTab('research')" data-tab="research">research</a>
    </nav>

    <div id="writeups" class="section active">
        {}
    </div>

    <div id="research" class="section">
        <p class="research-note">not peer-reviewed research. long-form explorations and loosely structured thinking on topics I find interesting.</p>
        {}
    </div>
</div>
</body>"#,
        writeup_entries.join("\n        "),
        research_entries.join("\n        ")
    );

    head.push_str(&body);
    head
}

fn write_index(draft_names: Vec<String>) {
    let content = gen_index_content(draft_names);
    fs::write("./index.html", content);
    println!("[+] Successfully built index");
}

fn main() {
    preemptive_remove();
    let drafts = get_drafts();

    let names: Vec<String> = drafts
        .iter()
        .filter_map(|entry| {
            entry
                .as_path()
                .file_name()
                .and_then(|n| n.to_str().map(|s| String::from(s)))
        })
        .collect();

    let mut draft_basenames: Vec<String> = Vec::new();

    for name in &names {
        println!("{:?}", name);
        to_html_pd(name.clone());

        let base = name.split(".").next().unwrap_or(name);
        draft_basenames.push(base.to_string());
    }

    write_index(draft_basenames);
}
