//! New-tab links in web applications open without an opener.
//!
//! eframe's web runner opens `OpenUrl { new_tab: true }` with
//! `window.open(url, "_blank")`, which hands the new page `window.opener`: a
//! linked page could then navigate the application's own tab elsewhere. The
//! [`NewTabLinks`] plugin takes those requests from each pass's output before
//! the runner sees them and opens them with `noopener,noreferrer` instead.
//! Same-tab requests are left to eframe.
//!
//! Every [`AppHost`](crate::AppHost) installs the plugin, and so does each of
//! Stobie's link components when it asks for a new tab, so the protection
//! holds however the application starts: through `WebHost`, or through
//! eframe's web runner directly. Natively the plugin leaves every request to
//! eframe, which opens the platform's browser.

/// Take every new-tab request out of `output`, returning the URLs in order.
#[cfg_attr(not(all(target_arch = "wasm32", feature = "eframe")), allow(dead_code))]
pub(crate) fn take_new_tab_urls(output: &mut egui::PlatformOutput) -> Vec<String> {
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

pub(crate) struct NewTabLinks;

impl egui::Plugin for NewTabLinks {
    fn debug_name(&self) -> &'static str {
        "stobie.new-tab-links"
    }

    #[cfg(all(target_arch = "wasm32", feature = "eframe"))]
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

/// Open every new-tab request made in `context` without an opener, in the
/// browser. [`AppHost`](crate::AppHost) and Stobie's link components already
/// do this; an application that runs eframe's web runner without an
/// `AppHost`, and asks for new tabs from its own code, calls it once with its
/// context. Calling it again changes nothing, and natively it changes nothing.
pub fn install_new_tab_links(context: &egui::Context) {
    context.add_plugin(NewTabLinks);
}

/// Ask for `url` in a new tab, without an opener in the browser.
pub(crate) fn open_new_tab(context: &egui::Context, url: impl ToString) {
    install_new_tab_links(context);
    context.open_url(egui::OpenUrl::new_tab(url));
}

/// Whether `context` opens new tabs without an opener.
#[cfg(test)]
pub(crate) fn installed(context: &egui::Context) -> bool {
    context.with_plugin::<NewTabLinks, _>(|_| ()).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_new_tab_requests_are_taken_in_order() {
        let mut output = egui::PlatformOutput {
            commands: vec![
                egui::OutputCommand::OpenUrl(egui::OpenUrl::new_tab("https://example.org/a")),
                egui::OutputCommand::CopyText("kept".into()),
                egui::OutputCommand::OpenUrl(egui::OpenUrl {
                    url: "https://example.org/same".into(),
                    new_tab: false,
                }),
                egui::OutputCommand::OpenUrl(egui::OpenUrl::new_tab("https://example.org/b")),
            ],
            ..Default::default()
        };
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

    #[test]
    fn a_new_tab_request_installs_the_plugin_and_leaves_requests_natively() {
        let context = egui::Context::default();
        assert!(!installed(&context));
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            open_new_tab(ui.ctx(), "https://example.org/a");
            open_new_tab(ui.ctx(), "https://example.org/b");
        });
        output.textures_delta.clear();
        install_new_tab_links(&context);
        assert!(installed(&context));
        // Natively the requests stay for eframe, which opens the platform's
        // browser.
        assert_eq!(
            output.platform_output.commands,
            [
                egui::OutputCommand::OpenUrl(egui::OpenUrl::new_tab("https://example.org/a")),
                egui::OutputCommand::OpenUrl(egui::OpenUrl::new_tab("https://example.org/b")),
            ]
        );
    }
}
