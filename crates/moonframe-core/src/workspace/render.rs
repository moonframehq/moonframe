//! Drawing: what views get as `app`, running the views that need to run,
//! and keeping the instances they draw.

use super::{messages::topic_message, *};

impl Workspace {
    /// Draws everything, hands each module loaded since the last time the
    /// current value of the topics it hears (so one that joins late starts
    /// in step), draws again if that changed anything, and saves state.
    pub(super) fn settle(&mut self) {
        // Each round can mount instances, which then hear their topics.
        for _ in 0..4 {
            self.render();
            // What each module loaded since the last round hears now.
            let catch_up: VecDeque<(String, String, Data)> = self
                .listeners()
                .filter(|(_, slot)| slot.fresh)
                .filter_map(|(key, slot)| Some((key, slot.module.as_ref()?)))
                .flat_map(|(key, module)| {
                    let topics = self
                        .topics
                        .iter()
                        .map(|(name, value)| (name.clone(), value));
                    let providers = self
                        .host
                        .iter()
                        .map(|(name, value)| (format!("{PROVIDER_TOPIC}{name}"), value));
                    topics
                        .chain(providers)
                        .filter(|(name, _)| module.subscribes(name))
                        .map(|(name, value)| {
                            (
                                format!("{TOPIC}{name}"),
                                key.to_owned(),
                                topic_message(&name, value),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            let fresh: Vec<String> = self
                .listeners()
                .filter(|(_, slot)| slot.fresh)
                .map(|(key, _)| key.to_owned())
                .collect();
            for key in fresh {
                if let Some(slot) = self.slot_mut(&key) {
                    slot.fresh = false;
                }
            }
            // What modules sent from `init`, after the catch-up.
            let mut catch_up = catch_up;
            catch_up.extend(self.take_pending());
            if catch_up.is_empty() {
                break;
            }
            self.process(catch_up);
        }
        self.save();
    }

    /// What views get as `app`, for tests.
    #[cfg(test)]
    pub(crate) fn app_data(&self) -> AppData {
        self.app()
    }

    /// What every `view` gets as `app`: what the host shares, the mods,
    /// which regions they asked for, and the theme.
    fn app(&self) -> AppData {
        let mut regions: BTreeMap<String, Vec<Data>> = BTreeMap::new();
        let names: BTreeSet<&str> = self
            .mods
            .values()
            .filter_map(|slot| slot.meta()?.region.as_deref())
            .collect();
        for region in names {
            regions.insert(
                region.to_owned(),
                self.region(region).into_iter().map(Data::from).collect(),
            );
        }
        let mut app = self.host.clone();
        app.insert("kinds".to_owned(), self.kinds());
        app.insert(
            "looks".to_owned(),
            Data::Map(
                self.stylesheet
                    .looks
                    .iter()
                    .map(|(kind, look)| (kind.clone(), Data::from(look.as_str())))
                    .collect(),
            ),
        );
        app.extend([
            ("dark".to_owned(), Data::Bool(self.dark)),
            ("appearance".to_owned(), {
                let (mode, fields) = &self.appearance;
                // Until the app says (and in checks), the stock look in both.
                let mut appearance = BTreeMap::from([
                    ("theme".to_owned(), Data::from("Default")),
                    ("light_theme".to_owned(), Data::from("Default")),
                    ("dark_theme".to_owned(), Data::from("Default")),
                    ("themes".to_owned(), Data::List(Vec::new())),
                ]);
                appearance.extend(fields.clone());
                appearance.insert("mode".to_owned(), Data::from(mode.name()));
                appearance.insert("dark".to_owned(), Data::Bool(self.dark));
                Data::Map(appearance)
            }),
            ("window".to_owned(), self.window.data()),
            (
                "capabilities".to_owned(),
                Data::Map(
                    self.capabilities
                        .iter()
                        .map(|(name, enabled)| (name.clone(), Data::Bool(*enabled)))
                        .collect(),
                ),
            ),
            (
                "mods".to_owned(),
                Data::List(self.mod_names().map(Data::from).collect()),
            ),
            (
                "regions".to_owned(),
                Data::Map(
                    regions
                        .into_iter()
                        .map(|(name, members)| (name, Data::List(members)))
                        .collect(),
                ),
            ),
        ]);
        app
    }

    /// What a tab bar's `+` menu offers, `app.kinds`: each component and mod
    /// with a `tab` entry, as `{ kind = "component:terminal" | "mod:explorer",
    /// title, icon, props, report }`, by title.
    fn kinds(&self) -> Data {
        let components = self.components.iter().filter_map(|component| {
            Some((
                format!("component:{}", component.name),
                &component.title,
                component.tab.as_ref()?,
            ))
        });
        let mods = self.mods.iter().filter_map(|(name, slot)| {
            let meta = slot.meta()?;
            Some((
                format!("mod:{name}"),
                meta.title.as_ref().unwrap_or(name),
                meta.tab.as_ref()?,
            ))
        });
        let mut kinds: Vec<(String, Data)> = components
            .chain(mods)
            .map(|(kind, title, tab)| {
                let title = tab.title.clone().unwrap_or_else(|| title.clone());
                let mut fields = BTreeMap::from([
                    ("kind".to_owned(), Data::from(kind.as_str())),
                    ("title".to_owned(), Data::from(title.as_str())),
                    ("props".to_owned(), tab.props.clone()),
                ]);
                if let Some(icon) = &tab.icon {
                    fields.insert("icon".to_owned(), Data::from(icon.name()));
                }
                if let Some(report) = &tab.report {
                    fields.insert("report".to_owned(), Data::from(report.as_str()));
                }
                (title, Data::Map(fields))
            })
            .collect();
        kinds.sort_by(|a, b| a.0.cmp(&b.0));
        Data::List(kinds.into_iter().map(|(_, kind)| kind).collect())
    }

    /// Runs every view whose state, props, lists or `app` keys it read
    /// changed since it was last drawn; the others keep their view.
    pub(super) fn render(&mut self) {
        let app = self.app();
        if app != *self.app {
            self.versions.compare(&self.app, &app);
            self.app = Rc::new(app);
        }
        let app = self.app.clone();
        let style = self.style_generation;
        let mut ran = usize::from(
            self.shell
                .render(&app, &self.versions, &self.stylesheet, style),
        );
        let demos = self.demos.values_mut().filter(|_| self.demos_shown);
        for slot in self.mods.values_mut().chain(demos) {
            ran += usize::from(slot.render(&app, &self.versions, &self.stylesheet, style));
        }
        self.views_run += ran;
        self.render_instances(&app);
    }

    /// Has the list `id` drawn by `owner` build the rows in `window`, as the
    /// window scrolls it, and draws again. Whether that changed anything.
    pub fn set_list_window(&mut self, owner: &str, id: &str, window: ListWindow) -> bool {
        let Some(slot) = self.slot_mut(owner) else {
            return false;
        };
        if slot.lists.get(id) == Some(&window) {
            return false;
        }
        slot.lists.insert(id.to_owned(), window);
        slot.dirty = true;
        self.settle();
        true
    }

    /// What every module sent from `init` and is not yet delivered.
    fn take_pending(&mut self) -> Vec<(String, String, Data)> {
        let slots = std::iter::once((SHELL, &mut self.shell))
            .chain(
                self.mods
                    .iter_mut()
                    .map(|(name, slot)| (name.as_str(), slot)),
            )
            .chain(
                self.demos
                    .iter_mut()
                    .map(|(name, slot)| (name.as_str(), slot)),
            )
            .chain(
                self.instances
                    .iter_mut()
                    .map(|(key, mounted)| (key.as_str(), &mut mounted.slot)),
            );
        let mut sends = Vec::new();
        for (key, slot) in slots {
            if let Some(module) = &mut slot.module {
                sends.extend(
                    module
                        .take_pending()
                        .into_iter()
                        .map(|send| (key.to_owned(), send.to, send.msg)),
                );
            }
        }
        sends
    }

    /// Draws every instance the views hold: creating the new ones (with the
    /// state they saved), handing each its props, and following instances
    /// drawn inside instances. Those no longer drawn are dropped; their
    /// state stays saved, so they resume where they were if drawn again.
    fn render_instances(&mut self, app: &Rc<AppData>) {
        let mut queue: Vec<(String, Instance)> = Vec::new();
        let collect = |owner: &str, view: Option<&Node>, queue: &mut Vec<(String, Instance)>| {
            if let Some(view) = view {
                view.instances(&mut |instance| queue.push((owner.to_owned(), instance.clone())));
            }
        };
        collect(SHELL, self.shell.view(), &mut queue);
        for (name, slot) in self.mods.iter().chain(&self.demos) {
            collect(name, slot.view(), &mut queue);
        }
        let mut drawn = BTreeSet::new();
        while let Some((owner, instance)) = queue.pop() {
            let key = instance.key(&owner);
            if drawn.contains(&key) {
                continue;
            }
            if drawn.len() == MAX_INSTANCES {
                if let Some(slot) = self.slot_mut(&owner) {
                    slot.view_error = Some(Error::new(format!(
                        "more than {MAX_INSTANCES} instances; is an id made from data?"
                    )));
                }
                break;
            }
            drawn.insert(key.clone());
            if !self.instances.contains_key(&key) {
                let restored = if owner.starts_with(DEMO) {
                    None
                } else {
                    self.restore(&key)
                };
                let slot = Slot {
                    restored,
                    props: instance.props.clone(),
                    ..Slot::default()
                };
                self.instances.insert(key.clone(), Mounted {
                    slot,
                    of: instance.of.clone(),
                    owner: owner.clone(),
                });
                self.load_instance(&key);
            }
            let Some(mounted) = self.instances.get_mut(&key) else {
                continue;
            };
            mounted.slot.set_props(&instance.props);
            if mounted
                .slot
                .render(app, &self.versions, &self.stylesheet, self.style_generation)
            {
                self.views_run += 1;
            }
            collect(&key, mounted.slot.view(), &mut queue);
        }
        // Instances in hidden `lazy` panes sleep: kept as they are if they
        // were started, with what they mount, but neither drawn nor started.
        let mut asleep: Vec<(String, Instance)> = Vec::new();
        let doze = |owner: &str, view: Option<&Node>, queue: &mut Vec<(String, Instance)>| {
            if let Some(view) = view {
                view.asleep(&mut |instance| queue.push((owner.to_owned(), instance.clone())));
            }
        };
        doze(SHELL, self.shell.view(), &mut asleep);
        for (name, slot) in self.mods.iter().chain(&self.demos) {
            doze(name, slot.view(), &mut asleep);
        }
        for (key, mounted) in &self.instances {
            if drawn.contains(key) {
                doze(key, mounted.slot.view(), &mut asleep);
            }
        }
        let mut kept = BTreeSet::new();
        while let Some((owner, instance)) = asleep.pop() {
            let key = instance.key(&owner);
            if drawn.contains(&key) || !kept.insert(key.clone()) {
                continue;
            }
            if let Some(mounted) = self.instances.get(&key) {
                // What it mounts, awake or asleep in its own view, sleeps too.
                collect(&key, mounted.slot.view(), &mut asleep);
                doze(&key, mounted.slot.view(), &mut asleep);
            }
        }
        self.instances
            .retain(|key, _| drawn.contains(key) || kept.contains(key));
    }
}
