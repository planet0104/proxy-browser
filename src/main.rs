#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
use rocket::{http::ContentType, response::content};
use rocket_contrib::serve::StaticFiles;
use scraper::{Html, Selector};
use std::convert::TryFrom;
use uriparse::URI;

fn replace_css(
    css: String,
    current_path: &str,
    base_path: &str,
) -> Result<String, Box<std::error::Error>> {
    let mut css_out = css.clone();
    let mut split = css.split("url(");
    let _ = split.next();

    while let Some(s) = split.next() {
        if let Some(mut url) = s.split(")").next() {
            if !url.starts_with("data:") {
                let mut trim = false;
                if url.starts_with("'") && url.ends_with("'") {
                    url = url.trim_matches('\'');
                    trim = true;
                }

                let new_url = if url.starts_with("https://") || url.starts_with("http://") {
                    url.to_string()
                } else if url.starts_with("//") {
                    let prefix = if base_path.starts_with("http://") {
                        "http:"
                    } else {
                        "https:"
                    };
                    format!("{}{}", prefix, url)
                } else if url.starts_with("/") {
                    format!("{}{}", base_path, url)
                } else {
                    format!("{}/{}", current_path, url)
                };
                if trim {
                    css_out = css_out.replace(
                        &format!("url('{}')", url),
                        &format!("url('/img?url={}')", hex::encode(&new_url)),
                    );
                } else {
                    css_out = css_out.replace(
                        &format!("url({})", url),
                        &format!("url(/img?url={})", hex::encode(new_url)),
                    );
                }
            }
        }
    }

    Ok(css_out)
}

#[get("/?<url>&<current_path>&<base_path>")]
fn css(
    url: String,
    current_path: String,
    base_path: String,
) -> Result<content::Css<String>, Box<std::error::Error>> {
    let css = fetch_text(&decode_url(url)?)?;
    Ok(content::Css(replace_css(
        css,
        &decode_url(current_path)?,
        &decode_url(base_path)?,
    )?))
}

#[get("/?<url>")]
fn img(url: String) -> Result<content::Content<Vec<u8>>, Box<std::error::Error>> {
    let mut content_type = ContentType::Binary;
    if url.ends_with(".jpg") || url.ends_with(".jpeg") {
        content_type = ContentType::JPEG;
    } else if url.ends_with(".png") {
        content_type = ContentType::PNG;
    } else if url.ends_with(".gif") {
        content_type = ContentType::GIF;
    } else if url.ends_with(".bmp") {
        content_type = ContentType::BMP;
    } else if url.ends_with(".webp") {
        content_type = ContentType::WEBP;
    } else if url.ends_with(".ico") {
        content_type = ContentType::Icon;
    } else if url.ends_with(".svg") {
        content_type = ContentType::SVG;
    } else if url.ends_with(".webm") {
        content_type = ContentType::WEBM;
    } else if url.ends_with(".weba") {
        content_type = ContentType::WEBA;
    } else if url.ends_with(".tif") || url.ends_with(".tiff") {
        content_type = ContentType::TIFF;
    }
    Ok(content::Content(
        content_type,
        fetch_binary(&decode_url(url)?),
    ))
}

#[get("/?<url>")]
fn js(url: String) -> Result<content::JavaScript<String>, Box<std::error::Error>> {
    Ok(content::JavaScript(fetch_text(&decode_url(url)?)?))
}

#[get("/?<url>")]
fn html(url: String) -> Result<content::Html<String>, Box<std::error::Error>> {
    Ok(content::Html(fetch_html(&decode_url(url)?)?))
}

fn decode_url(url: String) -> Result<String, Box<std::error::Error>> {
    Ok(String::from_utf8(hex::decode(url)?)?)
}

fn fetch_text(url: &str) -> reqwest::Result<String> {
    let resp = reqwest::get(url)?.text()?;
    Ok(resp)
}

