use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::str;

fn rename_dir(from: &str, to: &str) {
    let mut child = Command::new("mv")
        .arg(format!("./post/{}", &from))
        .arg(format!("./post/{}", &to))
        .spawn()
        .expect("failed to compile");

    let _result = child.wait().unwrap();
}

fn extract_name_from_filename(filename: &str) -> &str {
    // Split the filename by "--" and take the second part
    let parts: Vec<&str> = filename.split("--").collect();

    parts[1]
}

fn extract_numeric_prefix(filename: &str) -> Option<i32> {
    let parts: Vec<&str> = filename.split("--").collect();
    if let Some(first_part) = parts.get(0) {
        let numeric_part = first_part.trim_matches(|c: char| !c.is_numeric());
        numeric_part.parse::<i32>().ok()
    } else {
        None
    }
}

fn get_drafts() -> Vec<PathBuf> {
    let mut paths = fs::read_dir("./drafts")
        .expect("Failed to read directory")
        .map(|entry| entry.expect("Failed to read entry").path())
        .collect::<Vec<PathBuf>>();

    // Sort the paths based on the numeric prefix in the filenames
    paths.sort_by(|a, b| {
        let a_filename = a.file_name().unwrap().to_str().unwrap();
        let b_filename = b.file_name().unwrap().to_str().unwrap();

        if let (Some(a_num), Some(b_num)) = (
            extract_numeric_prefix(a_filename),
            extract_numeric_prefix(b_filename),
        ) {
            a_num.cmp(&b_num)
        } else {
            // Fallback to comparing full filenames if numeric prefix extraction fails
            a_filename.cmp(b_filename)
        }
    });

    paths
}

fn get_comp() -> Vec<PathBuf> {
    let mut paths = fs::read_dir("./post")
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
            // Fallback to comparing full filenames if numeric prefix extraction fails
            a_filename.cmp(b_filename)
        }
    });

    paths
}

fn preemptive_remove() {
    let mut child = Command::new("rm")
        .arg("-rf")
        .arg("./post/")
        .spawn()
        .expect("failed to compile");
    let _result = child.wait().unwrap();

    let mut child1 = Command::new("mkdir")
        .arg("./post/")
        .spawn()
        .expect("failed to compile");
    let _result = child1.wait().unwrap();
}

fn build_handler(name: String) {
    let splitted: &Vec<&str> = &name.split(".").collect();

    if splitted.len() != 1 {
        let dir_name = splitted[0];
        let post_name_split: &Vec<&str> = &dir_name.split(")--").collect();
        let mut parent = format!(
            r#"
    <!DOCTYPE html>
    <html class="">
    <head>
    
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.6.0/styles/idea.min.css">
    <link href="../../index.css" rel="stylesheet">
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.6.0/highlight.min.js"></script>
    <!-- Cloudflare Web Analytics --><script defer src='https://static.cloudflareinsights.com/beacon.min.js' data-cf-beacon='{{"token": "e9adb517193447a3a9c4d5064ffa2550"}}'></script><!-- End Cloudflare Web Analytics -->
    <!-- and it's easy to individually load additional languages -->
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.6.0/languages/rust.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.6.0/languages/javascript.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.6.0/languages/python.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.6.0/languages/bash.min.js"></script>
    <script>
    fetch("./index.html").then(resp => resp.text().then(content => {{
        const post = document.createElement("div");
        post.innerHTML =content;
        document.getElementById("post").appendChild(post);
        hljs.highlightAll();
    }}));
    /*
    function resizeIframe(obj) {{
        obj.style.height = obj.contentWindow.document.documentElement.scrollHeight + 'px';
        //obj.contentWindow.document.body.style.color = "white"
        for (el of obj.contentWindow.document.getElementsByTagName("a")) {{
            el.style.color = "\#337AB7"
        }}
    }}
    */
    </script>
    <style>
    code {{
    background-color: #cdcdcd;
    padding: 2px;
    border-radius: 5px;
    max-height: 800px;
        }}
    </style>
    </head>
    <body>
    <div id="post">
    <h4><a href="/">see all posts</a></h1>
    <h1 class="title">{}</h1>
    <!--<iframe src="./index.html" frameborder="0" scrolling="no" width="100%" onload="resizeIframe(this)" />-->
    </div>
        <script>
    function resizeIframe(obj) {{
        obj.style.height = obj.contentWindow.document.documentElement.scrollHeight + 'px';
        obj.style.color = "\#fff"
    }}

    /*fetch("./index.html").then(resp => resp.text().then(content => {{
    const post = document.createElement("div");
    post.innerHTML = /<BODY.*?>([\s\S]*)<\/BODY>/.exec(content)[1];
    document.body.appendChild(post);
    }}));
    */

    </script>
    </body>
    </html>
    "#,
            post_name_split[2].replace("-", " ")
        );

        let mut file = File::create(format!("./post/{}/post.html", &dir_name)).unwrap();
        file.write_all(parent.as_bytes());
    }
}

