//! Building the app as a bundle, and checking for and installing updates.

use super::*;

impl MoonframeApp {
    /// Builds the app this window runs as a release bundle, in the
    /// background, and shows it in Finder when it is done.
    pub(crate) fn build_app(&mut self, cx: &mut Context<Self>) {
        if !cfg!(feature = "studio") {
            return;
        }
        if self.building {
            self.show_toast("The app is already being built", cx);
            return;
        }
        let Some(checkout) = crate::build::checkout(&self.folders.builtin) else {
            self.show_toast(
                "Building needs the app's sources: run it from its checkout",
                cx,
            );
            return;
        };
        let build = crate::build::Build {
            checkout,
            profile: self.folders.profile.clone(),
            user: self.folders.user.clone(),
        };
        self.building = true;
        // Stays up until the build ends: cargo can take minutes.
        let id = self.next_toast();
        self.toast = Some((id, format!("Building {}…", crate::title()).into()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let built = cx
                .background_executor()
                .spawn(async move { build.run() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.building = false;
                match built {
                    Ok(bundle) => {
                        let name = bundle
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        crate::build::reveal(&bundle);
                        this.show_toast_text(format!("Built {name}").into(), cx);
                    },
                    Err(problem) => {
                        this.show_toast_text(format!("The build failed: {problem}").into(), cx);
                    },
                }
            });
        })
        .detach();
    }

    /// Turns Beta Releases on or off, and on, looks for a pre-release at
    /// once rather than at the next daily check.
    pub(crate) fn toggle_beta_releases(&mut self, cx: &mut Context<Self>) {
        let on = !crate::update::beta();
        crate::update::set_beta(on);
        // The menu shows the switch checked while it is on.
        cx.set_menus(menus(self.workspace.menus()));
        if on {
            self.show_toast("Beta Releases: updates include pre-releases", cx);
            self.check_for_updates(false, cx);
        } else {
            self.show_toast("Beta Releases are off: only published releases", cx);
        }
    }

    /// Asks the app's repository for a newer release and offers it. `asked`:
    /// the user chose Check for Updates, so every outcome is told; the
    /// daily check speaks only of a newer release.
    pub(crate) fn check_for_updates(&mut self, asked: bool, cx: &mut Context<Self>) {
        let Some(source) = crate::update::source().cloned() else {
            if asked {
                self.show_toast("This app names no repository to update from", cx);
            }
            return;
        };
        if self.updating {
            if asked {
                self.show_toast("Already checking for updates", cx);
            }
            return;
        }
        self.updating = true;
        if asked {
            self.show_toast("Checking for updates…", cx);
        }
        let profile = self.folders.profile.clone();
        let beta = crate::update::beta();
        cx.spawn(async move |this, cx| {
            let looked = source.clone();
            let found = cx
                .background_executor()
                .spawn(async move { crate::update::check(&looked, beta) })
                .await;
            // Beta Releases changed while this check ran: its answer is for
            // the old choice, so it neither counts as the day's nor is
            // offered; a check for the new choice runs instead.
            if crate::update::beta() != beta {
                let _ = this.update(cx, |this, cx| {
                    this.updating = false;
                    this.check_for_updates(asked, cx);
                });
                return;
            }
            // Only a check that reached GitHub counts as the day's: one that
            // failed (offline, rate limited) is tried again within the hour.
            if found.is_ok() {
                crate::update::checked(profile.as_deref());
            }
            let title = crate::title();
            let release = match found {
                Ok(crate::update::Found::Newer(release)) => release,
                other => {
                    let _ = this.update(cx, |this, cx| {
                        this.updating = false;
                        let message = match other {
                            Ok(crate::update::Found::UpToDate { latest }) => {
                                format!("{title} is up to date ({latest})")
                            },
                            Ok(_) => format!("{title} has no release yet"),
                            Err(problem) => {
                                tracing::warn!("checking for updates: {problem}");
                                format!("Cannot check for updates: {problem}")
                            },
                        };
                        if asked {
                            this.show_toast_text(message.into(), cx);
                        }
                    });
                    return;
                },
            };
            let installable = release.installable(&source);
            let answers: &[&str] = match (installable, &release.asset) {
                (true, _) => &["Install and Relaunch", "Release Notes", "Later"],
                (false, Some(_)) => &["Download", "Release Notes", "Later"],
                (false, None) => &["Release Notes", "Later"],
            };
            let message = format!("{title} {} is available", release.version);
            let detail = if installable {
                format!(
                    "You have {}. It downloads and replaces this copy, then {title} starts again.",
                    source.version
                )
            } else {
                format!("You have {}.", source.version)
            };
            let window =
                cx.update(|cx| cx.active_window().or_else(|| cx.windows().first().copied()));
            let prompt = window.and_then(|window| {
                cx.update_window(window, |_, window, cx| {
                    window.prompt(PromptLevel::Info, &message, Some(&detail), answers, cx)
                })
                .ok()
            });
            let choice = match prompt {
                Some(answer) => answer
                    .await
                    .ok()
                    .and_then(|index| answers.get(index).copied()),
                None => None,
            };
            match choice {
                Some("Install and Relaunch") => {
                    let _ = this.update(cx, |this, cx| this.install_update(release, source, cx));
                    return;
                },
                Some("Download") => {
                    if let Some(asset) = &release.asset {
                        cx.update(|cx| cx.open_url(&asset.url));
                    }
                },
                Some("Release Notes") => cx.update(|cx| cx.open_url(&release.page)),
                _ => {},
            }
            let _ = this.update(cx, |this, _| this.updating = false);
        })
        .detach();
    }

    /// Downloads `release` and puts it in place of the running app, which
    /// quits and starts again; on a failure, says why and opens the
    /// release's page to download it from.
    fn install_update(
        &mut self,
        release: crate::update::Release,
        source: crate::update::Source,
        cx: &mut Context<Self>,
    ) {
        // Stays up until the download ends.
        let id = self.next_toast();
        self.toast = Some((
            id,
            format!("Downloading {} {}…", crate::title(), release.version).into(),
        ));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let staged = cx
                .background_executor()
                .spawn({
                    let release = release.clone();
                    async move {
                        crate::update::stage(&release, &source).and_then(crate::update::relaunch)
                    }
                })
                .await;
            match staged {
                Ok(()) => {
                    tracing::info!("installing {} and starting again", release.version);
                    cx.update(|cx| cx.quit());
                },
                Err(problem) => {
                    tracing::warn!("installing the update: {problem}");
                    cx.update(|cx| cx.open_url(&release.page));
                    let _ = this.update(cx, |this, cx| {
                        this.updating = false;
                        this.show_toast_text(format!("The update failed: {problem}").into(), cx);
                    });
                },
            }
        })
        .detach();
    }
}
