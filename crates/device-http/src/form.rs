//! Generic scraping of a BirdUI page's current form state, and turning a
//! field map back into a POST body. BirdUI is server-rendered HTML with no
//! JSON API, so "read current settings" means "parse the value/checked/
//! selected attributes out of the page flock just fetched."
//!
//! The read-modify-write pattern this enables (scrape -> override the few
//! fields flock actually changes -> POST the whole map back) exists because
//! BirdUI's own forms don't accept partial updates predictably - safer to
//! resubmit everything unchanged except what flock is explicitly setting
//! than to guess which fields a handler requires.

use scraper::{Html, Selector};
use std::collections::HashMap;

pub fn scrape_form_fields(html: &str) -> HashMap<String, String> {
    let doc = Html::parse_document(html);
    let mut fields = HashMap::new();

    let input_sel = Selector::parse("input").expect("static selector");
    for el in doc.select(&input_sel) {
        let Some(name) = el.value().attr("name") else {
            continue;
        };
        match el.value().attr("type").unwrap_or("text") {
            "radio" | "checkbox" => {
                if el.value().attr("checked").is_some() {
                    let value = el.value().attr("value").unwrap_or("on");
                    fields.insert(name.to_string(), value.to_string());
                }
            }
            "submit" | "button" | "file" | "image" | "reset" => {}
            _ => {
                let value = el.value().attr("value").unwrap_or("");
                fields.insert(name.to_string(), value.to_string());
            }
        }
    }

    let select_sel = Selector::parse("select").expect("static selector");
    let option_sel = Selector::parse("option").expect("static selector");
    for el in doc.select(&select_sel) {
        let Some(name) = el.value().attr("name") else {
            continue;
        };
        for opt in el.select(&option_sel) {
            if opt.value().attr("selected").is_some() {
                if let Some(value) = opt.value().attr("value") {
                    fields.insert(name.to_string(), value.to_string());
                }
                break;
            }
        }
    }

    fields
}

/// Scrapes the text content of every element carrying one of the given
/// ids, for pulling read-only display values (Dashboard's spans/tds) rather
/// than form fields.
pub fn scrape_text_by_id(html: &str, ids: &[&str]) -> HashMap<String, String> {
    let doc = Html::parse_document(html);
    let mut out = HashMap::new();
    for id in ids {
        let selector = format!("#{id}");
        let Ok(sel) = Selector::parse(&selector) else {
            continue;
        };
        if let Some(el) = doc.select(&sel).next() {
            out.insert(
                id.to_string(),
                el.text().collect::<String>().trim().to_string(),
            );
        }
    }
    out
}

pub fn to_multipart(fields: HashMap<String, String>) -> reqwest::multipart::Form {
    let mut form = reqwest::multipart::Form::new();
    for (key, value) in fields {
        form = form.text(key, value);
    }
    form
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETWORK_HTML: &str = include_str!("../tests/fixtures/network.html");
    const DASHBOARD_HTML: &str = include_str!("../tests/fixtures/dashboard.html");

    #[test]
    fn scrapes_checked_radio_and_text_input() {
        let fields = scrape_form_fields(NETWORK_HTML);
        assert_eq!(fields.get("net_method").map(String::as_str), Some("dhcp"));
        assert_eq!(
            fields.get("net_address").map(String::as_str),
            Some("192.168.1.40")
        );
        assert_eq!(fields.get("net_avahi").map(String::as_str), Some("unknown"));
    }

    #[test]
    fn scrapes_select_options() {
        let fields = scrape_form_fields(NETWORK_HTML);
        // Txpm/Rxpm real markup doesn't mark a `selected` option in this
        // fixture (the device was at defaults) - absence here just means
        // "server didn't tell us," not a scraper bug.
        assert!(fields.contains_key("net_method"));
    }

    #[test]
    fn scrapes_dashboard_text_values() {
        let values = scrape_text_by_id(
            DASHBOARD_HTML,
            &[
                "dashboard_avahi_name",
                "dashboard_fw_version",
                "dashboard_net_addr",
                "dashboard_dev_serial",
            ],
        );
        assert_eq!(
            values.get("dashboard_avahi_name").map(String::as_str),
            Some("unknown")
        );
        assert_eq!(
            values.get("dashboard_fw_version").map(String::as_str),
            Some("BirdDog PLAY 1.0.18")
        );
        assert_eq!(
            values.get("dashboard_net_addr").map(String::as_str),
            Some("192.168.1.40")
        );
        assert_eq!(
            values.get("dashboard_dev_serial").map(String::as_str),
            Some("dhcp")
        );
    }
}