fn to_html_pd(name: String) {
    let splitted: &Vec<&str> = &name.split(".").collect();

    if splitted.len() != 1 {
        let dir_name = splitted[0];
        let ext_name = splitted[1];
        let mut child1 = Command::new("mkdir")
            .arg(format!("./post/{}", &dir_name))
            .spawn()
            .expect("failed to compile");
        let _result1 = child1.wait().unwrap();

        if ext_name == "tex" {
            let mut child = Command::new("pandoc")
                .arg("-f")
                .arg("latex")
                .arg("-t")
                .arg("html")
                .arg(format!("./drafts/{}", name))
                .arg("-o")
                .arg(format!("./post/{}/index.html", &dir_name))
                .output()
                .expect("failed to compile");
            //    let result = child.wait().unwrap();
            //        let mut out = String::new();
            //        out.push_str(match str::from_utf8(&child.stdout) {
            //            Ok(val) => val,
            //            Err(_) => panic!("got non UTF-8 data from git"),
            //        });
        } else {
            let mut child = Command::new("pandoc")
                .arg("-f")
                .arg("markdown")
                .arg("-t")
                .arg("html")
                .arg(format!("./drafts/{}", name))
                .arg("-o")
                .arg(format!("./post/{}/index.html", &dir_name))
                .output()
                .expect("failed to compile");
            //    let result = child.wait().unwrap();
            //        let mut out = String::new();
            //        out.push_str(match str::from_utf8(&child.stdout) {
            //            Ok(val) => val,
            //            Err(_) => panic!("got non UTF-8 data from git"),
            //        });
        }
        build_handler(name);
    } else {
        let mut child1 = Command::new("mkdir")
            .arg(format!("./post/{}", &name))
            .spawn()
            .expect("failed to compile");
        let _result1 = child1.wait().unwrap();
    }

    //    fs::write(format!("./post/{}/index.html", &dir_name), out);
}

fn to_html(name: String) {
    let splitted: &Vec<&str> = &name.split(".").collect();
    let dir_name = splitted[0];
    let mut child = Command::new("latex2html")
        .arg(format!("./drafts/{}", name))
        .spawn()
        .expect("failed to compile");
    let _result = child.wait().unwrap();

    let mut child1 = Command::new("mv")
        .arg(format!("./drafts/{}", dir_name))
        .arg("./post")
        .spawn()
        .expect("failed to compile");
    let _result1 = child1.wait().unwrap();

    build_handler(name);
}

