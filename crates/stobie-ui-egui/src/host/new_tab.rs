//! New-tab links in hosted web applications open without an opener.
//!
//! eframe's web runner opens `OpenUrl { new_tab: true }` with
//! `window.open(url, "_blank")`, which hands the new page `window.opener`: a
//! linked page could then navigate the application's own tab elsewhere. The
//! web host's plugin takes those requests from each pass's output before the
//! runner sees them and opens them with `noopener,noreferrer` instead.
//! Same-tab requests are left to eframe.

/// Take every new-tab request out of `output`, returning the URLs in order.
#[cfg_attr(not(all(target_arch = "wasm32", feature = "eframe")), allow(dead_code))]
pub(super) fn take_new_tab_urls(output: &mut egui::PlatformOutput) -> Vec<String> {
    let mut urls = Vec::new();
    output.commands.retain(|command| match command {
        egui::OutputCommand::OpenUrl(open) if open.new_tab => {
            urls.push(open.url.clone());
            false
        }
        _ => true,
    });
    urls
}

#[cfg(all(target_arch = "wasm32", feature = "eframe"))]
pub(super) struct NewTabLinks;

#[cfg(all(target_arch = "wasm32", feature = "eframe"))]
impl egui::Plugin for NewTabLinks {
    fn debug_name(&self) -> &'static str {
        "stobie.new-tab-links"
    }

    fn output_hook(&mut self, _context: &egui::Context, output: &mut egui::FullOutput) {
        // Each pass's own output arrives here once, so a request from a pass
        // egui later discards still opens exactly once.
        let Some(window) = web_sys::window() else {
            return;
        };
        for url in take_new_tab_urls(&mut output.platform_output) {
            if let Err(error) =
                window.open_with_url_and_target_and_features(&url, "_blank", "noopener,noreferrer")
            {
                web_sys::console::error_1(&error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_new_tab_requests_are_taken_in_order() {
        let mut output = egui::PlatformOutput::default();
        output.commands = vec![
            egui::OutputCommand::OpenUrl(egui::OpenUrl::new_tab("https://example.org/a")),
            egui::OutputCommand::CopyText("kept".into()),
            egui::OutputCommand::OpenUrl(egui::OpenUrl {
                url: "https://example.org/same".into(),
                new_tab: false,
            }),
            egui::OutputCommand::OpenUrl(egui::OpenUrl::new_tab("https://example.org/b")),
        ];
        assert_eq!(
            take_new_tab_urls(&mut output),
            ["https://example.org/a", "https://example.org/b"]
        );
        assert_eq!(output.commands.len(), 2);
        assert!(matches!(
            &output.commands[1],
            egui::OutputCommand::OpenUrl(open) if !open.new_tab
        ));
    }
}