fn fetch_binary(url: &str) -> Vec<u8> {
    let mut buf: Vec<u8> = vec![];
    if let Ok(mut resp) = reqwest::get(url) {
        let _ = resp.copy_to(&mut buf);
    }
    buf
}

fn get_url_path(url: &str) -> (String, String) {
    let mut uri = URI::try_from(url).unwrap();
    uri.map_path(|mut path| {
        path.pop();
        path
    });
    let mut builder = uri.into_builder();
    builder
        .query(None::<uriparse::query::Query>)
        .fragment(None::<uriparse::Fragment>);
    let mut uri = builder.build().unwrap();
    let mut current_path = uri.to_string();
    if current_path.ends_with("/") {
        current_path.pop();
    }
    let _ = uri.set_path("");
    let mut base_path = uri.to_string();
    if base_path.ends_with("/") {
        base_path.pop();
    }
    (current_path, base_path)
}

fn replace(url: &str, html: &str, href: &str, attr: &str, ime: &str) -> String {
    let (current_path, base_path) = get_url_path(url);

    let final_href = if href.starts_with("https://") || href.starts_with("http://") {
        href.to_string()
    } else if href.starts_with("//") {
        let prefix = if base_path.starts_with("http://") {
            "http:"
        } else {
            "https:"
        };
        format!("{}{}", prefix, href)
    } else if href.starts_with("/") {
        format!("{}{}", base_path, href)
    } else {
        format!("{}/{}", current_path, href)
    };

    html.replace(
        &format!("{}=\"{}\"", attr, href),
        &format!(
            "{}=\"{}?url={}&current_path={}&base_path={}\"",
            attr,
            ime,
            hex::encode(&final_href),
            hex::encode(&current_path),
            hex::encode(&base_path)
        ),
    )
    .replace(
        &format!("{}='{}'", attr, href),
        &format!(
            "{}=\"{}?url={}&current_path={}&base_path={}\"",
            attr,
            ime,
            hex::encode(&final_href),
            hex::encode(&current_path),
            hex::encode(&base_path)
        ),
    )
}

fn fetch_html(url: &str) -> reqwest::Result<String> {
    let mut html = reqwest::get(url)?.text()?;
    let document = Html::parse_document(&html);

    //替换img
    let selector = Selector::parse("img").unwrap();
    for element in document.select(&selector) {
        if let Some(href) = element.value().attr("src") {
            html = replace(url, &html, href, "src", "img");
        }
    }

    //替换内联CSS
    // let selector = Selector::parse("style").unwrap();
    // for element in document.select(&selector) {
    //     let css = element.inner_html();
    //     //println!("内联CSS{}", css);
    //     html = html.replace(&css, "内联CSS");
    // }
    let (current_path, base_path) = get_url_path(url);
    html = replace_css(html.clone(), &current_path, &base_path).unwrap();

    //替换css
    let selector = Selector::parse("link").unwrap();
    for element in document.select(&selector) {
        if let Some(rel) = element.value().attr("rel") {
            if rel != "stylesheet" {
                continue;
            }
        }
        if let Some(href) = element.value().attr("href") {
            html = replace(url, &html, href, "href", "css");
        }
    }

    //替换js
    let selector = Selector::parse("script").unwrap();
    for element in document.select(&selector) {
        if let Some(src) = element.value().attr("src") {
            html = replace(url, &html, src, "src", "js");
        }
    }

    //替换html链接
    let selector = Selector::parse("a").unwrap();
    for element in document.select(&selector) {
        if let Some(href) = element.value().attr("href") {
            html = replace(url, &html, href, "href", "html");
        }
    }
    Ok(html)
}

fn main() {
    rocket::ignite()
        .mount("/html", routes![html])
        .mount("/css", routes![css])
        .mount("/js", routes![js])
        .mount("/img", routes![img])
        .mount("/", StaticFiles::from("static"))
        .launch();
}