fn gen_index_content(mut posts: Vec<String>) -> String {
    let mut head = String::from(
        r#"<!DOCTYPE html>
<html class="">
<meta charset="UTF-8">
<head> 
<link href="index.css" rel="stylesheet">
<!-- Cloudflare Web Analytics --><script defer src='https://static.cloudflareinsights.com/beacon.min.js' data-cf-beacon='{{"token": "e9adb517193447a3a9c4d5064ffa2550"}}'></script><!-- End Cloudflare Web Analytics -->
</head>
<style>
:root {
        --primary-color: #2d2d2d;
        --secondary-color: #666;
        --accent-color: rgb(249, 15, 18);
        --background-color: #fff;
        --container-width: 800px;
    }

    * {
        margin: 0;
        padding: 0;
        box-sizing: border-box;
    }

    body {
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
        line-height: 1.6;
        color: var(--primary-color);
        background-color: var(--background-color);
        padding: 2rem 1rem;
    }

    .container {
        max-width: var(--container-width);
        margin: 0 auto;
    }

    header {
        margin-bottom: 3rem;
        text-align: center;
    }

    h1 {
        font-size: 2.5rem;
        margin-bottom: 0.5rem;
    }

    .subtitle {
        color: var(--secondary-color);
        font-size: 1.1rem;
        margin-bottom: 1.5rem;
    }

    .title {
        font-size: 1.5rem;
    }

    .bio {
        max-width: 600px;
        margin: 0 auto 1.5rem;
        line-height: 1.7;
    }

    .note {
        font-size: 0.9rem;
        color: var(--secondary-color);
        margin-top: 1rem;
    }

    a {
        text-decoration: none;
        transition: color 0.2s;
    }

    a:hover {
        text-decoration: underline;
    }

    .social-links {
        margin-top: 1.5rem;
    }

    .social-links a {
        display: inline-block;
        margin: 0 0.5rem;
        opacity: 0.7;
        transition: opacity 0.2s;
    }

    .social-links a:hover {
        opacity: 1;
    }

    .posts-list ul {
        list-style: none;
    }

    .posts-list li {
        margin-bottom: 1.5rem;
        padding-bottom: 1.5rem;
        border-bottom: 1px solid #eee;
    }

    
    .posts-list h3 {
        font: 500 1rem/1.6 -apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,Arial,sans-serif;
    }

    .posts-list h3 a {
        text-decoration: none;
        font-size: 1.3rem;
        color: black
        
    }

    .posts-list h3 a:hover {
        color: var(--accent-color);
    }

    .year {
        color: var(--secondary-color);
        font-size: 1.4rem;
        margin: 2rem 0 1rem;
        border-bottom: none !important;
    }

    @media (max-width: 600px) {
        body {
            padding: 1rem;
        }

        h1 {
            font-size: 2rem;
        }

        .bio {
            font-size: 0.95rem;
        }
    }
</style>

"#,
    );

    let mut li_list = String::new();
    let mut posts: Vec<String> = posts.into_iter().rev().collect();
    for post in posts {
        let post_href = post.split(")").collect::<Vec<&str>>()[0];
        println!("{}", post);
        rename_dir(&post, &post_href);

        let post_name_split: &Vec<&str> = &post.split(")--").collect();

        if post_name_split.len() != 1 {
            let content = fs::read(format!("./drafts/{}.md", post)).unwrap_or(vec![]);
            let subtitle = content.split_at_checked(200).unwrap_or((&[], &[])).0;
            let sub_string = String::from_utf8(subtitle.to_vec()).unwrap();

            let (date, sub): (&str, &str) = {
                let split = sub_string.split('`').collect::<Vec<&str>>();
                (
                    split.get(1).map_or("", |v| v),
                    split.get(2).map_or("", |v| v),
                )
            };

            let mut dis_name = String::new();
            dis_name.push_str(&post_name_split[2].replace("-", " "));

            if &post_name_split[1] == &"pub" {
                dis_name.push_str(" &#128215");
            } else {
                dis_name.push_str(" &#128216");
            }

            li_list.push_str(&format!(
                r#"
    <li>
    <p>{}</p>
    <h3><a href="./post/{}/post.html">{}</a></h3>
    <p class="subtitle">{}...</p>
    <a href="./post/{}/post.html">read more</a>
    </li>
    "#,
                date, &post_href, &dis_name, sub, &post_href
            ))
        } else {
            li_list.push_str(&format!(
                r#"
    <li>
    <h3 class="year"><p>{}</p></h3>
    </li>
    "#,
                &post.split("--").collect::<Vec<&str>>()[1]
            ))
        }
    }

    let mut ul = String::from(
        r#"
<body>
    <div class="container">
        <header>
            <h1 class="title">Tommaso De Ponti</h1>
            <a href="https://x.com/heytdep" target="__blank"><div class="subtitle">@heytdep</div></a>
            
            <div class="bio">
                <p>Co-founder <a target="_blank" href="https://github.com/xycloo/">Xycloo Labs</a>.</p>
                <p>Working with VMs, cloud computing infra, TEEs and blockchain. Interested in research about VMs, MeV, validators, blockchain microstructure, TEEs, and DeFi.</p>
                <p class="note">Articles with the &#128215 suffix are intended for general public, 
                   &#128216 are personal notes.</p>
            </div>
        </header>

        <main>
            <div class="posts-list">
                <ul>"#,
    );

    ul.push_str(&li_list);
    ul.push_str("</ul></nav></div></body>");

    head.push_str(&ul);
    head
}

fn write_index() {
    let posts_dirs = get_comp();

    let posts = posts_dirs
        .iter()
        .filter_map(|entry| {
            entry
                .as_path()
                .file_name()
                .and_then(|n| n.to_str().map(|s| String::from(s)))
        })
        .collect::<Vec<String>>();

    let content = gen_index_content(posts);
    fs::write("./index.html", content);
    println!("[+] Successfully built posts");
}

fn main() {
    preemptive_remove();
    let drafts = get_drafts();

    let posts = drafts
        .iter()
        .filter_map(|entry| {
            entry
                .as_path()
                .file_name()
                .and_then(|n| n.to_str().map(|s| String::from(s)))
        })
        .collect::<Vec<String>>();

    for draft in posts {
        println!("{:?}", draft);
        to_html_pd(draft);
    }

    write_index()
}
